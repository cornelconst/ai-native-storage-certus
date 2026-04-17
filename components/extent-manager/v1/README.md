# extent-manager

Fixed-size storage extent allocator with crash-consistent on-disk layout for NVMe SSDs. Part of the Certus project.

## Overview

The extent manager allocates, tracks, and recovers named extents (size-classified storage regions) on block devices. It uses a two-phase write protocol with CRC-32 integrity checks for power-fail safety.

## On-Disk Layout

```
Block 0:          Superblock (magic "EXTMGRV1", format v1, slab table, CRC-32)
Blocks 1..N:      Per-slab bitmap regions (one bit per slot, rounded up to 4KiB blocks)
Blocks N+1..M:    Per-slab extent record regions (one 4KiB block per slot)
```

Each extent record is a 4096-byte block containing metadata (key, size class, namespace ID, LBA offset, filename, data CRC) with a CRC-32 checksum at bytes 4092-4096.

### Crash Consistency

Write protocol:
1. Write the extent record block (with CRC-32)
2. Flip the bitmap bit atomically
3. Update the in-memory index

On recovery (`open()`), the manager scans for inconsistencies:
- **Orphan records** (bitmap bit not set but record exists) — zeroed out
- **Corrupt CRC records** — cleared from bitmap and zeroed

## Interfaces

| Interface | Role | Description |
|-----------|------|-------------|
| `IExtentManager` | Provided | Extent CRUD operations |
| `IExtentManagerAdmin` | Provided | Lifecycle management (DMA setup, initialize, open) |
| `IBlockDevice` | Receptacle | Underlying NVMe block device |

### IExtentManager API

| Method | Description |
|--------|-------------|
| `create_extent(key, size_class, filename, data_crc, has_crc)` | Allocate an extent, returns serialized metadata |
| `remove_extent(key)` | Deallocate an extent |
| `lookup_extent(key)` | Find an extent by key, returns serialized metadata |
| `extent_count()` | Number of allocated extents |

Valid size classes: 128KiB to 5MiB, must be 4KiB-aligned.

## Build

```bash
# Default build (with SPDK feature)
cargo build -p extent-manager

# Without SPDK (for testing only)
cargo build -p extent-manager --no-default-features
```

This crate is excluded from the workspace `default-members` and must be built explicitly.

## Test

```bash
cargo test -p extent-manager
```

All tests run without NVMe hardware using `MockBlockDevice`, an in-memory block device implementation with optional fault injection.

### Test Suites

| File | Coverage |
|------|----------|
| `tests/api_operations.rs` | Create, remove, lookup, extent count, error cases |
| `tests/crash_recovery.rs` | Orphan detection, CRC corruption, bitmap/record consistency |
| `tests/thread_safety.rs` | Concurrent access patterns |

## Benchmarks

Criterion-based benchmarks using `MockBlockDevice`:

```bash
cargo bench -p extent-manager

# Individual suites
cargo bench -p extent-manager --bench create_benchmark
cargo bench -p extent-manager --bench remove_benchmark
cargo bench -p extent-manager --bench lookup_benchmark
cargo bench -p extent-manager --bench iterate_benchmark
```

## Source Layout

```
src/
  lib.rs            ExtentManagerComponentV1 definition, IExtentManager/IExtentManagerAdmin impls
  metadata.rs       ExtentMetadata, OnDiskExtentRecord (4KiB block with CRC-32)
  superblock.rs     Superblock format (magic, version, slab table, CRC-32)
  bitmap.rs         AllocationBitmap (bit-per-slot, multi-block serialization)
  block_device.rs   BlockDeviceClient wrapper (read_block/write_block at 4KiB granularity)
  state.rs          ExtentManagerState (in-memory index, slab descriptors, free-LBA cursor)
  recovery.rs       Crash recovery: scan and repair orphan/corrupt records on open()
  error.rs          Error constructors
  test_support.rs   MockBlockDevice, FaultConfig, test helpers (feature = "testing")
tests/
  api_operations.rs   Extent CRUD and error handling tests
  crash_recovery.rs   Power-fail recovery tests
  thread_safety.rs    Concurrent access tests
benches/
  create_benchmark.rs
  remove_benchmark.rs
  lookup_benchmark.rs
  iterate_benchmark.rs
```
