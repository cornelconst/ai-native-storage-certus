use crate::error;
use interfaces::ExtentManagerError;

pub const SUPERBLOCK_SIZE: usize = 4096;
pub const SUPERBLOCK_MAGIC: u64 = 0x4345_5254_5553_5632; // "CERTUSV2"
pub const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone)]
pub struct Superblock {
    pub magic: u64,
    pub version: u32,
    pub disk_size: u64,
    pub current_index_lba: u64,
    pub previous_index_lba: u64,
    pub block_size: u32,
    pub slab_size: u32,
    pub max_element_size: u32,
    pub chunk_size: u32,
    pub checkpoint_seq: u64,
    pub checksum: u32,
}

impl Superblock {
    pub fn new(
        disk_size: u64,
        block_size: u32,
        slab_size: u32,
        max_element_size: u32,
        chunk_size: u32,
    ) -> Self {
        Self {
            magic: SUPERBLOCK_MAGIC,
            version: FORMAT_VERSION,
            disk_size,
            current_index_lba: 0,
            previous_index_lba: 0,
            block_size,
            slab_size,
            max_element_size,
            chunk_size,
            checkpoint_seq: 0,
            checksum: 0,
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = vec![0u8; SUPERBLOCK_SIZE];
        let mut pos = 0;

        buf[pos..pos + 8].copy_from_slice(&self.magic.to_le_bytes());
        pos += 8;
        buf[pos..pos + 4].copy_from_slice(&self.version.to_le_bytes());
        pos += 4;
        buf[pos..pos + 8].copy_from_slice(&self.disk_size.to_le_bytes());
        pos += 8;
        buf[pos..pos + 8]
            .copy_from_slice(&self.current_index_lba.to_le_bytes());
        pos += 8;
        buf[pos..pos + 8]
            .copy_from_slice(&self.previous_index_lba.to_le_bytes());
        pos += 8;
        buf[pos..pos + 4].copy_from_slice(&self.block_size.to_le_bytes());
        pos += 4;
        buf[pos..pos + 4].copy_from_slice(&self.slab_size.to_le_bytes());
        pos += 4;
        buf[pos..pos + 4]
            .copy_from_slice(&self.max_element_size.to_le_bytes());
        pos += 4;
        buf[pos..pos + 4].copy_from_slice(&self.chunk_size.to_le_bytes());
        pos += 4;
        buf[pos..pos + 8]
            .copy_from_slice(&self.checkpoint_seq.to_le_bytes());
        pos += 8;

        let crc = crc32fast::hash(&buf[..pos]);
        buf[pos..pos + 4].copy_from_slice(&crc.to_le_bytes());

        buf
    }

    pub fn deserialize(buf: &[u8]) -> Result<Self, ExtentManagerError> {
        if buf.len() < SUPERBLOCK_SIZE {
            return Err(error::corrupt_metadata("superblock too short"));
        }

        let mut pos = 0;

        let magic = u64::from_le_bytes(buf[pos..pos + 8].try_into().unwrap());
        pos += 8;
        if magic != SUPERBLOCK_MAGIC {
            return Err(error::corrupt_metadata(&format!(
                "invalid superblock magic: {magic:#x}"
            )));
        }

        let version =
            u32::from_le_bytes(buf[pos..pos + 4].try_into().unwrap());
        pos += 4;
        let disk_size =
            u64::from_le_bytes(buf[pos..pos + 8].try_into().unwrap());
        pos += 8;
        let current_index_lba =
            u64::from_le_bytes(buf[pos..pos + 8].try_into().unwrap());
        pos += 8;
        let previous_index_lba =
            u64::from_le_bytes(buf[pos..pos + 8].try_into().unwrap());
        pos += 8;
        let block_size =
            u32::from_le_bytes(buf[pos..pos + 4].try_into().unwrap());
        pos += 4;
        let slab_size =
            u32::from_le_bytes(buf[pos..pos + 4].try_into().unwrap());
        pos += 4;
        let max_element_size =
            u32::from_le_bytes(buf[pos..pos + 4].try_into().unwrap());
        pos += 4;
        let chunk_size =
            u32::from_le_bytes(buf[pos..pos + 4].try_into().unwrap());
        pos += 4;
        let checkpoint_seq =
            u64::from_le_bytes(buf[pos..pos + 8].try_into().unwrap());
        pos += 8;

        let stored_crc =
            u32::from_le_bytes(buf[pos..pos + 4].try_into().unwrap());
        let computed_crc = crc32fast::hash(&buf[..pos]);

        if stored_crc != computed_crc {
            return Err(error::corrupt_metadata(&format!(
                "superblock CRC mismatch: stored={stored_crc:#x} computed={computed_crc:#x}"
            )));
        }

        Ok(Self {
            magic,
            version,
            disk_size,
            current_index_lba,
            previous_index_lba,
            block_size,
            slab_size,
            max_element_size,
            chunk_size,
            checkpoint_seq,
            checksum: stored_crc,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let sb = Superblock::new(1024 * 1024 * 1024, 4096, 1024 * 1024, 65536, 131072);
        let buf = sb.serialize();
        assert_eq!(buf.len(), SUPERBLOCK_SIZE);

        let recovered = Superblock::deserialize(&buf).unwrap();
        assert_eq!(recovered.magic, SUPERBLOCK_MAGIC);
        assert_eq!(recovered.version, FORMAT_VERSION);
        assert_eq!(recovered.disk_size, 1024 * 1024 * 1024);
        assert_eq!(recovered.block_size, 4096);
        assert_eq!(recovered.slab_size, 1024 * 1024);
        assert_eq!(recovered.max_element_size, 65536);
        assert_eq!(recovered.chunk_size, 131072);
        assert_eq!(recovered.checkpoint_seq, 0);
    }

    #[test]
    fn corrupt_crc_detected() {
        let sb = Superblock::new(1024 * 1024, 4096, 65536, 4096, 131072);
        let mut buf = sb.serialize();
        buf[10] ^= 0xFF;
        let err = Superblock::deserialize(&buf).unwrap_err();
        assert!(err.to_string().contains("CRC mismatch"));
    }

    #[test]
    fn invalid_magic_detected() {
        let sb = Superblock::new(1024 * 1024, 4096, 65536, 4096, 131072);
        let mut buf = sb.serialize();
        buf[0] = 0xFF;
        // Recompute won't help since magic check is first
        let err = Superblock::deserialize(&buf).unwrap_err();
        assert!(err.to_string().contains("magic"));
    }
}
