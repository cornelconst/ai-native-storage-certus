# Interface Contract: IDispatchMap

**Crate**: interfaces (`components/interfaces/src/idispatch_map.rs`)
**Feature gate**: `#[cfg(feature = "spdk")]`

## Types

```rust
pub type CacheKey = u64;

pub enum LookupResult {
    NotExist,
    MismatchSize,
    Staging { ptr: *mut c_void, len: usize },
    BlockDevice { offset: u64, device_id: u16 },
}

pub enum DispatchMapError {
    KeyNotFound(CacheKey),
    AlreadyExists(CacheKey),
    ActiveReferences(CacheKey),
    Timeout(CacheKey),
    AllocationFailed(String),
    InvalidSize,
    NotInitialized(String),
    RefCountUnderflow(CacheKey),
    NoWriteReference(CacheKey),
    InvalidState(String),
}
```

## Interface Methods

| Method | Signature | Semantics |
|--------|-----------|-----------|
| `set_dma_alloc` | `(&self, alloc: DmaAllocFn)` | Set the DMA buffer allocator. Must be called before `create_staging`. |
| `initialize` | `(&self) -> Result<(), DispatchMapError>` | Recover committed extents from IExtentManager. Must be called after receptacles are bound. |
| `create_staging` | `(&self, key: CacheKey, size: u32) -> Result<*mut c_void, DispatchMapError>` | Allocate staging buffer, record entry with write_ref=1. Error on size=0, alloc failure, or key exists. |
| `lookup` | `(&self, key: CacheKey, timeout: Duration) -> Result<LookupResult, DispatchMapError>` | Return location; blocks until write_ref=0 or timeout. Increments read_ref on success. |
| `convert_to_storage` | `(&self, key: CacheKey, offset: u64, block_device_id: u16) -> Result<(), DispatchMapError>` | Transition Staging → BlockDevice. Frees DMA buffer. Error if not staging or key not found. |
| `take_read` | `(&self, key: CacheKey, timeout: Duration) -> Result<(), DispatchMapError>` | Wait for write_ref=0 (up to timeout), then increment read_ref. |
| `take_write` | `(&self, key: CacheKey, timeout: Duration) -> Result<(), DispatchMapError>` | Wait for read_ref=0 and write_ref=0 (up to timeout), then set write_ref=1. |
| `release_read` | `(&self, key: CacheKey) -> Result<(), DispatchMapError>` | Decrement read_ref. Error if already 0. Notifies blocked writers. |
| `release_write` | `(&self, key: CacheKey) -> Result<(), DispatchMapError>` | Decrement write_ref. Error if already 0. Notifies blocked readers/writers. |
| `downgrade_reference` | `(&self, key: CacheKey) -> Result<(), DispatchMapError>` | Atomically: write_ref=0, read_ref+=1. Error if no write ref held. |
| `remove` | `(&self, key: CacheKey) -> Result<(), DispatchMapError>` | Delete entry. Error if any refs active or key not found. |

## Invariants

1. `write_ref` is always 0 or 1.
2. If `write_ref == 1`, `read_ref == 0` (enforced by `take_write` precondition).
3. `downgrade_reference` transitions atomically: no window where both refs are 0.
4. After `convert_to_storage`, the DMA buffer is freed and subsequent lookups return `BlockDevice`.
5. `remove` is only valid when `read_ref == 0` and `write_ref == 0`.
6. All methods are thread-safe (`&self` with internal synchronization).

## Error Semantics

All error conditions return `Err(DispatchMapError)` — no panics, no silent no-ops. The caller is expected to handle errors explicitly.
