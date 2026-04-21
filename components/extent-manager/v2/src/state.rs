use std::collections::HashMap;

use interfaces::{Extent, ExtentKey, ExtentManagerError, FormatParams};

use crate::buddy::BuddyAllocator;
use crate::error;
use crate::slab::{SizeClassManager, Slab};
use crate::superblock::SUPERBLOCK_SIZE;

pub(crate) struct ManagerState {
    pub index: HashMap<ExtentKey, Extent>,
    pub slabs: Vec<Slab>,
    pub size_classes: SizeClassManager,
    pub buddy: BuddyAllocator,
    pub format_params: FormatParams,
    pub dirty: bool,
    pub checkpoint_seq: u64,
}

impl ManagerState {
    pub fn new(
        buddy: BuddyAllocator,
        format_params: FormatParams,
        checkpoint_seq: u64,
    ) -> Self {
        Self {
            index: HashMap::new(),
            slabs: Vec::new(),
            size_classes: SizeClassManager::new(),
            buddy,
            format_params,
            dirty: false,
            checkpoint_seq,
        }
    }

    pub fn new_for_testing(
        total_size: u64,
        block_size: u32,
        slab_size: u32,
        max_element_size: u32,
    ) -> Self {
        let buddy = BuddyAllocator::new(total_size, block_size);
        let format_params = FormatParams {
            slab_size,
            max_element_size,
            chunk_size: block_size,
            block_size,
        };
        Self::new(buddy, format_params, 0)
    }

    fn align_to_block_size(&self, size: u32) -> u32 {
        let bs = self.format_params.block_size;
        (size + bs - 1) / bs * bs
    }

    pub fn alloc_extent(
        &mut self,
        size: u32,
    ) -> Result<(usize, usize, u64), ExtentManagerError> {
        let element_size = self.align_to_block_size(size);

        for &slab_idx in self.size_classes.get_slabs(element_size) {
            if let Some((slot_idx, offset)) = self.slabs[slab_idx].alloc_slot() {
                return Ok((slab_idx, slot_idx, offset));
            }
        }

        let buddy_offset = self
            .buddy
            .alloc(self.format_params.slab_size as u64)
            .ok_or_else(error::out_of_space)?;

        let disk_offset = buddy_offset + SUPERBLOCK_SIZE as u64;
        let slab = Slab::new(disk_offset, self.format_params.slab_size, element_size);
        let slab_idx = self.slabs.len();
        self.slabs.push(slab);
        self.size_classes.add_slab(element_size, slab_idx);

        let (slot_idx, offset) = self.slabs[slab_idx]
            .alloc_slot()
            .expect("freshly created slab must have free slot");

        Ok((slab_idx, slot_idx, offset))
    }

    pub fn free_slot(&mut self, slab_idx: usize, slot_idx: usize) {
        let slab = &mut self.slabs[slab_idx];
        slab.free_slot(slot_idx);

        if slab.is_empty() {
            let disk_offset = slab.start_offset;
            let slab_size = slab.slab_size;
            let element_size = slab.element_size;

            self.size_classes.remove_slab(element_size, slab_idx);
            let buddy_offset = disk_offset - SUPERBLOCK_SIZE as u64;
            self.buddy.free(buddy_offset, slab_size as u64);
        }
    }

    pub fn insert_extent(
        &mut self,
        key: ExtentKey,
        extent: Extent,
    ) -> Result<(), ExtentManagerError> {
        if self.index.contains_key(&key) {
            return Err(error::duplicate_key(key));
        }
        self.index.insert(key, extent);
        self.dirty = true;
        Ok(())
    }

    pub fn remove_extent(
        &mut self,
        key: ExtentKey,
    ) -> Result<(usize, usize), ExtentManagerError> {
        let extent = self
            .index
            .remove(&key)
            .ok_or_else(|| error::key_not_found(key))?;

        for (slab_idx, slab) in self.slabs.iter().enumerate() {
            if slab.element_size == self.align_to_block_size(extent.size) {
                if let Some(slot_idx) = slab.slot_for_offset(extent.offset) {
                    self.dirty = true;
                    return Ok((slab_idx, slot_idx));
                }
            }
        }

        self.dirty = true;
        Err(error::corrupt_metadata(&format!(
            "extent for key {} at offset {} not found in any slab",
            key, extent.offset
        )))
    }
}
