# Data Model: Dispatch Map Component

**Date**: 2026-04-27

## Entities

### CacheKey

Type alias for `u64`. Uniquely identifies an extent in the dispatch map. Matches `ExtentKey` from the extent manager.

### Location

Enum representing where extent data resides.

| Variant | Fields | Size | Description |
|---------|--------|------|-------------|
| Staging | ptr: `*mut c_void`, len: `usize` | 16 bytes | In-memory DMA buffer pointer and length |
| BlockDevice | offset: `u64`, device_id: `u16` | 10 bytes + pad | On-disk location on a specific block device |

Transitions: Staging → BlockDevice (one-way via `convert_to_storage`). No reverse transition.

### DispatchEntry

Per-key metadata stored in the hash map.

| Field | Type | Size | Description |
|-------|------|------|-------------|
| location | Location | 16 bytes | Where the data resides |
| extent_manager_id | u16 | 2 bytes | Which extent manager owns this extent |
| size_blocks | u32 | 4 bytes | Extent size in 4KiB blocks |
| read_ref | u32 | 4 bytes | Active reader count |
| write_ref | u32 | 4 bytes | Active writer count (0 or 1) |
| **Total** | | **30 bytes** | + padding ≈ 32 bytes |

### DispatchMapState

Internal synchronization wrapper (not exposed via interface).

| Field | Type | Description |
|-------|------|-------------|
| entries | `Mutex<HashMap<CacheKey, DispatchEntry>>` | Protected map of all entries |
| buffers | `Mutex<HashMap<CacheKey, DmaBuffer>>` | Owned DMA buffers for staging entries |
| condvar | `Condvar` | Wakes threads blocked on ref count conditions |
| dma_alloc | `Option<DmaAllocFn>` | DMA buffer allocator, set during setup |

Note: `entries` and `buffers` could share a single Mutex to ensure atomicity of operations that touch both (e.g., `convert_to_storage` removes from buffers and updates entry location). Implementation will determine whether a single Mutex or paired locking (always acquire in same order) is simpler.

### LookupResult

Return type for `lookup()`.

| Variant | Fields | Description |
|---------|--------|-------------|
| NotExist | — | Key not found in map |
| MismatchSize | — | Key found but caller-expected size differs |
| Staging | ptr: `*mut c_void`, len: `usize` | DMA buffer pointer for direct I/O |
| BlockDevice | offset: `u64`, device_id: `u16` | On-disk location |

### DispatchMapError

Error enum for all IDispatchMap operations.

| Variant | Fields | When |
|---------|--------|------|
| KeyNotFound | key: CacheKey | Operation on non-existent key |
| AlreadyExists | key: CacheKey | `create_staging` on key that already exists |
| ActiveReferences | key: CacheKey | `remove` while refs > 0 |
| Timeout | key: CacheKey | Blocking wait exceeded deadline |
| AllocationFailed | msg: String | DMA buffer allocation failed |
| InvalidSize | — | `create_staging` with size=0 |
| NotInitialized | msg: String | Operation before `initialize()` or missing DmaAllocFn |
| RefCountUnderflow | key: CacheKey | `release_read`/`release_write` when count is 0 |
| NoWriteReference | key: CacheKey | `downgrade_reference` without write ref held |
| InvalidState | msg: String | `convert_to_storage` on non-staging entry |

## State Machine

```
                    create_staging(key, size)
                           │
                           ▼
    ┌─────────────────────────────────────┐
    │            Staging                   │
    │  ptr, len, extent_manager_id, size  │
    │  write_ref=1                        │
    └───────────────┬─────────────────────┘
                    │ convert_to_storage(key, offset, device_id)
                    ▼
    ┌─────────────────────────────────────┐
    │          BlockDevice                 │
    │  offset, device_id, size            │
    │  write_ref=0, read_ref=0            │
    └───────────────┬─────────────────────┘
                    │ remove(key)  [requires all refs=0]
                    ▼
              (entry deleted)
```

Recovery path: `initialize()` → entries created directly as BlockDevice (from extent manager iteration).

## Relationships

```
DispatchMapComponentV0
    ├── provides: IDispatchMap
    ├── receptacle: ILogger (logging)
    ├── receptacle: IExtentManager (recovery via for_each_extent)
    └── internal: DispatchMapState
                    ├── entries: HashMap<CacheKey, DispatchEntry>
                    ├── buffers: HashMap<CacheKey, DmaBuffer>
                    └── condvar: Condvar
```
