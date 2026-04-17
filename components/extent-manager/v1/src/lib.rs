mod bitmap;
mod block_device;
mod error;
mod metadata;
mod state;

#[cfg(any(test, feature = "testing"))]
pub mod test_support;

use std::sync::{Mutex, RwLock};

use interfaces::{
    DmaAllocFn, Extent, ExtentManagerError, IBlockDevice, IExtentManager, ILogger, NvmeBlockError,
};

use component_macros::define_component;

use crate::block_device::BlockDeviceClient;
use crate::metadata::{ExtentMetadata, OnDiskExtentRecord, BLOCK_SIZE};
use crate::state::{ExtentManagerState, PoolState};

define_component! {
    pub ExtentManagerComponentV1 {
        version: "0.1.0",
        provides: [IExtentManager],
        receptacles: {
            block_device: IBlockDevice,
            logger: ILogger,
        },
        fields: {
            state: RwLock<ExtentManagerState>,
            dma_alloc: Mutex<Option<DmaAllocFn>>,
        },
    }
}

impl ExtentManagerComponentV1 {
    #[allow(dead_code)]
    pub(crate) fn new_inner() -> std::sync::Arc<Self> {
        ExtentManagerComponentV1::new_default()
    }

    fn log_info(&self, msg: &str) {
        if let Ok(log) = self.logger.get() {
            log.info(msg);
        }
    }

    fn log_debug(&self, msg: &str) {
        if let Ok(log) = self.logger.get() {
            log.debug(msg);
        }
    }

    fn get_client(&self) -> Result<BlockDeviceClient, NvmeBlockError> {
        let bd = self
            .block_device
            .get()
            .map_err(|_| NvmeBlockError::NotInitialized("block device not connected".into()))?;
        let channels = bd.connect_client()?;
        let alloc = self
            .dma_alloc
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| NvmeBlockError::NotInitialized("DMA allocator not set".into()))?;
        Ok(BlockDeviceClient::new(channels, alloc))
    }

    fn allocate_slab(
        &self,
        pool: &mut PoolState,
        client: &BlockDeviceClient,
        extent_size: u32,
    ) -> Result<usize, ExtentManagerError> {
        if !pool.can_allocate_slab() {
            return Err(error::out_of_space());
        }

        let start_lba = pool.next_free_lba;
        let slab_size = pool.slab_size_blocks;

        let zero = metadata::zero_block();
        for offset in 0..slab_size as u64 {
            client
                .write_block(1, start_lba + offset, &zero)
                .map_err(error::nvme_to_em)?;
        }

        let slab_idx = pool.add_slab_descriptor(extent_size, start_lba);
        self.log_debug(&format!(
            "slab allocated: index={slab_idx}, extent_size={extent_size}, start_lba={start_lba}"
        ));

        let bitmap_blocks = pool.slabs[slab_idx].bitmap.serialize_to_blocks();
        for (i, block) in bitmap_blocks.iter().enumerate() {
            client
                .write_block(1, pool.slabs[slab_idx].bitmap_start_lba + i as u64, block)
                .map_err(error::nvme_to_em)?;
        }

        Ok(slab_idx)
    }
}

impl IExtentManager for ExtentManagerComponentV1 {
    fn set_dma_alloc(&self, alloc: DmaAllocFn) {
        *self.dma_alloc.lock().unwrap() = Some(alloc);
    }

    fn initialize(
        &self,
        total_size_bytes: u64, // e.g. capacity of the data SSD
        slab_size_bytes: u32,  // size of new slab allocations, e.g., 1GiB
    ) -> Result<(), ExtentManagerError> {
        self.log_info(&format!(
            "initializing: total_size={total_size_bytes}B, slab_size={slab_size_bytes}B"
        ));

        if total_size_bytes == 0 {
            return Err(ExtentManagerError::IoError(
                "total_size_bytes must be > 0".into(),
            ));
        }
        if slab_size_bytes < (BLOCK_SIZE as u32 * 2) {
            return Err(ExtentManagerError::IoError(format!(
                "slab_size_bytes must be >= {} (2 blocks)",
                BLOCK_SIZE * 2,
            )));
        }
        if slab_size_bytes as u64 % BLOCK_SIZE as u64 != 0 {
            return Err(ExtentManagerError::IoError(
                "slab_size_bytes must be a multiple of block size (4096)".into(),
            ));
        }

        let total_blocks = total_size_bytes / BLOCK_SIZE as u64;
        let slab_size_blocks = slab_size_bytes / BLOCK_SIZE as u32;

        let pool = PoolState::new(total_blocks, slab_size_blocks);
        self.log_debug(&format!(
            "pool initialized: total_blocks={total_blocks}, slab_size_blocks={slab_size_blocks}"
        ));

        let mut state = self.state.write().unwrap();
        state.pool = Some(pool);
        Ok(())
    }

    fn create_extent(
        &self,
        key: u64,
        extent_size: u32,
        filename: &str,
        data_crc: u32,
    ) -> Result<Extent, ExtentManagerError> {
        let mut state = self.state.write().unwrap();

        if state.pool.is_none() {
            return Err(error::not_initialized("pool not initialized"));
        }

        if state.index.contains_key(&key) {
            return Err(error::duplicate_key(key));
        }

        let pool = state.pool.as_mut().unwrap();

        let (slab_idx, slot) = match pool.find_free_slot(extent_size) {
            Some(result) => result,
            None => {
                let client = self.get_client().map_err(error::nvme_to_em)?;
                let new_slab_idx = self.allocate_slab(pool, &client, extent_size)?;
                let slot = pool.slabs[new_slab_idx]
                    .bitmap
                    .find_free()
                    .ok_or_else(error::out_of_space)?;
                (new_slab_idx, slot)
            }
        };

        let slab = &pool.slabs[slab_idx];
        let offset_lba = slab.record_start_lba + slot as u64;

        let fname = if filename.is_empty() {
            None
        } else {
            Some(filename.to_string())
        };

        let meta = ExtentMetadata {
            key,
            size_class: extent_size,
            offset_lba,
            filename: fname,
            data_crc: Some(data_crc),
            slab_index: slab_idx,
        };

        let client = self.get_client().map_err(error::nvme_to_em)?;

        let record = OnDiskExtentRecord::from_metadata(&meta);
        client
            .write_block(1, offset_lba, &record.data)
            .map_err(error::nvme_to_em)?;

        pool.slabs[slab_idx].bitmap.set(slot);
        let bitmap_blocks = pool.slabs[slab_idx].bitmap.serialize_to_blocks();
        let bitmap_block_idx = (slot as usize) / (BLOCK_SIZE * 8);
        if bitmap_block_idx < bitmap_blocks.len() {
            let bitmap_lba = pool.slabs[slab_idx].bitmap_start_lba + bitmap_block_idx as u64;
            client
                .write_block(1, bitmap_lba, &bitmap_blocks[bitmap_block_idx])
                .map_err(error::nvme_to_em)?;
        }

        let extent = meta.to_extent();
        self.log_debug(&format!(
            "extent created: key={key}, size={extent_size}, lba={offset_lba}"
        ));
        state.index.insert(key, meta);

        Ok(extent)
    }

    fn remove_extent(&self, key: u64) -> Result<(), ExtentManagerError> {
        let mut state = self.state.write().unwrap();

        let meta = state
            .index
            .get(&key)
            .ok_or_else(|| error::key_not_found(key))?
            .clone();

        let pool = state
            .pool
            .as_mut()
            .ok_or_else(|| error::not_initialized("pool not initialized"))?;

        let slab_idx = meta.slab_index;
        let slab = &pool.slabs[slab_idx];
        let slot = (meta.offset_lba - slab.record_start_lba) as u32;

        let client = self.get_client().map_err(error::nvme_to_em)?;

        pool.slabs[slab_idx].bitmap.clear(slot);
        let bitmap_blocks = pool.slabs[slab_idx].bitmap.serialize_to_blocks();
        let bitmap_block_idx = (slot as usize) / (BLOCK_SIZE * 8);
        if bitmap_block_idx < bitmap_blocks.len() {
            let bitmap_lba = pool.slabs[slab_idx].bitmap_start_lba + bitmap_block_idx as u64;
            client
                .write_block(1, bitmap_lba, &bitmap_blocks[bitmap_block_idx])
                .map_err(error::nvme_to_em)?;
        }

        let zero = metadata::zero_block();
        client
            .write_block(1, meta.offset_lba, &zero)
            .map_err(error::nvme_to_em)?;

        self.log_debug(&format!("extent removed: key={key}"));
        state.index.remove(&key);
        Ok(())
    }

    fn lookup_extent(&self, key: u64) -> Result<Extent, ExtentManagerError> {
        let state = self.state.read().unwrap();

        let meta = state
            .index
            .get(&key)
            .ok_or_else(|| error::key_not_found(key))?;

        Ok(meta.to_extent())
    }

    fn get_extents(&self) -> Vec<Extent> {
        let state = self.state.read().unwrap();
        state.index.values().map(|m| m.to_extent()).collect()
    }
}
