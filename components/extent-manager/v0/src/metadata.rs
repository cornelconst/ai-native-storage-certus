use crc32fast::Hasher;

pub(crate) const BLOCK_SIZE: usize = 4096;
pub(crate) const RECORD_CRC_OFFSET: usize = BLOCK_SIZE - 4;

#[derive(Debug, Clone)]
pub(crate) struct ExtentMetadata {
    pub key: interfaces::ExtentKey,
    pub size_class: u32,
    pub offset_lba: u64,
    pub slab_index: usize,
}

impl ExtentMetadata {
    pub fn to_extent(&self) -> interfaces::Extent {
        interfaces::Extent {
            key: self.key,
            size: self.size_class,
            offset: self.offset_lba,
        }
    }
}

#[derive(Debug)]
pub(crate) struct OnDiskExtentRecord {
    pub data: [u8; BLOCK_SIZE],
}

impl OnDiskExtentRecord {
    pub fn from_metadata(meta: &ExtentMetadata) -> Self {
        let mut data = [0u8; BLOCK_SIZE];

        data[0..8].copy_from_slice(&meta.key.to_le_bytes());
        data[8..12].copy_from_slice(&meta.size_class.to_le_bytes());
        data[12..20].copy_from_slice(&meta.offset_lba.to_le_bytes());

        let crc = compute_record_crc(&data);
        data[RECORD_CRC_OFFSET..BLOCK_SIZE].copy_from_slice(&crc.to_le_bytes());

        OnDiskExtentRecord { data }
    }

    #[allow(dead_code)]
    pub fn to_metadata(&self) -> Option<ExtentMetadata> {
        let key = u64::from_le_bytes(self.data[0..8].try_into().ok()?);
        if key == 0 {
            return None;
        }

        let size_class = u32::from_le_bytes(self.data[8..12].try_into().ok()?);
        let offset_lba = u64::from_le_bytes(self.data[12..20].try_into().ok()?);

        Some(ExtentMetadata {
            key,
            size_class,
            offset_lba,
            slab_index: 0,
        })
    }

    #[allow(dead_code)]
    pub fn verify_crc(&self) -> bool {
        let stored = u32::from_le_bytes(
            self.data[RECORD_CRC_OFFSET..BLOCK_SIZE]
                .try_into()
                .unwrap_or([0; 4]),
        );
        let computed = compute_record_crc(&self.data);
        stored == computed
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        u64::from_le_bytes(self.data[0..8].try_into().unwrap_or([0; 8])) == 0
    }
}

pub(crate) fn compute_record_crc(data: &[u8; BLOCK_SIZE]) -> u32 {
    let mut hasher = Hasher::new();
    hasher.update(&data[..RECORD_CRC_OFFSET]);
    hasher.finalize()
}

pub(crate) fn zero_block() -> [u8; BLOCK_SIZE] {
    [0u8; BLOCK_SIZE]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_roundtrip() {
        let meta = ExtentMetadata {
            key: 42,
            size_class: 131072,
            offset_lba: 100,
            slab_index: 0,
        };
        let record = OnDiskExtentRecord::from_metadata(&meta);
        assert!(record.verify_crc());
        let recovered = record.to_metadata().unwrap();
        assert_eq!(recovered.key, 42);
        assert_eq!(recovered.size_class, 131072);
        assert_eq!(recovered.offset_lba, 100);
    }

    #[test]
    fn empty_record_returns_none() {
        let record = OnDiskExtentRecord {
            data: [0u8; BLOCK_SIZE],
        };
        assert!(record.is_empty());
        assert!(record.to_metadata().is_none());
    }

    #[test]
    fn corrupt_crc_detected() {
        let meta = ExtentMetadata {
            key: 1,
            size_class: 131072,
            offset_lba: 0,
            slab_index: 0,
        };
        let mut record = OnDiskExtentRecord::from_metadata(&meta);
        record.data[0] ^= 0xFF;
        assert!(!record.verify_crc());
    }

    #[test]
    fn metadata_to_extent() {
        let meta = ExtentMetadata {
            key: 123,
            size_class: 262144,
            offset_lba: 999,
            slab_index: 0,
        };
        let extent = meta.to_extent();
        assert_eq!(extent.key, 123);
        assert_eq!(extent.size, 262144);
        assert_eq!(extent.offset, 999);
    }
}
