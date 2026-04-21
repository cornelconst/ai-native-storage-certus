use std::collections::HashMap;

use interfaces::{Extent, ExtentKey, ExtentManagerError, FormatParams};

use crate::buddy::BuddyAllocator;
use crate::error;
use crate::slab::{SizeClassManager, Slab};
use crate::superblock::Superblock;

pub(crate) struct RegionState {
    pub region_index: usize,
    pub index: HashMap<ExtentKey, Extent>,
    pub slabs: Vec<Slab>,
    pub size_classes: SizeClassManager,
    pub buddy: BuddyAllocator,
    pub dirty: bool,
}

pub(crate) struct SharedState {
    pub format_params: FormatParams,
    pub checkpoint_seq: u64,
    pub disk_size: u64,
    pub superblock: Superblock,
}

impl RegionState {
    pub fn new(region_index: usize, buddy: BuddyAllocator) -> Self {
        Self {
            region_index,
            index: HashMap::new(),
            slabs: Vec::new(),
            size_classes: SizeClassManager::new(),
            buddy,
            dirty: false,
        }
    }

    fn align_to_block_size(&self, size: u32, block_size: u32) -> u32 {
        (size + block_size - 1) / block_size * block_size
    }

    pub fn alloc_extent(
        &mut self,
        size: u32,
        format_params: &FormatParams,
    ) -> Result<(usize, usize, u64), ExtentManagerError> {
        let element_size = self.align_to_block_size(size, format_params.block_size);

        for &slab_idx in self.size_classes.get_slabs(element_size) {
            if let Some((slot_idx, offset)) = self.slabs[slab_idx].alloc_slot() {
                return Ok((slab_idx, slot_idx, offset));
            }
        }

        let disk_offset = self
            .buddy
            .alloc(format_params.slab_size as u64)
            .ok_or_else(error::out_of_space)?;

        let slab = Slab::new(disk_offset, format_params.slab_size, element_size);
        let slab_idx = self.slabs.len();
        self.slabs.push(slab);
        self.size_classes.add_slab(element_size, slab_idx);

        let (slot_idx, offset) = self.slabs[slab_idx]
            .alloc_slot()
            .expect("freshly created slab must have free slot");

        Ok((slab_idx, slot_idx, offset))
    }

    pub fn free_slot(&mut self, slab_idx: usize, slot_idx: usize, slab_size: u32) {
        let slab = &mut self.slabs[slab_idx];
        slab.free_slot(slot_idx);

        if slab.is_empty() {
            let disk_offset = slab.start_offset;
            let element_size = slab.element_size;

            self.buddy.free(disk_offset, slab_size as u64);
            self.size_classes.remove_slab(element_size, slab_idx);

            if slab_idx < self.slabs.len() - 1 {
                let last_idx = self.slabs.len() - 1;
                let last_element_size = self.slabs[last_idx].element_size;
                self.size_classes.update_slab_index(last_element_size, last_idx, slab_idx);
            }
            self.slabs.swap_remove(slab_idx);
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
        block_size: u32,
    ) -> Result<(usize, usize), ExtentManagerError> {
        let extent = self
            .index
            .remove(&key)
            .ok_or_else(|| error::key_not_found(key))?;

        let aligned_size = self.align_to_block_size(extent.size, block_size);
        for (slab_idx, slab) in self.slabs.iter().enumerate() {
            if slab.element_size == aligned_size {
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
