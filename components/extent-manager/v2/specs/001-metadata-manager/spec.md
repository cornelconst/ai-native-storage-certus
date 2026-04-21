# Feature Specification: Metadata Manager Component

**Feature Branch**: `001-metadata-manager`  
**Created**: 2026-04-20  
**Status**: Draft  
**Input**: User description: "Metadata management component for a simplified, flat-namespace, application-specific user space file system built on the component framework, with block I/O via SPDK-based interfaces."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Reserve, Write, and Publish a File (Priority: P1)

A writer declares a file size and key up front. The metadata manager reserves a contiguous disk extent of the appropriate size class and returns a write handle. The writer uses the handle to write file data, then calls publish() on the handle. At publish time, the manager checks for key conflicts; if none, the extent is marked allocated and the file becomes visible to readers and immutable. The caller can subsequently look up the key and receive the extent information.

**Why this priority**: This is the fundamental operation — the reserve/write/publish lifecycle is the core contract of the file system. It validates the in-memory index, extent allocation bitmaps, write handle semantics, and deferred conflict detection.

**Independent Test**: Can be fully tested by reserving an extent, publishing it, then looking it up by key and verifying the returned offset and size match. Delivers the minimal viable metadata manager.

**Acceptance Scenarios**:

1. **Given** an initialized metadata manager with available space, **When** a caller reserves an extent with a 64-bit key and a size, **Then** the manager returns a write handle backed by a contiguous disk extent of the appropriate size class.
2. **Given** a write handle for key K, **When** the writer calls publish(), **Then** the extent is marked allocated in the index, the file is visible to readers, and the file is immutable from that point forward.
3. **Given** a published file with key K, **When** a caller looks up key K, **Then** the manager returns the extent descriptor with the correct offset and size.
4. **Given** two concurrent writers both reserving extents with the same key K, **When** the first writer publishes successfully and the second writer subsequently calls publish(), **Then** the second publish returns a duplicate-key error and the second writer's reserved extent is freed.

---

### User Story 2 - Abort a Reservation (Priority: P2)

A writer that has reserved an extent decides not to complete the file. The writer calls abort() on the write handle. The metadata manager frees the reserved extent back to the appropriate size-class pool. No trace of the reservation remains in the index.

**Why this priority**: Abort is essential for error handling and any workflow where a writer cannot complete. Without it, failed writes permanently leak disk space.

**Independent Test**: Can be tested by reserving an extent, aborting it, and verifying that the space is reclaimed by successfully reserving another extent in the same size class.

**Acceptance Scenarios**:

1. **Given** a write handle for an uncommitted reservation, **When** the writer calls abort(), **Then** the reserved extent is returned to the free pool and no entry is added to the index.
2. **Given** a write handle that has been aborted, **When** a caller looks up the key that was reserved, **Then** the manager returns key-not-found.

---

### User Story 3 - Remove a Published File (Priority: P3)

A caller removes a published file by its 64-bit key. The metadata manager frees the associated disk extent back into the appropriate size-class pool and removes the key from the in-memory index.

**Why this priority**: Deallocation completes the full lifecycle (reserve, publish, remove) and is required for any workload that recycles storage.

**Independent Test**: Can be tested by publishing an extent, removing it, verifying that a subsequent lookup returns key-not-found, and then reserving a new extent to confirm the freed space is reusable.

**Acceptance Scenarios**:

1. **Given** a published extent for key K, **When** a caller removes key K, **Then** the key is no longer found in the index and the disk blocks are returned to the free pool.
2. **Given** no published extent exists for key K, **When** a caller attempts to remove key K, **Then** the manager returns a key-not-found error.
3. **Given** a full size-class pool that was subsequently freed by removal, **When** a caller reserves a new extent of that size class, **Then** the reservation succeeds using reclaimed space.

---

### User Story 4 - Checkpoint Metadata to Disk (Priority: P4)

In-memory changes are not persisted until a checkpoint is performed. A caller may request a synchronous checkpoint; after it returns, all changes made up to that point are durable on disk. In addition, a background task periodically invokes the checkpoint function (default: every 5 seconds, configurable at runtime). The checkpoint writes the full index to a new location on disk, then atomically updates a superblock (4 KiB at disk offset zero) to point to the new index location.

**Why this priority**: Checkpointing is the durability mechanism. It is separated from stories P1–P3 because the in-memory operations work independently of persistence; checkpointing turns them durable.

**Independent Test**: Can be tested by publishing several extents, calling checkpoint, simulating a restart, and verifying all published extents are recovered.

**Acceptance Scenarios**:

1. **Given** one or more published or removed extents since the last checkpoint, **When** a caller requests a synchronous checkpoint, **Then** the full index is written to a new disk location and the superblock is updated to reference it; after the call returns, all prior changes are durable.
2. **Given** an idle system with no changes since the last checkpoint, **When** the background checkpoint task fires, **Then** it detects no changes and skips the write (or writes an identical checkpoint) without error.
3. **Given** a checkpoint that has been written, **When** the superblock is read, **Then** it contains a valid CRC32 checksum, the magic number, format version, configured block size, the location of the current index, the location of the previous index, disk size, and an incremented checkpoint sequence number.

---

### User Story 5 - Initialize and Recover Metadata from Disk (Priority: P5)

On startup, the metadata manager reads the superblock at disk offset zero, locates the most recent valid index checkpoint, and rebuilds the in-memory index and allocation bitmaps. If the primary index location is damaged, the manager falls back to the previous index location recorded in the superblock. If the device is fresh (no valid superblock), the manager initializes clean structures.

**Why this priority**: Recovery depends on the checkpoint mechanism (P4) to have produced on-disk state to recover from.

**Independent Test**: Can be tested by publishing extents, checkpointing, reinitializing from disk, and verifying all checkpointed extents are present. Additionally, corrupt the primary index and verify fallback to the previous index.

**Acceptance Scenarios**:

1. **Given** a device with no valid superblock (fresh format), **When** the manager initializes, **Then** the in-memory index is empty and all extents are marked free.
2. **Given** a device with a valid superblock and index, **When** the manager initializes, **Then** all previously checkpointed key-to-extent mappings are present and the allocation bitmaps match.
3. **Given** a device whose primary index location is corrupted but the previous index location is intact, **When** the manager initializes, **Then** it falls back to the previous index and recovers to that checkpoint's state.
4. **Given** a device whose superblock has an invalid magic number, **When** the manager attempts to initialize, **Then** it returns a corrupt-metadata error.

---

### User Story 6 - Enumerate All Allocated Extents (Priority: P6)

A caller iterates over all published extents without needing to know the keys in advance. This supports bulk operations such as integrity checks, migration, and diagnostics.

**Why this priority**: Enumeration is a supporting capability needed by higher-level tooling but is not required for basic file system operation.

**Independent Test**: Can be tested by publishing a known set of extents, enumerating, and confirming the returned set matches exactly.

**Acceptance Scenarios**:

1. **Given** N published extents, **When** a caller enumerates all extents, **Then** exactly N extent descriptors are returned, each matching a previously published key.
2. **Given** an empty metadata manager (no extents), **When** a caller enumerates, **Then** zero results are returned without error.
3. **Given** extents that are reserved but not yet published, **When** a caller enumerates, **Then** the reserved-but-unpublished extents are not included in the results.

---

### Edge Cases

- What happens when all extents of a given size class are exhausted? The manager returns an out-of-space error on reserve.
- What happens when the caller provides a key of 0 or `u64::MAX`? These are valid keys in the flat namespace; no key value is reserved.
- What happens if a block I/O write fails during a checkpoint? The checkpoint returns an I/O error; the superblock is not updated, so the previous checkpoint remains the durable state. In-memory state is unaffected.
- What happens if multiple threads concurrently reserve extents with different keys? Reservations proceed concurrently since conflict detection is deferred to publish time.
- What happens if a writer never calls publish() or abort() on a write handle? The handle's drop/destructor should free the reservation (equivalent to abort).
- What happens when a file size does not match any existing size class? A new size class is created dynamically to accommodate that size.
- What happens if the background checkpoint fires while a synchronous checkpoint is already in progress? Only one checkpoint runs at a time; the background task waits or skips.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The component MUST implement the `IExtentManagerV2` interface using the `define_component!` macro from the component framework.
- **FR-002**: The component MUST declare a receptacle for the `IBlockDevice` interface; all disk I/O MUST go through this receptacle's connected provider.
- **FR-003**: All I/O buffers used with the block device MUST be DMA-compatible (allocated via the DMA allocator provided by the SPDK environment).
- **FR-004**: Files MUST be identified by 64-bit unsigned integer keys in a single flat namespace with no hierarchical directory structure.
- **FR-005**: Each key MUST map to exactly one contiguous disk extent.
- **FR-006**: The component MUST support allocation from extent size classes that are established dynamically — when a file is created with a size not matching any existing size class, a new size class is created automatically. There is no cap on the number of distinct size classes.
- **FR-006a**: All extent sizes MUST be aligned to a block size that is configurable at format time. The block size is established when the device is first formatted and recorded in the superblock.
- **FR-007**: The full in-memory index (key-to-extent mapping) and extent allocation bitmaps MUST fit in main memory.
- **FR-008**: A caller MUST declare the file size up front when reserving an extent. The component returns a write handle for the reserved extent.
- **FR-009**: A write handle MUST support two terminal operations: publish() and abort(). No other transition is valid after either is called.
- **FR-010**: On publish(), the extent MUST be marked allocated in the in-memory index and the file MUST become visible to readers and immutable. Key conflict detection MUST occur at publish time.
- **FR-011**: On abort(), the reserved extent MUST be freed back to the appropriate size-class pool with no trace in the index.
- **FR-012**: If a write handle is dropped without calling publish() or abort(), it MUST behave as if abort() was called.
- **FR-013**: Published files MUST be immutable; no modification of a published file's data or metadata is permitted.
- **FR-014**: In-memory changes MUST NOT be persisted to disk until a checkpoint is performed.
- **FR-015**: The component MUST provide a synchronous checkpoint function; after it returns, all changes made prior to the call MUST be durable on disk.
- **FR-016**: A background task MUST invoke the checkpoint function periodically. The default interval is 5 seconds, configurable at runtime via a component parameter.
- **FR-017**: The checkpoint MUST write the full index to a new location on disk, then atomically update a superblock to reference the new index location.
- **FR-018**: The superblock MUST be 4 KiB in size, located at disk offset zero, and contain at minimum: a magic number, a format version (u32), the configured block size, the location of the current index on disk, the location of the previous index on disk, and the disk size.
- **FR-019**: On initialization, the component MUST read the superblock, locate the most recent valid index, and rebuild the in-memory state. If the primary index is damaged, it MUST fall back to the previous index.
- **FR-020**: The component MUST detect corruption of the superblock or index and report a clear error rather than loading inconsistent state. Corruption detection MUST use CRC32 checksums: one CRC32 covering the superblock, and per-chunk CRC32s on the index checkpoint data, where a chunk is the metadata chunk size configured at format time (e.g., 128 KiB).
- **FR-021**: The component MUST guarantee crash consistency back to the most recent successfully completed checkpoint.
- **FR-022**: The component MUST return an out-of-space error when no free extents of the requested size class remain.
- **FR-023**: The component MUST support concurrent access from multiple threads with no data races or partial-state visibility.
- **FR-024**: Only one checkpoint MUST execute at a time; concurrent checkpoint requests MUST be serialized.
- **FR-025**: The component MUST emit structured log events at key state transitions: checkpoint start/complete, recovery start/complete, errors (I/O failures, corruption detected), and space exhaustion per size class.

### Key Entities

- **ExtentKey**: A 64-bit unsigned integer uniquely identifying a file in the flat namespace.
- **Extent**: A descriptor containing the key, byte offset on disk, and size in bytes.
- **Write Handle**: A handle returned on reservation that allows the writer to write file data and then either publish() or abort() the file. Dropping without calling either is equivalent to abort.
- **Size Class**: An extent size (e.g., 4 KiB, 64 KiB, 1 MiB) established dynamically when a file of that size is first created; must be a multiple of the format-time configured block size. Each size class has its own allocation bitmap. There is no cap on the number of distinct size classes.
- **Allocation Bitmap**: A per-size-class bit vector tracking which extents are free, reserved, or allocated. Held entirely in memory.
- **Index**: An in-memory map from ExtentKey to Extent, containing only published (not reserved) entries. Reconstructable from the on-disk checkpoint.
- **Superblock**: A 4 KiB structure at disk offset zero containing the magic number, format version (u32), configured block size, location of the current index checkpoint, location of the previous index checkpoint, disk size, default slab size, maximum element size, metadata chunk size, checkpoint sequence number, and CRC32 checksum.
- **Checkpoint**: A complete snapshot of the index written to a new disk location, made durable by updating the superblock. The previous index location is retained for recovery. Index checkpoint data is written in fixed-size metadata chunks (e.g., 128 KiB), each protected by a CRC32 checksum.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A reserve-then-publish-then-lookup round trip completes in under 10 microseconds for the in-memory path (excluding disk I/O).
- **SC-002**: The metadata manager supports at least 1 million published extents without degradation in lookup latency beyond 2x the single-extent baseline.
- **SC-003**: After crash or restart, 100% of extents published before the most recent successful checkpoint are recoverable with no data loss.
- **SC-004**: Concurrent reserve, publish, abort, and remove operations from 8 threads produce no data races, deadlocks, or inconsistent state (verified by stress tests under thread sanitizer).
- **SC-005**: All public APIs have unit tests, doc tests, and the component passes `cargo test --all` with zero failures.
- **SC-006**: All performance-sensitive APIs have Criterion benchmarks demonstrating measured throughput and latency.

## Assumptions

- The block I/O component (SPDK-based `IBlockDevice`) is available and connected via the component framework's receptacle/binding mechanism before the metadata manager is initialized.
- The DMA allocator function is provided by the SPDK environment and set on the metadata manager before initialization.
- The underlying block device provides sufficient capacity for both the metadata region and the data extents; capacity planning is outside this component's scope.
- The metadata manager does not handle data I/O to file contents — it only manages the mapping metadata. Data reads and writes to file extents are performed by higher-level components using the write handle and extent descriptors returned by this component.
- Key conflicts are expected to be rare; deferring detection to publish time is acceptable.

## Clarifications

### Session 2026-04-20

- Q: Should the superblock include a format version field for forward compatibility? → A: Yes, include a version field (u32) in the superblock.
- Q: Must extent sizes be aligned to the block device's block size, and is there a maximum number of distinct size classes? → A: Extents must be aligned to a format-time configurable block size (recorded in superblock); no cap on size class count.
- Q: What should the background checkpoint interval be? → A: Default 5 seconds, configurable at runtime via a component parameter.
- Q: What corruption detection mechanism should be used? → A: CRC32 on superblock; per-chunk CRC32 on index checkpoint data, where a chunk is the metadata chunk size (e.g., 128 KiB).
- Q: What level of observability should the metadata manager provide? → A: Structured log events at key transitions (checkpoint start/complete, recovery, errors, space exhaustion).
