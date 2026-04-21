# Quickstart: Metadata Manager V2

**Branch**: `001-metadata-manager` | **Date**: 2026-04-20 | **Spec**: [spec.md](spec.md)

## Setup

Add the crate to your workspace and configure dependencies:

```toml
# Cargo.toml
[dependencies]
component-core = { path = "../../component-core" }
component-macros = { path = "../../component-macros" }
component-framework = { path = "../../component-framework" }
interfaces = { path = "../../interfaces", features = ["spdk"] }
crc32fast = "1.4"
```

## Format a Device

Before first use, format the block device with the desired parameters:

```rust
use extent_manager_v2::MetadataManagerV2;
use interfaces::{IBlockDevice, IExtentManagerV2, FormatParams};

// Obtain block device and DMA allocator from the SPDK environment
let manager = MetadataManagerV2::new();
registry.bind_receptacle(&manager, "block_device", &block_device)?;

manager.set_dma_alloc(dma_alloc_fn);
manager.format(FormatParams {
    slab_size: 16 * 1024 * 1024,       // 16 MiB slabs
    max_element_size: 1024 * 1024,      // 1 MiB max file size
    chunk_size: 128 * 1024,             // 128 KiB metadata chunks (per-page CRC32 unit)
    block_size: 4096,                   // 4 KiB block alignment and minimum buddy block
})?;
```

## Initialize (Startup / Recovery)

On subsequent starts, initialize from the existing superblock:

```rust
manager.set_dma_alloc(dma_alloc_fn);
manager.initialize()?;
// All previously checkpointed extents are now available
```

## Reserve, Write, and Publish a File

```rust
use interfaces::ExtentKey;

let key: ExtentKey = 42;
let file_size: u32 = 64 * 1024; // 64 KiB

// Step 1: Reserve an extent
let handle = manager.reserve_extent(key, file_size)?;

// Step 2: Write file data using the block device at the given offset
let offset = handle.extent_offset();
let size = handle.extent_size();
// ... write data to block device at `offset` ...

// Step 3: Publish — makes the file visible and immutable
let extent = handle.publish()?;
assert_eq!(extent.key, key);
assert_eq!(extent.offset, offset);
```

## Abort a Reservation

```rust
let handle = manager.reserve_extent(99, 4096)?;
// Decided not to write this file
handle.abort();
// Or simply drop the handle — same effect
```

## Look Up a File

```rust
let extent = manager.lookup_extent(42)?;
println!("File 42 is at offset {} with size {}", extent.offset, extent.size);
```

## Remove a File

```rust
manager.remove_extent(42)?;
// Space is returned to the free pool
```

## Enumerate All Files

```rust
// Option A: Collect into a Vec
let all_extents = manager.get_extents();

// Option B: Iterate without allocation
manager.for_each_extent(&mut |extent| {
    println!("Key: {}, Offset: {}, Size: {}", extent.key, extent.offset, extent.size);
});
```

## Checkpoint (Persist to Disk)

```rust
// Synchronous checkpoint — all changes become durable
manager.checkpoint()?;

// A background task also runs checkpoint() every few seconds automatically.
```

## Error Handling

```rust
use interfaces::ExtentManagerError;

match manager.reserve_extent(key, size) {
    Ok(handle) => { /* proceed */ }
    Err(ExtentManagerError::OutOfSpace) => { /* disk full */ }
    Err(ExtentManagerError::NotInitialized(msg)) => { /* call initialize() first */ }
    Err(e) => { /* unexpected error */ }
}

match handle.publish() {
    Ok(extent) => { /* success */ }
    Err(ExtentManagerError::DuplicateKey(k)) => {
        // Another writer published the same key first.
        // The reservation has been freed automatically.
    }
    Err(e) => { /* unexpected */ }
}
```

## Typical Lifecycle

```text
format()        ← once, on fresh device
initialize()    ← every startup
reserve_extent → write data → publish/abort  ← per file
checkpoint()    ← periodically (automatic + manual)
remove_extent   ← when files are deleted
```
