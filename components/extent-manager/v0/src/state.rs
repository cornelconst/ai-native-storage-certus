use std::collections::HashMap;

use crate::bitmap::AllocationBitmap;
use crate::metadata::{ExtentMetadata, BLOCK_SIZE};

pub(crate) const MAX_SLABS: usize = 256;
const BITS_PER_BLOCK: usize = BLOCK_SIZE * 8;

pub(crate) fn compute_slab_layout(slab_size_blocks: u32) -> (u32, u32) {
    let total = slab_size_blocks as usize;
    if total < 2 {
        return (0, 0);
    }
    let mut num_slots = total - 1;
    loop {
        let bitmap_blocks = num_slots.div_ceil(BITS_PER_BLOCK);
        let available = total - bitmap_blocks;
        if available <= num_slots {
            num_slots = available;
            let bitmap_blocks = num_slots.div_ceil(BITS_PER_BLOCK);
            return (bitmap_blocks as u32, num_slots as u32);
        }
        num_slots = available;
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub(crate) struct SlabDescriptor {
    pub slab_index: usize,
    pub size_class: u32,
    pub start_lba: u64,
    pub bitmap_start_lba: u64,
    pub record_start_lba: u64,
    pub bitmap_blocks: u32,
    pub num_slots: u32,
    pub bitmap: AllocationBitmap,
}

impl SlabDescriptor {
    pub fn new(slab_index: usize, size_class: u32, start_lba: u64, slab_size_blocks: u32) -> Self {
        let (bitmap_blocks, num_slots) = compute_slab_layout(slab_size_blocks);
        let bitmap_start_lba = start_lba;
        let record_start_lba = start_lba + bitmap_blocks as u64;
        SlabDescriptor {
            slab_index,
            size_class,
            start_lba,
            bitmap_start_lba,
            record_start_lba,
            bitmap_blocks,
            num_slots,
            bitmap: AllocationBitmap::new(num_slots),
        }
    }
}

#[derive(Debug)]
pub(crate) struct PoolState {
    pub total_blocks: u64,
    pub slab_size_blocks: u32,
    pub slabs: Vec<SlabDescriptor>,
    pub size_class_slabs: HashMap<u32, Vec<usize>>,
    pub next_free_lba: u64,
}

impl PoolState {
    pub fn new(total_blocks: u64, slab_size_blocks: u32) -> Self {
        PoolState {
            total_blocks,
            slab_size_blocks,
            slabs: Vec::new(),
            size_class_slabs: HashMap::new(),
            next_free_lba: 0,
        }
    }

    pub fn find_free_slot(&self, size_class: u32) -> Option<(usize, u32)> {
        let slab_indices = self.size_class_slabs.get(&size_class)?;
        for &idx in slab_indices {
            let slab = &self.slabs[idx];
            if let Some(slot) = slab.bitmap.find_free() {
                return Some((idx, slot));
            }
        }
        None
    }

    pub fn can_allocate_slab(&self) -> bool {
        self.slabs.len() < MAX_SLABS
            && self.next_free_lba + self.slab_size_blocks as u64 <= self.total_blocks
    }

    pub fn add_slab_descriptor(&mut self, size_class: u32, start_lba: u64) -> usize {
        let idx = self.slabs.len();
        let slab = SlabDescriptor::new(idx, size_class, start_lba, self.slab_size_blocks);
        self.slabs.push(slab);
        self.size_class_slabs
            .entry(size_class)
            .or_default()
            .push(idx);
        self.next_free_lba = start_lba + self.slab_size_blocks as u64;
        idx
    }
}

#[derive(Debug, Default)]
pub(crate) struct ExtentManagerState {
    pub index: HashMap<interfaces::ExtentKey, ExtentMetadata>,
    pub pool: Option<PoolState>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_slab_layout_1gib() {
        let slab_blocks = 262144u32;
        let (bitmap_blocks, num_slots) = compute_slab_layout(slab_blocks);
        assert_eq!(bitmap_blocks + num_slots as u32, slab_blocks);
        assert!(num_slots > 0);
        assert!(bitmap_blocks > 0);
        assert!(num_slots.div_ceil(BITS_PER_BLOCK as u32) <= bitmap_blocks);
    }

    #[test]
    fn compute_slab_layout_minimum() {
        let (bitmap_blocks, num_slots) = compute_slab_layout(2);
        assert_eq!(bitmap_blocks, 1);
        assert_eq!(num_slots, 1);
    }

    #[test]
    fn compute_slab_layout_too_small() {
        let (bitmap_blocks, num_slots) = compute_slab_layout(1);
        assert_eq!(bitmap_blocks, 0);
        assert_eq!(num_slots, 0);
    }

    #[test]
    fn pool_state_find_free_slot() {
        let mut pool = PoolState::new(1000, 10);
        pool.add_slab_descriptor(131072, 0);
        let result = pool.find_free_slot(131072);
        assert!(result.is_some());
        assert!(pool.find_free_slot(999).is_none());
    }

    #[test]
    fn pool_state_can_allocate_slab() {
        let pool = PoolState::new(20, 10);
        assert!(pool.can_allocate_slab());
        let pool2 = PoolState::new(5, 10);
        assert!(!pool2.can_allocate_slab());
    }
}
