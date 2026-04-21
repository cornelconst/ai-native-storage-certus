mod bitmap;
mod block_io;
mod buddy;
pub(crate) mod checkpoint;
mod error;
mod recovery;
mod slab;
pub(crate) mod state;
mod write_handle;

#[cfg(any(test, feature = "testing"))]
pub mod superblock;
#[cfg(not(any(test, feature = "testing")))]
pub(crate) mod superblock;

#[cfg(any(test, feature = "testing"))]
pub mod test_support;

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use parking_lot::RwLock;

use interfaces::{
    DmaAllocFn, Extent, ExtentKey, ExtentManagerError, FormatParams, IBlockDevice,
    IExtentManagerV2, ILogger, WriteHandle,
};

use component_macros::define_component;

use crate::block_io::BlockDeviceClient;
use crate::buddy::BuddyAllocator;
use crate::state::ManagerState;
use crate::superblock::{Superblock, SUPERBLOCK_SIZE};

define_component! {
    pub MetadataManagerV2 {
        version: "0.1.0",
        provides: [IExtentManagerV2],
        receptacles: {
            block_device: IBlockDevice,
            logger: ILogger,
        },
        fields: {
            state: Arc<RwLock<Option<ManagerState>>>,
            dma_alloc: Mutex<Option<DmaAllocFn>>,
            superblock: Mutex<Option<Superblock>>,
            checkpoint_interval_ms: AtomicU64,
            shutdown: Arc<AtomicBool>,
            checkpoint_thread: Mutex<Option<JoinHandle<()>>>,
        },
    }
}

impl MetadataManagerV2 {
    pub fn new_inner() -> Arc<Self> {
        let component = MetadataManagerV2::new_default();
        component
            .checkpoint_interval_ms
            .store(5000, Ordering::Relaxed);
        component
    }

    pub fn set_checkpoint_interval(&self, interval: std::time::Duration) {
        self.checkpoint_interval_ms
            .store(interval.as_millis() as u64, Ordering::Relaxed);
    }

    fn get_client(&self) -> Result<BlockDeviceClient, ExtentManagerError> {
        let bd = self
            .block_device
            .get()
            .map_err(|_| error::not_initialized("block device not connected"))?;

        let channels = bd
            .connect_client()
            .map_err(error::nvme_to_em)?;

        let alloc = self
            .dma_alloc
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| error::not_initialized("DMA allocator not set"))?;

        let block_size = {
            let state = self.state.read();
            match state.as_ref() {
                Some(s) => s.format_params.block_size,
                None => {
                    let sb = self.superblock.lock().unwrap();
                    sb.as_ref()
                        .map(|s| s.block_size)
                        .unwrap_or(4096)
                }
            }
        };

        Ok(BlockDeviceClient::new(channels, alloc, block_size))
    }

    fn with_state<F, R>(&self, f: F) -> Result<R, ExtentManagerError>
    where
        F: FnOnce(&ManagerState) -> Result<R, ExtentManagerError>,
    {
        let state = self.state.read();
        let s = state
            .as_ref()
            .ok_or_else(|| error::not_initialized("component not initialized"))?;
        f(s)
    }

    fn with_state_mut<F, R>(&self, f: F) -> Result<R, ExtentManagerError>
    where
        F: FnOnce(&mut ManagerState) -> Result<R, ExtentManagerError>,
    {
        let mut state = self.state.write();
        let s = state
            .as_mut()
            .ok_or_else(|| error::not_initialized("component not initialized"))?;
        f(s)
    }

    fn log_info(&self, msg: &str) {
        if let Ok(logger) = self.logger.get() {
            logger.info(msg);
        }
    }

    fn log_error(&self, msg: &str) {
        if let Ok(logger) = self.logger.get() {
            logger.error(msg);
        }
    }

    fn log_warn(&self, msg: &str) {
        if let Ok(logger) = self.logger.get() {
            logger.warn(msg);
        }
    }

    fn start_background_checkpoint(self: &Arc<Self>) {
        let this = Arc::clone(self);
        let shutdown = Arc::clone(&self.shutdown);

        let handle = std::thread::spawn(move || {
            loop {
                let interval_ms = this.checkpoint_interval_ms.load(Ordering::Relaxed);
                let duration = std::time::Duration::from_millis(interval_ms);
                std::thread::sleep(duration);

                if shutdown.load(Ordering::Relaxed) {
                    break;
                }

                if let Err(e) = this.checkpoint() {
                    this.log_error(&format!("background checkpoint failed: {e}"));
                }
            }
        });

        *self.checkpoint_thread.lock().unwrap() = Some(handle);
    }
}

impl Drop for MetadataManagerV2 {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(handle) = self.checkpoint_thread.lock().unwrap().take() {
            let _ = handle.join();
        }
    }
}

fn mark_chain_allocated(
    client: &BlockDeviceClient,
    head_lba: u64,
    chunk_size: u32,
    block_size: u32,
    buddy: &mut BuddyAllocator,
) {
    let mut current_lba = head_lba;
    while current_lba != 0 {
        let buddy_offset = current_lba * block_size as u64 - SUPERBLOCK_SIZE as u64;
        buddy.mark_allocated(buddy_offset, chunk_size as u64);
        let next = client
            .read_blocks(current_lba, chunk_size as usize)
            .ok()
            .and_then(|raw| {
                let magic = u32::from_le_bytes(raw[0..4].try_into().ok()?);
                if magic != checkpoint::CHUNK_MAGIC {
                    return None;
                }
                Some(u64::from_le_bytes(raw[20..28].try_into().ok()?))
            })
            .unwrap_or(0);
        current_lba = next;
    }
}

impl IExtentManagerV2 for MetadataManagerV2 {
    fn set_dma_alloc(&self, alloc: DmaAllocFn) {
        *self.dma_alloc.lock().unwrap() = Some(alloc);
    }

    fn format(&self, params: FormatParams) -> Result<(), ExtentManagerError> {
        if params.block_size == 0 {
            return Err(error::corrupt_metadata("block_size must be > 0"));
        }
        if params.slab_size % params.block_size != 0 {
            return Err(error::corrupt_metadata(
                "slab_size must be a multiple of block_size",
            ));
        }
        if params.max_element_size > params.slab_size {
            return Err(error::corrupt_metadata(
                "max_element_size must be <= slab_size",
            ));
        }
        if params.chunk_size % params.block_size != 0 {
            return Err(error::corrupt_metadata(
                "chunk_size must be a multiple of block_size",
            ));
        }

        let client = self.get_client()?;

        let bd = self
            .block_device
            .get()
            .map_err(|_| error::not_initialized("block device not connected"))?;
        let disk_size = bd.num_sectors(1).map_err(error::nvme_to_em)?
            * bd.sector_size(1).map_err(error::nvme_to_em)? as u64;

        let usable_size = disk_size - SUPERBLOCK_SIZE as u64;
        let buddy = BuddyAllocator::new(usable_size, params.block_size);

        let sb = Superblock::new(
            disk_size,
            params.block_size,
            params.slab_size,
            params.max_element_size,
            params.chunk_size,
        );

        let sb_data = sb.serialize();
        client.write_blocks(0, &sb_data)?;

        let state = ManagerState::new(buddy, params, 0);
        *self.state.write() = Some(state);
        *self.superblock.lock().unwrap() = Some(sb);

        self.log_info("format complete");

        Ok(())
    }

    fn initialize(&self) -> Result<(), ExtentManagerError> {
        self.log_info("recovery_start");

        let client = self.get_client()?;
        let (sb, index, slab_descriptors) = recovery::recover(&client, self)?;

        let usable_size = sb.disk_size - SUPERBLOCK_SIZE as u64;
        let mut buddy = BuddyAllocator::new(usable_size, sb.block_size);

        let mut slabs = Vec::new();
        let mut size_classes = crate::slab::SizeClassManager::new();

        for desc in &slab_descriptors {
            let slab = crate::slab::Slab::new(
                desc.start_offset,
                desc.slab_size,
                desc.element_size,
            );
            let slab_idx = slabs.len();
            size_classes.add_slab(desc.element_size, slab_idx);
            let buddy_offset = desc.start_offset - SUPERBLOCK_SIZE as u64;
            buddy.mark_allocated(buddy_offset, desc.slab_size as u64);
            slabs.push(slab);
        }

        let block_size = sb.block_size;
        for extent in index.values() {
            let aligned_size =
                (extent.size + block_size - 1) / block_size * block_size;
            for slab in slabs.iter_mut() {
                if slab.element_size == aligned_size {
                    if let Some(slot_idx) = slab.slot_for_offset(extent.offset) {
                        slab.mark_slot_allocated(slot_idx);
                        break;
                    }
                }
            }
        }

        for &chain_lba in &[sb.current_index_lba, sb.previous_index_lba] {
            if chain_lba != 0 {
                mark_chain_allocated(
                    &client,
                    chain_lba,
                    sb.chunk_size,
                    sb.block_size,
                    &mut buddy,
                );
            }
        }

        let format_params = FormatParams {
            slab_size: sb.slab_size,
            max_element_size: sb.max_element_size,
            chunk_size: sb.chunk_size,
            block_size: sb.block_size,
        };

        let mut state = ManagerState::new(buddy, format_params, sb.checkpoint_seq);
        state.index = index;
        state.slabs = slabs;
        state.size_classes = size_classes;

        *self.state.write() = Some(state);
        *self.superblock.lock().unwrap() = Some(sb);

        self.log_info("recovery_complete");

        Ok(())
    }

    fn reserve_extent(
        &self,
        key: ExtentKey,
        size: u32,
    ) -> Result<WriteHandle, ExtentManagerError> {
        let (slab_idx, slot_idx, offset) = self.with_state_mut(|state| {
            state.alloc_extent(size)
        })?;

        let aligned_size = {
            let state = self.state.read();
            let s = state.as_ref().unwrap();
            (size + s.format_params.block_size - 1)
                / s.format_params.block_size
                * s.format_params.block_size
        };

        let state_ref = Arc::clone(&self.state);
        let publish_state = Arc::clone(&self.state);

        let publish_fn = Box::new(move || {
            let mut state = publish_state.write();
            let s = state.as_mut().unwrap();

            if s.index.contains_key(&key) {
                s.free_slot(slab_idx, slot_idx);
                return Err(error::duplicate_key(key));
            }

            let extent = Extent {
                key,
                offset,
                size: aligned_size,
            };

            s.index.insert(key, extent.clone());
            s.dirty = true;
            Ok(extent)
        });

        let abort_fn = Box::new(move || {
            let mut state = state_ref.write();
            if let Some(s) = state.as_mut() {
                s.free_slot(slab_idx, slot_idx);
            }
        });

        Ok(WriteHandle::new(key, offset, aligned_size, publish_fn, abort_fn))
    }

    fn lookup_extent(&self, key: ExtentKey) -> Result<Extent, ExtentManagerError> {
        self.with_state(|state| {
            state
                .index
                .get(&key)
                .cloned()
                .ok_or_else(|| error::key_not_found(key))
        })
    }

    fn get_extents(&self) -> Vec<Extent> {
        let state = self.state.read();
        match state.as_ref() {
            Some(s) => s.index.values().cloned().collect(),
            None => Vec::new(),
        }
    }

    fn for_each_extent(&self, cb: &mut dyn FnMut(&Extent)) {
        let state = self.state.read();
        if let Some(s) = state.as_ref() {
            for extent in s.index.values() {
                cb(extent);
            }
        }
    }

    fn remove_extent(&self, key: ExtentKey) -> Result<(), ExtentManagerError> {
        self.with_state_mut(|state| {
            let (slab_idx, slot_idx) = state.remove_extent(key)?;
            state.free_slot(slab_idx, slot_idx);
            Ok(())
        })
    }

    fn checkpoint(&self) -> Result<(), ExtentManagerError> {
        let is_dirty = self.with_state(|s| Ok(s.dirty))?;
        if !is_dirty {
            return Ok(());
        }

        self.log_info("checkpoint_start");

        let client = self.get_client()?;

        {
            let mut sb_lock = self.superblock.lock().unwrap();
            let sb = sb_lock
                .as_mut()
                .ok_or_else(|| error::not_initialized("no superblock"))?;

            checkpoint::write_checkpoint(&client, &self.state, sb)?;

            let sb_data = sb.serialize();
            client.write_blocks(0, &sb_data)?;
        }

        self.with_state_mut(|s| {
            s.dirty = false;
            Ok(())
        })?;

        self.log_info("checkpoint_complete");

        Ok(())
    }
}
