# Tasks: Metadata Manager V2

**Input**: Design documents from `/specs/001-metadata-manager/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/public-api.md, quickstart.md

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: Which user story this task belongs to (US1–US6)
- Paths relative to repository root (`components/extent-manager/v2/`)

---

## Phase 1: Setup

**Purpose**: Create the crate skeleton and declare all dependencies.

- [X] T001 Create project directory structure: `src/`, `tests/`, `benches/` per plan.md source code layout
- [ ] T002 Create `Cargo.toml` with dependencies: component-core, component-macros, component-framework, interfaces (features = ["spdk"]), crc32fast, parking_lot; dev-dependencies: criterion (features = ["html_reports"]); configure `[[bench]]` for benchmarks.rs with `harness = false`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core building-block modules that ALL user stories depend on. No user story work can begin until this phase is complete.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [ ] T003 Define `IExtentManagerV2` interface in `interfaces/src/iextent_manager_v2.rs`: use `define_interface!` macro to declare trait with methods `set_dma_alloc`, `format`, `initialize`, `reserve_extent`, `lookup_extent`, `get_extents`, `for_each_extent`, `remove_extent`, `checkpoint` (NOTE: `set_checkpoint_interval` is an inherent method on the component, NOT a trait method — implement it in T011). Define `FormatParams` struct (slab_size, max_element_size, chunk_size, block_size — all u32). Define `WriteHandle` struct with fields (key, offset, size) and FnOnce closures for publish/abort, accessors (`key()`, `extent_offset()`, `extent_size()`), `publish(self) -> Result<Extent, ExtentManagerError>`, `abort(self)`, and `Drop` impl that calls abort. Gate behind `#[cfg(feature = "spdk")]`. Export from `interfaces/src/lib.rs`
- [ ] T004 [P] Implement error helpers in `src/error.rs`: convenience functions `duplicate_key(k)`, `key_not_found(k)`, `out_of_space()`, `not_initialized(msg)`, `io_error(msg)`, `corrupt_metadata(msg)` returning `ExtentManagerError` variants. Adapt pattern from extent-manager/v0 `src/error.rs`
- [ ] T005 [P] Implement `AllocationBitmap` in `src/bitmap.rs`: u64-word bit vector with `new(num_slots)`, `set(idx)`, `clear(idx)`, `is_set(idx) -> bool`, `find_free_from(start) -> Option<usize>` (roving search), `is_all_free() -> bool` (for slab reclamation check per R-009), `count_set() -> usize`. Include unit tests inline (`#[cfg(test)]` module). Adapt from extent-manager/v0 `src/bitmap.rs`
- [ ] T006 [P] Implement `BuddyAllocator` in `src/buddy.rs`: `new(total_usable_size, block_size)` with non-power-of-two decomposition per R-010 (iterate set bits of `usable_size / block_size`, place each on appropriate free list). `alloc(size) -> Option<u64>` (find smallest sufficient order, split if needed). `free(offset, size)` (merge with buddy if buddy exists within bounds — `buddy_offset < total_usable_size`). `mark_allocated(offset, size)` (for recovery rebuild). Include unit tests inline: power-of-two size, non-power-of-two size, alloc/free/merge, tail block no-merge, exhaust and reclaim
- [ ] T007 [P] Implement `Superblock` in `src/superblock.rs`: struct with fields per data-model.md (magic, version, disk_size, current_index_lba, previous_index_lba, block_size, slab_size, max_element_size, chunk_size, checkpoint_seq, checksum). Constants: `SUPERBLOCK_SIZE = 4096`, `SUPERBLOCK_MAGIC`. `serialize(&self) -> Vec<u8>` (write fields in order, compute CRC32 over all fields except checksum, append checksum, pad to 4096). `deserialize(buf: &[u8]) -> Result<Self, ExtentManagerError>` (validate size, read fields, verify CRC32, verify magic). Include unit tests inline: round-trip serialize/deserialize, corrupt CRC detected, invalid magic detected
- [ ] T008 [P] Implement `BlockDeviceClient` in `src/block_io.rs`: wraps `ClientChannels` + `DmaAllocFn`. `new(channels, alloc_fn)`. `write_blocks(lba, data: &[u8]) -> Result<(), ExtentManagerError>` (allocate DmaBuffer, copy data, send WriteSync command, await WriteDone completion). `read_blocks(lba, num_bytes) -> Result<DmaBuffer, ExtentManagerError>` (allocate DmaBuffer, send ReadSync, await ReadDone). Convert channel errors to `ExtentManagerError::IoError`. Adapt from extent-manager/v0 `src/block_device.rs`
- [ ] T009 Implement `Slab` and size-class management in `src/slab.rs`: `Slab` struct (start_offset: u64, slab_size: u32, element_size: u32, bitmap: AllocationBitmap, rover: usize). `alloc_slot(&mut self) -> Option<(usize, u64)>` (find free via bitmap rover, set bit, return slot index and byte offset). `free_slot(&mut self, slot_index: usize)` (clear bit). `is_empty(&self) -> bool` (delegate to bitmap.is_all_free()). `slot_offset(slot_index) -> u64` (start_offset + slot_index * element_size). `SizeClassManager` struct wrapping `HashMap<u32, Vec<usize>>` mapping element_size to slab indices. Include unit tests inline: alloc/free round trip, exhaust all slots, rover wraps, is_empty after full free. Depends on T005, T006
- [ ] T010 Implement `ManagerState` in `src/state.rs`: struct containing `index: HashMap<ExtentKey, Extent>`, `slabs: Vec<Slab>`, `size_classes: SizeClassManager`, `buddy: BuddyAllocator`, `format_params: FormatParams`, `dirty: bool`, `checkpoint_seq: u64`. Methods: `alloc_extent(element_size) -> Result<(usize, usize, u64), ExtentManagerError>` (find slab with free slot or allocate new slab from buddy, alloc slot). `free_slot(slab_idx, slot_idx)` (free bitmap slot, check slab reclamation per R-009 — if slab empty, remove from size_classes and slabs, buddy.free the region). `insert_extent(key, extent) -> Result<(), ExtentManagerError>` (duplicate check, insert, mark dirty). `remove_extent(key) -> Result<(usize, usize), ExtentManagerError>` (find slab/slot for key's extent, remove from index, return indices for free_slot). `new_for_testing(total_size, block_size, slab_size, max_element_size) -> Self` (construct with buddy and empty index for unit tests without disk I/O). Depends on T009
- [ ] T011 Create component skeleton in `src/lib.rs`: `define_component!` with `provides: [IExtentManagerV2]`, `receptacles: { block_device: IBlockDevice, logger: ILogger }`, `fields: { state: Arc<parking_lot::RwLock<ManagerState>>, dma_alloc: Mutex<Option<DmaAllocFn>>, checkpoint_interval: AtomicU64, checkpoint_thread: Mutex<Option<JoinHandle<()>>> }`. Implement `set_dma_alloc` (store in Mutex). Implement `set_checkpoint_interval` as an inherent method (not a trait method) — store duration as millis in AtomicU64. Stub remaining IExtentManagerV2 methods with `todo!()`. Re-export public modules. Depends on T003, T010

**Checkpoint**: All building blocks compiled. User story implementation can begin.

---

## Phase 3: User Story 1 — Reserve, Write, and Publish a File (Priority: P1) 🎯 MVP

**Goal**: A writer reserves an extent by key and size, writes file data, publishes the file making it visible and immutable, then looks it up by key.

**Independent Test**: Reserve an extent, publish it, look it up by key, verify offset and size match.

### Implementation for User Story 1

- [ ] T012 [US1] Implement `reserve_extent` in `src/lib.rs`: acquire write lock on state, call `state.alloc_extent(size)` to get (slab_idx, slot_idx, offset), release lock, construct `WriteHandle` with publish and abort closures capturing `Arc<RwLock<ManagerState>>`. Publish closure: write-lock state, check duplicate key in index, if duplicate free slot and return `DuplicateKey`, otherwise insert extent and mark dirty. Abort closure: write-lock state, call `state.free_slot(slab_idx, slot_idx)`. Return `WriteHandle`
- [ ] T013 [US1] Implement `lookup_extent` in `src/lib.rs`: acquire read lock on state, look up key in index HashMap, return `Extent` clone or `KeyNotFound` error
- [ ] T014 [US1] Unit tests for reserve-publish-lookup round trip in `tests/lifecycle.rs`: create component with test-only state (via `ManagerState::new_for_testing`), reserve extent with key 42 and size 4096, verify handle accessors (key, offset, size), publish, lookup key 42, verify offset and size match. Test multiple distinct keys. Test duplicate key at publish: reserve two handles with same key, publish first succeeds, publish second returns `DuplicateKey` and slot is freed
- [ ] T015 [US1] Unit test for `OutOfSpace` in `tests/lifecycle.rs`: create state with minimal disk space (one slab worth), exhaust all slots via reserve, verify next reserve returns `OutOfSpace`

**Checkpoint**: Reserve → publish → lookup works. Minimal viable metadata manager.

---

## Phase 4: User Story 2 — Abort a Reservation (Priority: P2)

**Goal**: A writer aborts a reservation, freeing the extent back to the pool with no trace in the index.

**Independent Test**: Reserve an extent, abort it, verify space reclaimed by reserving again in the same size class.

### Implementation for User Story 2

- [ ] T016 [US2] Unit tests for explicit abort in `tests/lifecycle.rs`: reserve extent with key 99, call `abort()`, lookup key 99 returns `KeyNotFound`. Reserve another extent of same size, verify it succeeds (space reclaimed)
- [ ] T017 [US2] Unit tests for drop-as-abort in `tests/lifecycle.rs`: reserve extent, drop handle without calling publish or abort, verify key not in index, verify slot freed (re-reserve succeeds)

**Checkpoint**: Abort path verified. WriteHandle RAII contract complete.

---

## Phase 5: User Story 3 — Remove a Published File (Priority: P3)

**Goal**: Remove a published file by key, freeing its extent and reclaiming the slab if empty.

**Independent Test**: Publish an extent, remove it, verify key-not-found, reserve a new extent to confirm space reuse.

### Implementation for User Story 3

- [ ] T018 [US3] Implement `remove_extent` in `src/lib.rs`: acquire write lock on state, call `state.remove_extent(key)` to get slab/slot indices, call `state.free_slot(slab_idx, slot_idx)` (which handles slab reclamation per R-009), mark dirty
- [ ] T019 [US3] Unit tests for remove in `tests/lifecycle.rs`: publish key K, remove K, lookup K returns `KeyNotFound`. Remove non-existent key returns `KeyNotFound`. Publish and remove all extents in a slab, verify slab reclaimed (buddy has the space back — reserve a different size class to verify)
- [ ] T020 [US3] Unit test for full lifecycle in `tests/lifecycle.rs`: reserve → publish → lookup → remove → re-reserve cycle with the same key, verifying each step

**Checkpoint**: Full in-memory lifecycle (reserve/publish/abort/remove/lookup) works. All in-memory user stories complete.

---

## Phase 6: User Story 4 — Checkpoint Metadata to Disk (Priority: P4)

**Goal**: Persist the in-memory index to disk via CRC32-protected metadata chunk chains and superblock update.

**Independent Test**: Publish several extents, call checkpoint, read back the superblock and verify it points to a valid chunk chain containing the expected entries.

### Implementation for User Story 4

- [ ] T021 [US4] Implement `MetadataChunk` in `src/checkpoint.rs`: header struct per data-model.md (magic, seq, prev_lba, next_lba, payload_len, checksum). Constants: `CHUNK_MAGIC`. `serialize_header(&self) -> Vec<u8>`. `deserialize_header(buf: &[u8]) -> Result<Self, ExtentManagerError>`. `compute_checksum(header_bytes, payload) -> u32` (CRC32 over header fields except checksum + payload). `validate_checksum(buf: &[u8]) -> Result<(), ExtentManagerError>`. Serialization format for index entries: `(key: u64, offset: u64, size: u32)` packed sequentially. Slab list entries: `(start_offset: u64, slab_size: u32, element_size: u32)` packed after index entries. Include unit tests: header round-trip, CRC validation, corrupt detection
- [ ] T022 [US4] Implement checkpoint serialization in `src/checkpoint.rs`: `serialize_index_and_slabs(state: &ManagerState, chunk_size: u32) -> Vec<Vec<u8>>` — serialize all index entries and slab descriptors into a sequence of chunk payloads, each fitting within `chunk_size - header_size`. Return vector of serialized chunks (header + payload + CRC32)
- [ ] T023 [US4] Implement checkpoint write in `src/checkpoint.rs`: `write_checkpoint(client: &BlockDeviceClient, state: &ManagerState, superblock: &mut Superblock) -> Result<(), ExtentManagerError>` — (1) allocate metadata chunk slots from state's metadata slab (under write lock, then downgrade per R-008), (2) serialize index + slab list into chunk payloads under read lock, (3) link chunks as doubly-linked list (set prev_lba/next_lba), (4) write each chunk to disk via client, (5) update superblock (previous_index_lba = old current, current_index_lba = new head, increment checkpoint_seq), (6) write superblock to LBA 0, (7) free old fallback chain's chunks. On I/O failure: do not update superblock, free pre-allocated chunks in memory
- [ ] T024 [US4] Implement `checkpoint()` on the component in `src/lib.rs`: acquire checkpoint mutex (FR-024 serialization), check dirty flag (skip if clean), call `write_checkpoint`, clear dirty flag. Wire into `IExtentManagerV2::checkpoint`
- [ ] T025 [US4] Implement `set_checkpoint_interval` and background checkpoint thread in `src/lib.rs`: `set_checkpoint_interval` stores duration in `AtomicU64` (as millis). Background thread: spawned during `initialize()` or `format()`, loops with `thread::sleep(interval)`, calls `self.checkpoint()`, logs start/complete events. Thread stopped on component drop (use a shutdown flag or channel). Default interval: 5 seconds per FR-016
- [ ] T026 [US4] Unit tests for checkpoint in `tests/checkpoint.rs`: requires mock or in-memory block device. Publish N extents, call checkpoint, read back superblock, verify magic/version/seq/CRC. Read chunk chain from current_index_lba, verify all N extents present. Checkpoint with no changes (dirty=false), verify no disk write. Two sequential checkpoints: verify previous_index_lba points to first checkpoint's chain

**Checkpoint**: Checkpoint writes durable metadata to disk. Superblock is the atomic commit point.

---

## Phase 7: User Story 5 — Initialize and Recover Metadata from Disk (Priority: P5)

**Goal**: On startup, read the superblock, recover the index from the checkpoint chain, and rebuild all in-memory state. Fall back to the previous checkpoint if the primary is corrupt.

**Independent Test**: Publish extents, checkpoint, reinitialize from disk, verify all extents present. Corrupt primary chain, verify fallback recovery.

### Implementation for User Story 5

- [ ] T027 [US5] Implement `format()` in `src/lib.rs`: validate FormatParams (block_size > 0, slab_size multiple of block_size, max_element_size <= slab_size, chunk_size multiple of block_size). Connect to block device via receptacle, get device size. Initialize BuddyAllocator with (device_size - SUPERBLOCK_SIZE, block_size). Allocate initial metadata slab from buddy. Build initial Superblock (magic, version=1, params, current_index_lba=0, previous_index_lba=0, checkpoint_seq=0). Write superblock to LBA 0 via BlockDeviceClient. Store ManagerState in Arc<RwLock>
- [ ] T028 [US5] Implement recovery in `src/recovery.rs`: `recover(client: &BlockDeviceClient) -> Result<(Superblock, HashMap<ExtentKey, Extent>, Vec<SlabDescriptor>), ExtentManagerError>` — read 4096 bytes at LBA 0, deserialize Superblock (validate CRC32, magic, version). Walk primary chain (`current_index_lba`): read each chunk, validate chunk CRC32 and seq matches superblock checkpoint_seq, follow next_lba links, collect payload. If any chunk fails: log `recovery_fallback` event, walk `previous_index_lba` chain with `checkpoint_seq - 1`. If both fail: return `CorruptMetadata`. Deserialize collected payload into index entries and slab descriptors
- [ ] T029 [US5] Implement `initialize()` in `src/lib.rs`: call `recover()` to get superblock + index + slab list. Rebuild BuddyAllocator: init all space as free (non-power-of-two decomposition), then `mark_allocated` for each slab. Rebuild Slab bitmaps: for each slab, create AllocationBitmap, mark slots as allocated for extents whose offset falls within that slab. Store ManagerState. Start background checkpoint thread. Log `recovery_complete` event
- [ ] T030 [US5] Unit tests for recovery in `tests/checkpoint.rs`: (1) format fresh device, initialize, verify empty index and all space free. (2) publish 100 extents, checkpoint, drop component, create new component, initialize, verify all 100 extents recoverable via lookup. (3) publish + checkpoint, publish more (not checkpointed), re-initialize, verify only checkpointed extents present. (4) corrupt primary chain (overwrite first chunk with zeros), initialize, verify fallback to previous chain. (5) superblock with invalid magic → `CorruptMetadata` error

**Checkpoint**: Full durability lifecycle works: format → use → checkpoint → crash → recover.

---

## Phase 8: User Story 6 — Enumerate All Allocated Extents (Priority: P6)

**Goal**: Iterate over all published extents without knowing keys in advance.

**Independent Test**: Publish a known set of extents, enumerate, confirm the returned set matches exactly.

### Implementation for User Story 6

- [ ] T031 [US6] Implement `get_extents` and `for_each_extent` in `src/lib.rs`: `get_extents`: read-lock state, collect index values into Vec<Extent>, return. `for_each_extent`: read-lock state, iterate index values calling callback
- [ ] T032 [US6] Unit tests for enumeration in `tests/lifecycle.rs`: publish N extents with known keys, `get_extents`, verify exactly N returned with matching keys. Empty manager returns empty vec. Reserved-but-unpublished extents excluded from results. `for_each_extent` visits same set

**Checkpoint**: All six user stories complete.

---

## Phase 9: Polish & Cross-Cutting Concerns

**Purpose**: Documentation, benchmarks, logging, stress tests, and CI gate validation.

- [ ] T033 [P] Add doc comments (`///`) with runnable doc tests to all public types and methods across `src/bitmap.rs`, `src/buddy.rs`, `src/slab.rs`, `src/superblock.rs`, `src/write_handle.rs`, `src/state.rs`, `src/checkpoint.rs`, `src/recovery.rs`, `src/block_io.rs`, `src/error.rs`, `src/lib.rs`. Each public item must have purpose, parameters, return value, errors, and at least one `/// # Examples` block per constitution §IV. Additionally, add crate-level `//!` documentation in `src/lib.rs` with a quick-start guide sufficient for a new contributor to build and run the test suite (constitution §IV requirement)
- [ ] T034 [P] Implement structured logging (FR-025) in `src/lib.rs`, `src/checkpoint.rs`, `src/recovery.rs`: emit log events per the observable behavior table in contracts/public-api.md (`checkpoint_start`, `checkpoint_complete`, `recovery_start`, `recovery_complete`, `recovery_fallback`, `io_error`, `corruption_detected`, `space_exhaustion`). Use the `ILogger` receptacle declared in T011 (consistent with v0 pattern: `self.logger.get()` for the logger instance)
- [ ] T035 [P] Create Criterion benchmarks in `benches/benchmarks.rs`: benchmark `reserve_extent` throughput (single-threaded, 10K iterations), `publish` latency, `lookup_extent` latency (1 key, 1K keys, 1M keys for SC-002 validation), `remove_extent` latency, `checkpoint` latency (100 extents, 10K extents). Verify SC-001 (<10 µs in-memory round trip) and SC-002 (<2x degradation at 1M)
- [ ] T036 [P] Multi-threaded stress tests in `tests/concurrent.rs`: 8 threads performing concurrent reserve/publish/abort/remove/lookup operations (SC-004). Verify no panics, no data races, no deadlocks. Run with `RUSTFLAGS="-Z sanitizer=thread"` on nightly for thread sanitizer verification. Test concurrent checkpoint + mutations (writers block, readers proceed)
- [ ] T037 [P] Edge case tests in `tests/edge_cases.rs`: key=0 and key=u64::MAX are valid (FR-004 edge). Out-of-space returns error, no panic. Dynamic size class creation (new size triggers new slab). Checkpoint I/O error leaves previous checkpoint valid. Background checkpoint skip when not dirty. Drop component with outstanding WriteHandles (abort triggered). Multiple sequential checkpoints (previous_index_lba chain rotates correctly)
- [ ] T038 Validate all CI gates pass: `cargo fmt --check && cargo clippy -- -D warnings && cargo test --all && cargo doc --no-deps` with zero failures per constitution CI gate

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — start immediately
- **Foundational (Phase 2)**: Depends on Phase 1 — BLOCKS all user stories
- **US1 (Phase 3)**: Depends on Phase 2 — MVP target
- **US2 (Phase 4)**: Depends on US1 (WriteHandle type and reserve_extent)
- **US3 (Phase 5)**: Depends on US1 (needs published extents to remove)
- **US4 (Phase 6)**: Depends on US1 (needs extents to checkpoint); uses block_io.rs, superblock.rs from Phase 2
- **US5 (Phase 7)**: Depends on US4 (needs checkpoint to have on-disk state to recover)
- **US6 (Phase 8)**: Depends on US1 (needs published extents to enumerate)
- **Polish (Phase 9)**: Depends on all user stories complete

### User Story Independence

| Story | Can Start After | Independent of |
|-------|----------------|----------------|
| US1 (P1) | Phase 2 | All others |
| US2 (P2) | US1 | US3, US4, US5, US6 |
| US3 (P3) | US1 | US2, US4, US5, US6 |
| US4 (P4) | US1 | US2, US3, US6 |
| US5 (P5) | US4 | US2, US3, US6 |
| US6 (P6) | US1 | US2, US3, US4, US5 |

After US1 completes, US2, US3, US4, and US6 can proceed in parallel.

### Parallel Opportunities Within Phases

**Phase 2** (maximum parallelism):
```
T003 ─────────────────────────────────┐
T004 [P] ──┐                          │
T005 [P] ──┤                          │
T006 [P] ──┤── T009 ── T010 ── T011 ──┘
T007 [P] ──┘
T008 [P] ──┘
```

**Phase 6** (US4 — sequential checkpoint pipeline):
```
T021 → T022 → T023 → T024 → T025
                               └── T026 (tests)
```

**Phase 9** (all [P] except final gate):
```
T033 [P] ─┐
T034 [P] ─┤
T035 [P] ─┼── T038 (final validation)
T036 [P] ─┤
T037 [P] ─┘
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational
3. Complete Phase 3: User Story 1 (reserve → publish → lookup)
4. **STOP and VALIDATE**: Test US1 independently — all in-memory operations work
5. This delivers a minimal viable metadata manager (no persistence)

### Incremental Delivery

1. Setup + Foundational → building blocks ready
2. US1 → in-memory reserve/publish/lookup → MVP
3. US2 → abort + RAII verified → error handling complete
4. US3 → remove + slab reclamation → full in-memory lifecycle
5. US4 → checkpoint to disk → durability
6. US5 → format + recovery → crash consistency
7. US6 → enumeration → supporting capability
8. Polish → docs, benchmarks, logging, stress tests → production ready

### Key Implementation Notes

- **In-memory first**: US1–US3 and US6 are pure in-memory operations testable without a block device. Use `ManagerState::new_for_testing()` for unit tests
- **Disk I/O deferred**: Block device interaction begins in US4. Tests in US4–US5 need a mock or in-memory block device implementation
- **parking_lot**: Required for `RwLockWriteGuard::downgrade()` in checkpoint (R-008). std `RwLock` does not support atomic downgrade at MSRV 1.75
- **WriteHandle closures**: Closures capture `Arc<parking_lot::RwLock<ManagerState>>` — not `&self` — to avoid self-referential lifetime issues
- **Slab reclamation (R-009)**: Checked on every `free_slot` call. O(bitmap_words) per check — bounded and fast

---

## Notes

- Constitution requires: unit tests, doc tests, Criterion benchmarks, clippy clean, cargo doc clean
- [P] tasks = different files, no dependencies on incomplete tasks within same phase
- [Story] labels map to spec.md user stories (US1=P1 through US6=P6)
- Commit after each task or logical group
- Stop at any checkpoint to validate the story independently
