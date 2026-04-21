# Data Model: Metadata Manager Component

**Branch**: `001-metadata-manager` | **Date**: 2026-04-20 | **Spec**: [spec.md](spec.md)

## Entities

### ExtentKey

A 64-bit unsigned integer uniquely identifying a file in the flat namespace.

| Field | Type | Constraints |
|-------|------|-------------|
| value | u64 | No reserved values; 0 and u64::MAX are valid keys |

---

### Extent

A descriptor for a published file's disk location.

| Field | Type | Constraints |
|-------|------|-------------|
| key | ExtentKey | Unique within the index |
| offset | u64 | Byte offset on disk; must fall within a slab boundary |
| size | u32 | Size in bytes; matches the slab element size |

**Relationships**: One-to-one with ExtentKey. Contained within exactly one Slab.

---

### WriteHandle

A transient handle representing a reserved-but-not-yet-published extent. Returned by `reserve_extent`, consumed by `publish()` or `abort()`.

| Field | Type | Constraints |
|-------|------|-------------|
| key | ExtentKey | The key the caller intends to publish under |
| offset | u64 | Byte offset of the reserved extent on disk |
| size | u32 | Size of the reserved extent in bytes |
| slab_index | usize | Index of the containing slab in the slab list |
| slot_index | usize | Slot index within the slab's bitmap |
| state | HandleState | Tracks lifecycle (Active → Published or Aborted) |

**Invariants**:
- A WriteHandle holds exclusive ownership of its bitmap slot (marked Reserved).
- Drop without publish/abort triggers abort semantics (free the slot).
- After publish() or abort(), the handle is consumed; no further operations are possible.

---

### HandleState

State machine for WriteHandle lifecycle.

```text
Active ──► Published  (via publish())
Active ──► Aborted    (via abort() or Drop)
```

| State | Description |
|-------|-------------|
| Active | Extent is reserved in bitmap; not yet visible in index |
| Published | Extent committed to index; handle consumed |
| Aborted | Extent freed from bitmap; handle consumed |

---

### Slab

A contiguous region of disk dedicated to allocations of a single element size.

| Field | Type | Constraints |
|-------|------|-------------|
| start_lba | u64 | Starting LBA on disk (from buddy allocator) |
| slab_size | u32 | Total size of the slab in bytes |
| element_size | u32 | Size of each slot in bytes; slab_size / element_size = slot count |

**In-memory-only fields** (not persisted):

| Field | Type | Description |
|-------|------|-------------|
| bitmap | BitVec | Per-slot state: Free, Reserved, or Allocated |
| rover | usize | Roving index — hint for next-fit search |

**Invariants**:
- `slab_size % element_size == 0`
- `element_size <= max_element_size` (superblock parameter)
- Bitmap length = `slab_size / element_size`

**Reclamation**: When all slots in a slab become free (bitmap all zeros after abort or remove), the slab is removed from the size-class slab list and its region is returned to the buddy allocator via `buddy.free()`. This prevents dead slabs from permanently locking disk space to one size class.

**Relationships**: Contains zero or more Extents. Allocated from and returned to the BuddyAllocator.

---

### SlabBitmapSlotState

Per-slot state within a slab bitmap.

| State | Value | Description |
|-------|-------|-------------|
| Free | 0b00 | Available for reservation |
| Reserved | 0b01 | Held by an active WriteHandle; not visible to readers |
| Allocated | 0b10 | Published; corresponding Extent exists in the Index |

---

### BuddyAllocator

In-memory-only binary buddy allocator managing the full disk (minus superblock) as power-of-two blocks.

| Field | Type | Constraints |
|-------|------|-------------|
| total_size | u64 | Disk size minus superblock (from superblock) |
| block_size | u32 | Minimum allocation unit in bytes; same as superblock block_size |
| max_order | usize | Computed: log2(total_size / block_size) |
| free_lists | Vec<Vec<u64>> | Per-order list of free block offsets |

**Initialization**: The usable disk size (total minus superblock) is decomposed into a sum of decreasing power-of-two blocks by iterating the set bits of `usable_blocks = usable_size / block_size`. Each block is placed on the appropriate free list. Tail blocks that lack a right buddy can never merge upward; the merge operation checks `buddy_offset < total_usable_size` before coalescing.

**Invariants**:
- Never persisted to disk.
- Rebuilt at recovery by: (1) initializing all space as free using the decomposition above, (2) walking the slab list and marking each slab's region as allocated.
- Used exclusively to allocate and reclaim slabs (not individual file extents).
- Merge checks buddy bounds — tail blocks without a right buddy are never coalesced upward.

---

### Index

In-memory map from ExtentKey to Extent, containing only published entries.

| Field | Type | Constraints |
|-------|------|-------------|
| entries | HashMap<ExtentKey, Extent> | Unique keys; only published extents |

**Invariants**:
- Contains only Published extents (not reserved).
- Serialized in full at checkpoint time.
- Reconstructed from disk at recovery time.

---

### Superblock

4 KiB structure at LBA 0, the atomic commit point for checkpoints.

| Field | Type | Constraints |
|-------|------|-------------|
| magic | u64 | Fixed validation constant |
| version | u32 | On-disk format version |
| disk_size | u64 | Total device size in bytes |
| current_index_lba | u64 | Head of the current checkpoint's metadata chunk chain |
| previous_index_lba | u64 | Head of the prior checkpoint (recovery fallback) |
| block_size | u32 | Configured block size in bytes — extent alignment unit and minimum buddy allocator bucket |
| slab_size | u32 | Default slab allocation size in bytes |
| max_element_size | u32 | Maximum slab element size in bytes |
| chunk_size | u32 | Size of metadata linked-list chunks in bytes |
| checkpoint_seq | u64 | Monotonically increasing sequence number |
| checksum | u32 | CRC32 of all preceding fields |

**Invariants**:
- Located at byte offset 0 on disk; exactly 4096 bytes.
- `checksum` covers all fields except itself and trailing reserved padding.
- `current_index_lba` and `previous_index_lba` must point to valid chunk chains (or 0 if none).

---

### MetadataChunk

Fixed-size block in a doubly-linked list, used to store serialized index and slab list data.

| Field | Type | Constraints |
|-------|------|-------------|
| magic | u32 | Chunk validation constant |
| seq | u64 | Must match superblock's checkpoint_seq |
| prev_lba | u64 | LBA of previous chunk (0 if first) |
| next_lba | u64 | LBA of next chunk (0 if last) |
| payload_len | u32 | Valid bytes of payload in this chunk |
| checksum | u32 | CRC32 of header + payload |
| payload | [u8] | Serialized data (index entries and/or slab list entries) |

**Invariants**:
- Allocated from the metadata slab.
- Forward and backward chains must be mutually consistent.
- `payload_len <= chunk_size - header_size`
- `seq` must match the superblock's `checkpoint_seq` for the chain to be valid.

---

## State Transitions

### WriteHandle Lifecycle

```text
             reserve_extent(key, size)
                      │
                      ▼
               ┌─────────────┐
               │   Active     │
               │ (bitmap slot │
               │  = Reserved) │
               └──────┬───────┘
                      │
              ┌───────┴───────┐
              ▼               ▼
         publish()        abort() / Drop
              │               │
              ▼               ▼
     ┌─────────────┐  ┌─────────────┐
     │  Published   │  │   Aborted   │
     │ (slot =      │  │ (slot =     │
     │  Allocated,  │  │  Free)      │
     │  key in      │  │             │
     │  index)      │  └─────────────┘
     └─────────────┘
```

### Checkpoint Lifecycle

```text
     Idle
       │
       ▼
     Acquire write lock
       │
       ├─► Allocate metadata chunks for new checkpoint
       │
       ▼
     Downgrade to read lock (atomic)
       │
       ├─► Serialize index + slab list to allocated chunks (I/O)
       │
       ▼
     Release read lock
       │
       ├─► Write superblock (atomic commit point)
       │
       ▼
     Free old fallback chunks ──► Idle
```

The write lock is held only during chunk allocation. The read lock is held during serialization and I/O, allowing concurrent lookups but blocking writers. The index is serialized in-place (not copied) to avoid doubling memory usage.

### Recovery Flow

```text
     Read Superblock ──► Validate ──► Read Primary Chain ──► Deserialize
            │                               │ (failed)
            │                               ▼
            │                     Read Fallback Chain ──► Deserialize
            │                               │ (failed)
            │                               ▼
            │                        CorruptMetadata Error
            ▼
     Rebuild Slab Bitmaps ──► Rebuild Buddy Allocator ──► Ready
```

### Slab Lifecycle

```text
     (new size encountered or slab full)
              │
              ▼
     Buddy allocates region ──► Slab created ──► Slots in use
              │                                       │
              ▼                                       │ (all slots freed)
     Slab added to slab list                          ▼
                                              Slab removed from list
                                                      │
                                                      ▼
                                              Region returned to buddy
```

## Format-Time Parameters

These values are set once when the device is formatted and stored in the superblock:

| Parameter | Stored In | Description |
|-----------|-----------|-------------|
| slab_size | Superblock | Bytes allocated per slab from buddy |
| max_element_size | Superblock | Maximum file (element) size |
| chunk_size | Superblock | Size of metadata linked-list chunks |
| block_size | Superblock | Extent alignment unit and minimum buddy allocator block size |
