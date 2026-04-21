# Public API Contract: Metadata Manager V2

**Branch**: `001-metadata-manager` | **Date**: 2026-04-20 | **Spec**: [spec.md](../spec.md)

## Overview

The v2 metadata manager exposes its functionality through a new `IExtentManagerV2` interface defined via `define_interface!`. This replaces the v0 `IExtentManager` interface to support the reserve/publish/abort write model, explicit checkpoint/recover operations, and format-time initialization.

The existing `ExtentKey`, `Extent`, and `ExtentManagerError` types from the interfaces crate are reused where possible, with extensions as noted.

## Interface: IExtentManagerV2

### Lifecycle Methods

#### `set_dma_alloc(alloc: DmaAllocFn)`

Set the DMA allocator used for block device I/O buffers.

- **Preconditions**: Must be called before `format` or `initialize`.
- **Postconditions**: The allocator is stored for use by all subsequent I/O operations.
- **Errors**: None (infallible).
- **Thread Safety**: May be called once before concurrent operations begin.

#### `format(params: FormatParams) -> Result<(), ExtentManagerError>`

Initialize a fresh device with the given format-time parameters. Writes the initial superblock and creates the first metadata slab.

- **Preconditions**: DMA allocator set. Block device receptacle bound. Device is either unformatted or caller intends to overwrite.
- **Postconditions**: Superblock written to LBA 0 with initial parameters. Buddy allocator and slab list initialized. Device ready for `initialize`.
- **Errors**:
  - `IoError` — block device write failed.
  - `NotInitialized` — DMA allocator or block device not set.

#### `initialize() -> Result<(), ExtentManagerError>`

Read the superblock from the bound block device, locate the most recent valid checkpoint, and rebuild all in-memory state (index, slab bitmaps, buddy allocator).

- **Preconditions**: DMA allocator set. Block device receptacle bound. Device has been formatted (valid superblock at LBA 0).
- **Postconditions**: In-memory index contains all previously checkpointed entries. Slab bitmaps and buddy allocator rebuilt. Component ready for reserve/publish/lookup/remove operations.
- **Recovery Behavior**: If the primary index chain (`current_index_lba`) is corrupt, falls back to `previous_index_lba`. If both are corrupt, returns `CorruptMetadata`.
- **Errors**:
  - `CorruptMetadata` — superblock invalid or both checkpoint chains corrupt.
  - `IoError` — block device read failed.
  - `NotInitialized` — DMA allocator or block device not set.

### Write Handle Operations

#### `reserve_extent(key: ExtentKey, size: u32) -> Result<WriteHandle, ExtentManagerError>`

Reserve a contiguous disk extent of the appropriate size class for the given key. The key is recorded in the handle but not yet committed to the index.

- **Preconditions**: Component initialized. Size > 0. Size ≤ `max_element_size`.
- **Postconditions**: A bitmap slot is marked Reserved in the appropriate slab. If no slab of the matching element size has free slots, a new slab is allocated from the buddy allocator.
- **Returns**: A `WriteHandle` providing the extent offset, size, and publish/abort methods.
- **Errors**:
  - `OutOfSpace` — no free slots in existing slabs and buddy allocator cannot allocate a new slab.
  - `NotInitialized` — component not initialized.

#### `WriteHandle::extent_offset() -> u64`

Returns the byte offset on disk of the reserved extent. The caller uses this to write file data via the block device.

#### `WriteHandle::extent_size() -> u32`

Returns the size in bytes of the reserved extent.

#### `WriteHandle::key() -> ExtentKey`

Returns the key associated with this reservation.

#### `WriteHandle::publish(self) -> Result<Extent, ExtentManagerError>`

Commit the reservation: check for key conflicts, mark the slot as Allocated, insert the entry into the in-memory index.

- **Preconditions**: Handle is in Active state (not yet published or aborted).
- **Postconditions**: On success — extent visible in the index, immutable. Handle is consumed. On failure — extent is freed (equivalent to abort).
- **Returns**: The published `Extent` descriptor.
- **Errors**:
  - `DuplicateKey(key)` — another extent with the same key was published first. The reserved slot is freed.

#### `WriteHandle::abort(self)`

Cancel the reservation: free the bitmap slot, discard the handle. No entry is added to the index.

- **Preconditions**: Handle is in Active state.
- **Postconditions**: Bitmap slot returned to Free. Handle consumed.
- **Errors**: None (infallible).

#### `WriteHandle::drop` (implicit)

If a `WriteHandle` is dropped without calling `publish()` or `abort()`, it behaves as `abort()`.

### Query Operations

#### `lookup_extent(key: ExtentKey) -> Result<Extent, ExtentManagerError>`

Look up a published extent by its key.

- **Preconditions**: Component initialized.
- **Returns**: The `Extent` descriptor for the given key.
- **Errors**:
  - `KeyNotFound(key)` — no published extent exists for this key.

#### `get_extents() -> Vec<Extent>`

Return a vector of all published extents. Allocates and copies.

- **Preconditions**: Component initialized.
- **Returns**: All published extent descriptors. Empty vector if none.

#### `for_each_extent(cb: &mut dyn FnMut(&Extent))`

Iterate over all published extents without allocating. The callback is invoked while the implementation holds a read lock.

- **Preconditions**: Component initialized.
- **Thread Safety**: Callback must not call back into the extent manager (would deadlock).

### Mutation Operations

#### `remove_extent(key: ExtentKey) -> Result<(), ExtentManagerError>`

Remove a published extent from the index and free its bitmap slot.

- **Preconditions**: Component initialized. Key exists in the index.
- **Postconditions**: Key removed from index. Bitmap slot returned to Free. If the containing slab's bitmap is now all zeros, the slab is removed from the size-class list and its region is returned to the buddy allocator.
- **Errors**:
  - `KeyNotFound(key)` — no published extent exists for this key.

### Persistence Operations

#### `checkpoint() -> Result<(), ExtentManagerError>`

Write the full index and slab list to disk at a new location, then atomically update the superblock. This is the synchronous checkpoint API.

- **Preconditions**: Component initialized.
- **Postconditions**: All changes up to the call are durable. The superblock's `current_index_lba` points to the new chain; `previous_index_lba` points to the old chain. `checkpoint_seq` incremented. The checkpoint before `previous_index_lba` is freed.
- **Serialization**: Only one checkpoint executes at a time (FR-024). Concurrent calls block until the active checkpoint completes, then execute.
- **Locking Strategy**: Acquires write lock to allocate metadata chunks for the new checkpoint, then atomically downgrades to read lock for index serialization and I/O. Lookups proceed concurrently during I/O; writers block until the checkpoint completes. The index is serialized in-place (not copied) to avoid doubling memory usage.
- **Failure**: On I/O error, the superblock is not updated; the previous checkpoint remains valid. Pre-allocated metadata chunks are freed in memory. On crash mid-checkpoint, recovery uses the last successfully committed superblock; orphaned chunks are reclaimed during bitmap/buddy rebuild since they were never committed.
- **Errors**:
  - `IoError` — block device write failed. Superblock is not updated; previous checkpoint remains valid.

## Types

### FormatParams

Parameters provided at format time. Stored in the superblock.

| Field | Type | Description | Default |
|-------|------|-------------|---------|
| slab_size | u32 | Bytes per slab allocation from buddy | — (required) |
| max_element_size | u32 | Maximum file (element) size in bytes | — (required) |
| chunk_size | u32 | Size of metadata chunks in bytes | — (required) |
| block_size | u32 | Extent alignment unit and minimum buddy allocator block size | — (required) |

### WriteHandle

See Write Handle Operations above. This is a move-only type (not Clone, not Copy). It implements `Drop` with abort semantics.

### ExtentManagerError (extensions)

The existing error enum is reused. No new variants are needed for v2 — the existing variants cover all error cases:

| Variant | Used By |
|---------|---------|
| `CorruptMetadata(String)` | `initialize` (superblock or chain validation failure) |
| `DuplicateKey(ExtentKey)` | `publish` (key conflict) |
| `IoError(String)` | `format`, `initialize`, `checkpoint` |
| `KeyNotFound(ExtentKey)` | `lookup_extent`, `remove_extent` |
| `NotInitialized(String)` | Any operation before `initialize` |
| `OutOfSpace` | `reserve_extent` (no free slots, buddy full) |

## Component Wiring

```rust
define_component! {
    MetadataManagerV2 {
        provides: [IExtentManagerV2],
        receptacles: {
            block_device: IBlockDevice,
        },
        // Internal fields managed by the component
    }
}
```

The component exposes `IExtentManagerV2` and requires an `IBlockDevice` receptacle. The block device must be connected and the DMA allocator set before `format` or `initialize` is called.

## Backward Compatibility

The v2 interface is **not** backward-compatible with `IExtentManager` v0. Key differences:

| v0 (`IExtentManager`) | v2 (`IExtentManagerV2`) |
|------------------------|-------------------------|
| `create_extent(key, size)` — synchronous, atomic | `reserve_extent(key, size)` → `WriteHandle` → `publish()` |
| `initialize(total_size, slab_size)` — in-memory only | `format(params)` + `initialize()` — disk-backed |
| No persistence | `checkpoint()` — full index persistence |
| No recovery | `initialize()` reads superblock and recovers |
| Fixed slab size parameter | Dynamic size classes |

Consumers of v0 will need to migrate to the v2 interface. The two can coexist in the workspace as separate crates (`extent-manager/v0` and `extent-manager/v2`).

## Runtime Configuration

#### `set_checkpoint_interval(interval: std::time::Duration)`

Configure the background checkpoint interval. Default is 5 seconds (FR-016).

- **Preconditions**: Must be called before `initialize()` or between checkpoints.
- **Postconditions**: The background checkpoint task uses the new interval.
- **Thread Safety**: Safe to call concurrently; the next tick uses the updated value.

## Observable Behavior (FR-025)

The component emits structured log events at key state transitions:

| Event | When |
|-------|------|
| `checkpoint_start` | Synchronous or background checkpoint begins |
| `checkpoint_complete` | Checkpoint successfully written to disk |
| `recovery_start` | `initialize()` begins reading superblock |
| `recovery_complete` | In-memory state fully rebuilt |
| `recovery_fallback` | Primary index corrupt; falling back to previous |
| `io_error` | Block device I/O operation failed |
| `corruption_detected` | CRC32 mismatch on superblock or metadata chunk |
| `space_exhaustion` | A size class has no free slots and buddy is full |
