use std::sync::Arc;

use parking_lot::RwLock;

use interfaces::{Extent, ExtentKey, ExtentManagerError};

use crate::block_io::BlockDeviceClient;
use crate::error;
use crate::state::ManagerState;
use crate::superblock::{Superblock, SUPERBLOCK_SIZE};

pub(crate) const CHUNK_MAGIC: u32 = 0x434B_4E4B; // "CKNK"
pub(crate) const CHUNK_HEADER_SIZE: usize = 36;

const INDEX_ENTRY_SIZE: usize = 20; // u64 key + u64 offset + u32 size
const SLAB_ENTRY_SIZE: usize = 16; // u64 start_offset + u32 slab_size + u32 element_size

#[derive(Debug, Clone)]
pub(crate) struct ChunkHeader {
    pub magic: u32,
    pub seq: u64,
    pub prev_lba: u64,
    pub next_lba: u64,
    pub payload_len: u32,
    pub checksum: u32,
}

impl ChunkHeader {
    pub fn serialize(&self, payload: &[u8]) -> Vec<u8> {
        let mut buf = Vec::with_capacity(CHUNK_HEADER_SIZE + payload.len());

        buf.extend_from_slice(&self.magic.to_le_bytes());
        buf.extend_from_slice(&self.seq.to_le_bytes());
        buf.extend_from_slice(&self.prev_lba.to_le_bytes());
        buf.extend_from_slice(&self.next_lba.to_le_bytes());
        buf.extend_from_slice(&self.payload_len.to_le_bytes());

        let crc_placeholder = 0u32;
        buf.extend_from_slice(&crc_placeholder.to_le_bytes());

        buf.extend_from_slice(payload);

        let crc = crc32fast::hash(&buf);
        buf[32..36].copy_from_slice(&crc.to_le_bytes());

        buf
    }

    pub fn deserialize(buf: &[u8]) -> Result<(Self, &[u8]), ExtentManagerError> {
        if buf.len() < CHUNK_HEADER_SIZE {
            return Err(error::corrupt_metadata("chunk too short"));
        }

        let magic = u32::from_le_bytes(buf[0..4].try_into().unwrap());
        if magic != CHUNK_MAGIC {
            return Err(error::corrupt_metadata(&format!(
                "invalid chunk magic: {magic:#x}"
            )));
        }

        let seq = u64::from_le_bytes(buf[4..12].try_into().unwrap());
        let prev_lba = u64::from_le_bytes(buf[12..20].try_into().unwrap());
        let next_lba = u64::from_le_bytes(buf[20..28].try_into().unwrap());
        let payload_len = u32::from_le_bytes(buf[28..32].try_into().unwrap());
        let stored_crc = u32::from_le_bytes(buf[32..36].try_into().unwrap());

        let crc_end = CHUNK_HEADER_SIZE + payload_len as usize;
        let mut check_buf = buf[..crc_end].to_vec();
        check_buf[32..36].copy_from_slice(&0u32.to_le_bytes());
        let computed_crc = crc32fast::hash(&check_buf);

        if stored_crc != computed_crc {
            return Err(error::corrupt_metadata(&format!(
                "chunk CRC mismatch: stored={stored_crc:#x} computed={computed_crc:#x}"
            )));
        }

        let payload_end = CHUNK_HEADER_SIZE + payload_len as usize;
        if buf.len() < payload_end {
            return Err(error::corrupt_metadata("chunk payload truncated"));
        }

        let header = Self {
            magic,
            seq,
            prev_lba,
            next_lba,
            payload_len,
            checksum: stored_crc,
        };

        Ok((header, &buf[CHUNK_HEADER_SIZE..payload_end]))
    }
}

pub(crate) fn serialize_index_and_slabs(
    state: &ManagerState,
    chunk_size: u32,
) -> Vec<Vec<u8>> {
    let max_payload = chunk_size as usize - CHUNK_HEADER_SIZE;
    let mut all_data = Vec::new();

    let num_entries = state.index.len() as u32;
    all_data.extend_from_slice(&num_entries.to_le_bytes());

    for (key, extent) in &state.index {
        all_data.extend_from_slice(&key.to_le_bytes());
        all_data.extend_from_slice(&extent.offset.to_le_bytes());
        all_data.extend_from_slice(&extent.size.to_le_bytes());
    }

    let num_slabs = state.slabs.len() as u32;
    all_data.extend_from_slice(&num_slabs.to_le_bytes());

    for slab in &state.slabs {
        all_data.extend_from_slice(&slab.start_offset.to_le_bytes());
        all_data.extend_from_slice(&slab.slab_size.to_le_bytes());
        all_data.extend_from_slice(&slab.element_size.to_le_bytes());
    }

    let mut chunks = Vec::new();
    let mut offset = 0;
    while offset < all_data.len() {
        let end = (offset + max_payload).min(all_data.len());
        chunks.push(all_data[offset..end].to_vec());
        offset = end;
    }

    if chunks.is_empty() {
        chunks.push(Vec::new());
    }

    chunks
}

pub(crate) fn deserialize_index_and_slabs(
    data: &[u8],
) -> Result<
    (
        std::collections::HashMap<ExtentKey, Extent>,
        Vec<SlabDescriptor>,
    ),
    ExtentManagerError,
> {
    if data.len() < 4 {
        return Err(error::corrupt_metadata("checkpoint data too short"));
    }

    let mut pos = 0;

    let num_entries = u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap()) as usize;
    pos += 4;

    let mut index = std::collections::HashMap::with_capacity(num_entries);
    for _ in 0..num_entries {
        if pos + INDEX_ENTRY_SIZE > data.len() {
            return Err(error::corrupt_metadata("truncated index entry"));
        }
        let key = u64::from_le_bytes(data[pos..pos + 8].try_into().unwrap());
        pos += 8;
        let offset = u64::from_le_bytes(data[pos..pos + 8].try_into().unwrap());
        pos += 8;
        let size = u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap());
        pos += 4;

        index.insert(key, Extent { key, offset, size });
    }

    if pos + 4 > data.len() {
        return Err(error::corrupt_metadata("truncated slab count"));
    }

    let num_slabs = u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap()) as usize;
    pos += 4;

    let mut slabs = Vec::with_capacity(num_slabs);
    for _ in 0..num_slabs {
        if pos + SLAB_ENTRY_SIZE > data.len() {
            return Err(error::corrupt_metadata("truncated slab entry"));
        }
        let start_offset = u64::from_le_bytes(data[pos..pos + 8].try_into().unwrap());
        pos += 8;
        let slab_size = u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap());
        pos += 4;
        let element_size = u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap());
        pos += 4;

        slabs.push(SlabDescriptor {
            start_offset,
            slab_size,
            element_size,
        });
    }

    Ok((index, slabs))
}

#[derive(Debug, Clone)]
pub(crate) struct SlabDescriptor {
    pub start_offset: u64,
    pub slab_size: u32,
    pub element_size: u32,
}

pub(crate) fn write_checkpoint(
    client: &BlockDeviceClient,
    state: &Arc<RwLock<Option<ManagerState>>>,
    superblock: &mut Superblock,
) -> Result<(), ExtentManagerError> {
    let chunk_size;
    let chunk_lbas: Vec<u64>;

    {
        let mut state_write = state.write();
        let s = state_write
            .as_mut()
            .ok_or_else(|| error::not_initialized("component not initialized"))?;

        chunk_size = s.format_params.chunk_size;
        let payloads = serialize_index_and_slabs(s, chunk_size);

        let mut lbas = Vec::with_capacity(payloads.len());
        for _ in &payloads {
            let lba_offset = s
                .buddy
                .alloc(chunk_size as u64)
                .ok_or_else(error::out_of_space)?;
            let lba = (lba_offset + SUPERBLOCK_SIZE as u64) / client.block_size() as u64;
            lbas.push(lba);
        }

        chunk_lbas = lbas;
    }

    let payloads;
    {
        let state_read = state.read();
        let s = state_read
            .as_ref()
            .ok_or_else(|| error::not_initialized("component not initialized"))?;

        payloads = serialize_index_and_slabs(s, chunk_size);
    }

    let new_seq = superblock.checkpoint_seq + 1;

    for (i, payload) in payloads.iter().enumerate() {
        let prev_lba = if i == 0 { 0 } else { chunk_lbas[i - 1] };
        let next_lba = if i + 1 < payloads.len() {
            chunk_lbas[i + 1]
        } else {
            0
        };

        let header = ChunkHeader {
            magic: CHUNK_MAGIC,
            seq: new_seq,
            prev_lba,
            next_lba,
            payload_len: payload.len() as u32,
            checksum: 0,
        };

        let mut chunk_data = header.serialize(payload);
        chunk_data.resize(chunk_size as usize, 0);

        client.write_blocks(chunk_lbas[i], &chunk_data)?;
    }

    let old_previous = superblock.previous_index_lba;
    superblock.previous_index_lba = superblock.current_index_lba;
    superblock.current_index_lba = chunk_lbas[0];
    superblock.checkpoint_seq = new_seq;

    {
        let mut state_write = state.write();
        let s = state_write.as_mut().unwrap();
        s.checkpoint_seq = new_seq;

        if old_previous != 0 {
            free_chain_allocations(client, old_previous, chunk_size, s);
        }
    }

    Ok(())
}

fn free_chain_allocations(
    client: &BlockDeviceClient,
    head_lba: u64,
    chunk_size: u32,
    state: &mut ManagerState,
) {
    let mut current_lba = head_lba;
    while current_lba != 0 {
        let buddy_offset =
            current_lba * client.block_size() as u64 - SUPERBLOCK_SIZE as u64;
        let next = client
            .read_blocks(current_lba, chunk_size as usize)
            .ok()
            .and_then(|raw| {
                let magic = u32::from_le_bytes(raw[0..4].try_into().ok()?);
                if magic != CHUNK_MAGIC {
                    return None;
                }
                Some(u64::from_le_bytes(raw[20..28].try_into().ok()?))
            })
            .unwrap_or(0);
        state.buddy.free(buddy_offset, chunk_size as u64);
        current_lba = next;
    }
}

pub(crate) fn read_chunk_chain(
    client: &BlockDeviceClient,
    head_lba: u64,
    expected_seq: u64,
    chunk_size: u32,
) -> Result<Vec<u8>, ExtentManagerError> {
    let mut data = Vec::new();
    let mut current_lba = head_lba;

    while current_lba != 0 {
        let raw = client.read_blocks(current_lba, chunk_size as usize)?;
        let (header, payload) = ChunkHeader::deserialize(&raw)?;

        if header.seq != expected_seq {
            return Err(error::corrupt_metadata(&format!(
                "chunk seq mismatch: expected={expected_seq} got={}",
                header.seq
            )));
        }

        data.extend_from_slice(payload);
        current_lba = header.next_lba;
    }

    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_header_round_trip() {
        let payload = b"hello world";
        let header = ChunkHeader {
            magic: CHUNK_MAGIC,
            seq: 42,
            prev_lba: 0,
            next_lba: 100,
            payload_len: payload.len() as u32,
            checksum: 0,
        };

        let serialized = header.serialize(payload);
        let (recovered, recovered_payload) = ChunkHeader::deserialize(&serialized).unwrap();

        assert_eq!(recovered.magic, CHUNK_MAGIC);
        assert_eq!(recovered.seq, 42);
        assert_eq!(recovered.prev_lba, 0);
        assert_eq!(recovered.next_lba, 100);
        assert_eq!(recovered_payload, payload);
    }

    #[test]
    fn chunk_corrupt_crc() {
        let payload = b"data";
        let header = ChunkHeader {
            magic: CHUNK_MAGIC,
            seq: 1,
            prev_lba: 0,
            next_lba: 0,
            payload_len: payload.len() as u32,
            checksum: 0,
        };

        let mut serialized = header.serialize(payload);
        serialized[CHUNK_HEADER_SIZE] ^= 0xFF;

        let err = ChunkHeader::deserialize(&serialized).unwrap_err();
        assert!(err.to_string().contains("CRC mismatch"));
    }

    #[test]
    fn serialize_deserialize_index() {
        let mut state = ManagerState::new_for_testing(1024 * 1024, 4096, 65536, 65536);
        state.index.insert(
            42,
            Extent {
                key: 42,
                offset: 8192,
                size: 4096,
            },
        );
        state.index.insert(
            99,
            Extent {
                key: 99,
                offset: 12288,
                size: 4096,
            },
        );

        let payloads = serialize_index_and_slabs(&state, 4096);
        let mut all_data = Vec::new();
        for p in &payloads {
            all_data.extend_from_slice(p);
        }

        let (index, slabs) = deserialize_index_and_slabs(&all_data).unwrap();
        assert_eq!(index.len(), 2);
        assert_eq!(index[&42].offset, 8192);
        assert_eq!(index[&99].offset, 12288);
        assert_eq!(slabs.len(), 0);
    }
}
