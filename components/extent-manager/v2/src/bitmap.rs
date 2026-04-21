pub(crate) struct AllocationBitmap {
    words: Vec<u64>,
    num_slots: u32,
}

impl AllocationBitmap {
    pub fn new(num_slots: u32) -> Self {
        let num_words = (num_slots as usize + 63) / 64;
        Self {
            words: vec![0u64; num_words],
            num_slots,
        }
    }

    pub fn set(&mut self, idx: usize) {
        debug_assert!((idx as u32) < self.num_slots);
        let word = idx / 64;
        let bit = idx % 64;
        self.words[word] |= 1u64 << bit;
    }

    pub fn clear(&mut self, idx: usize) {
        debug_assert!((idx as u32) < self.num_slots);
        let word = idx / 64;
        let bit = idx % 64;
        self.words[word] &= !(1u64 << bit);
    }

    pub fn is_set(&self, idx: usize) -> bool {
        debug_assert!((idx as u32) < self.num_slots);
        let word = idx / 64;
        let bit = idx % 64;
        (self.words[word] >> bit) & 1 == 1
    }

    pub fn find_free_from(&self, start: usize) -> Option<usize> {
        let n = self.num_slots as usize;
        for i in 0..n {
            let idx = (start + i) % n;
            if !self.is_set(idx) {
                return Some(idx);
            }
        }
        None
    }

    pub fn is_all_free(&self) -> bool {
        let full_words = self.num_slots as usize / 64;
        for w in &self.words[..full_words] {
            if *w != 0 {
                return false;
            }
        }
        let remainder = self.num_slots as usize % 64;
        if remainder > 0 {
            let mask = (1u64 << remainder) - 1;
            if self.words[full_words] & mask != 0 {
                return false;
            }
        }
        true
    }

    pub fn count_set(&self) -> usize {
        let full_words = self.num_slots as usize / 64;
        let mut count: u32 = 0;
        for w in &self.words[..full_words] {
            count += w.count_ones();
        }
        let remainder = self.num_slots as usize % 64;
        if remainder > 0 {
            let mask = (1u64 << remainder) - 1;
            count += (self.words[full_words] & mask).count_ones();
        }
        count as usize
    }

    pub fn num_slots(&self) -> u32 {
        self.num_slots
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_clear_round_trip() {
        let mut bm = AllocationBitmap::new(128);
        assert!(!bm.is_set(0));
        bm.set(0);
        assert!(bm.is_set(0));
        bm.clear(0);
        assert!(!bm.is_set(0));
    }

    #[test]
    fn find_free_from_wraps() {
        let mut bm = AllocationBitmap::new(4);
        bm.set(1);
        bm.set(2);
        bm.set(3);
        assert_eq!(bm.find_free_from(1), Some(0));
    }

    #[test]
    fn find_free_returns_none_when_full() {
        let mut bm = AllocationBitmap::new(64);
        for i in 0..64 {
            bm.set(i);
        }
        assert_eq!(bm.find_free_from(0), None);
    }

    #[test]
    fn is_all_free() {
        let mut bm = AllocationBitmap::new(100);
        assert!(bm.is_all_free());
        bm.set(50);
        assert!(!bm.is_all_free());
        bm.clear(50);
        assert!(bm.is_all_free());
    }

    #[test]
    fn count_set_correct() {
        let mut bm = AllocationBitmap::new(130);
        bm.set(0);
        bm.set(63);
        bm.set(64);
        bm.set(129);
        assert_eq!(bm.count_set(), 4);
    }

    #[test]
    fn non_multiple_of_64() {
        let mut bm = AllocationBitmap::new(3);
        bm.set(0);
        bm.set(1);
        bm.set(2);
        assert_eq!(bm.count_set(), 3);
        assert_eq!(bm.find_free_from(0), None);
        bm.clear(1);
        assert_eq!(bm.find_free_from(0), Some(1));
        assert!(!bm.is_all_free());
    }
}
