# Research: Metadata Manager Component

**Branch**: `001-metadata-manager` | **Date**: 2026-04-20 | **Spec**: [spec.md](spec.md)

## Phase 0 Research Decisions

No NEEDS CLARIFICATION items remain in the Technical Context — the user provided all design decisions directly. This document records the research-level decisions and alternatives considered.

### R-001: Allocator Architecture — Two-Level (Buddy + Slab)

**Decision**: Use a binary buddy allocator (lower level) to manage raw disk space in power-of-two blocks, and slab allocators (upper level) to efficiently allocate many same-sized file extents within each slab.

**Rationale**: The workload expects a small number of distinct file sizes with many files per size class. Slab allocation gives O(1) allocate/free via bitmaps with excellent spatial locality for same-sized objects. The buddy allocator handles the variable-sized slab allocations efficiently (O(log n) splits/merges) without external fragmentation.

**Alternatives Considered**:
- **Single-level buddy for all files**: High internal fragmentation for non-power-of-two file sizes. Each file wastes up to 50% of its buddy block.
- **Single free list / first-fit**: O(n) scan for large extent pools. Fragmentation grows over time with no coalescence mechanism.
- **Extent tree (e.g., B-tree of free ranges)**: More complex, better for highly variable sizes. Overkill when the number of distinct size classes is small (<20).

---

### R-002: In-Memory Only Bitmaps and Buddy State

**Decision**: Slab allocation bitmaps, roving indices, and buddy allocator data structures are maintained in memory only — never persisted to disk. They are rebuilt at recovery time from the persisted index and slab list.

**Rationale**: The index is the single source of truth for allocated extents. Writing bitmaps to disk on every allocate/free would add write amplification with no durability gain — the bitmaps can be reconstructed deterministically. Recovery cost is bounded because the full index fits in memory.

**Alternatives Considered**:
- **Persist bitmaps to disk**: Faster recovery (skip bitmap rebuild), but adds I/O on every allocate/free. Write amplification is unacceptable for high-throughput allocation.
- **WAL for bitmap changes**: Lower write amplification than full bitmap persistence, but adds log management complexity for marginal recovery speed benefit.

---

### R-003: Full-Index Checkpoint (Not Incremental)

**Decision**: Each checkpoint writes the complete index and slab list to a new disk location, then atomically updates the superblock.

**Rationale**: The index fits in memory by design. Serialization cost is bounded and proportional to the number of published extents, which is known to be manageable (millions of entries at ~20 bytes each ≈ tens of MB). Full-index checkpoints are simple, correct, and atomic — no log compaction or journal replay needed.

**Alternatives Considered**:
- **Write-ahead log (WAL) with periodic compaction**: More complex; requires log replay on recovery and periodic compaction to bound log growth. Marginal benefit when the full index is memory-sized.
- **Incremental / dirty-page checkpointing**: Requires tracking dirty entries; adds bookkeeping overhead. Risk of subtle bugs in incremental state management.

---

### R-004: Deferred Key Conflict Detection (at Publish Time)

**Decision**: Key uniqueness is checked at publish() time, not at reserve() time.

**Rationale**: Key conflicts are expected to be rare. Deferring the check avoids taking a lock on the index during the reserve hot path, which is the most contention-sensitive operation. At publish time, a single HashMap lookup (O(1) expected) detects duplicates.

**Alternatives Considered**:
- **Eager check at reserve**: Provides earlier error feedback but requires index read-lock on every reserve. Under concurrent load, this becomes a bottleneck for a condition that almost never triggers.
- **Optimistic check + retry**: Similar to deferred but with an automatic retry loop. Adds complexity; the caller is better positioned to decide retry policy.

---

### R-005: Doubly-Linked Metadata Chunks

**Decision**: Metadata chunks are organized as a doubly-linked list with forward and backward LBA pointers, CRC32 checksums, and sequence numbers.

**Rationale**: Back-links enable chain integrity validation during recovery — the forward and backward chains must be mutually consistent. Without back-links, a corrupted forward pointer could silently truncate the chain.

**Alternatives Considered**:
- **Singly-linked list**: Simpler but provides no integrity validation for the chain structure beyond CRC of individual chunks.
- **Array/table of chunk LBAs in superblock**: Limits the number of chunks to what fits in the superblock; doesn't scale well for large indexes.

---

### R-006: IExtentManager Interface Extension

**Decision**: The v2 metadata manager will need an extended version of the `IExtentManager` interface to support the reserve/publish/abort write model and checkpoint operations. The existing interface's `create_extent` maps to the v0 synchronous model; v2 splits this into `reserve_extent` → `WriteHandle` → `publish()`/`abort()`.

**Rationale**: The existing interface lacks: (1) write handles with lifecycle semantics, (2) explicit checkpoint/recover methods, (3) format-time initialization with configurable parameters.

**Approach**: Define the new interface contract in `contracts/public-api.md`. At implementation time, either extend `IExtentManager` with new methods or define a new `IExtentManagerV2` interface. The exact approach will be finalized during the contracts phase, but a new interface is likely cleaner to avoid backward-compatibility constraints.

---

### R-007: DMA Buffer Management

**Decision**: All I/O buffers for block device operations must be DMA-compatible, allocated via the `DmaAllocFn` provided by the SPDK environment and set on the component via `set_dma_alloc`.

**Rationale**: SPDK NVMe drivers require DMA-accessible memory for zero-copy I/O. Using standard heap allocations would fail at the driver level.

**Approach**: The component will store the DMA allocator function and use it for all checkpoint I/O buffers (superblock reads/writes, metadata chunk reads/writes). The `DmaBuffer` type from the interfaces crate wraps the raw allocation.

---

### R-008: Concurrency Model

**Decision**: Use `RwLock<ManagerState>` for the main component state, with read locks for lookups/enumerations and write locks for mutations (reserve, publish, abort, remove). Checkpoint uses a write-then-downgrade strategy: acquire the write lock briefly to allocate metadata chunks for the new checkpoint, then atomically downgrade to a read lock for serialization and I/O.

**Rationale**: The component framework's `define_component!` requires the component struct to be `Send + Sync`. An `RwLock` allows concurrent readers (lookup, enumerate) while serializing writers. The write-then-downgrade checkpoint strategy keeps the write lock held only for the brief chunk allocation phase; the expensive serialization and disk I/O proceeds under a read lock, so lookups are not blocked during checkpoint I/O. Writers (reserve, publish, abort, remove) block during checkpoint I/O, which is acceptable since checkpoint serializes them anyway (FR-024). Because the full index may be too large to copy into a separate buffer, the read lock is held over I/O rather than snapshot-and-release. Atomic downgrade requires `parking_lot::RwLock` (std `RwLock` does not support downgrade at MSRV 1.75).

**Alternatives Considered**:
- **Mutex-only**: Simpler but blocks readers during writes. Lookups are expected to be frequent; RwLock avoids unnecessary contention.
- **Write lock for entire checkpoint**: Blocks both readers and writers during all checkpoint I/O. Unacceptable latency for lookups at scale.
- **Snapshot-and-release (copy index under write lock, release, write to disk)**: Avoids holding any lock during I/O, but doubles memory usage for the index. At 1M extents (~20 MB), this may not be affordable.
- **Lock-free index (e.g., concurrent HashMap)**: Higher complexity. The `RwLock<HashMap>` with downgrade is sufficient given the performance targets.
- **Actor model with message passing**: Consistent with the block device component's pattern but adds latency from channel serialization. Overkill when the shared state is a simple in-memory map.

---

### R-009: Slab Reclamation

**Decision**: When all slots in a slab become free (bitmap is all zeros), the slab is removed from the slab list and its region is returned to the buddy allocator for reuse.

**Rationale**: Without reclamation, a workload that creates and deletes files of varying sizes accumulates dead slabs — their disk regions are locked to one size class even though all slots are free. Returning empty slabs to the buddy allows the space to be reused by any size class, preventing long-term fragmentation under churn.

**Implementation**: On each `abort()` or `remove_extent()` that frees a slot, check if the containing slab's bitmap is all zeros. If so, remove the slab from the size-class slab list and call `buddy.free(slab.start_lba, slab.slab_size)`. The check is O(bitmap_words) which is bounded and fast.

**Alternatives Considered**:
- **No reclamation (permanent slabs)**: Simpler but leaks usable disk space under churn across size classes.
- **Deferred reclamation (batch during checkpoint)**: Delays the free, adding complexity to checkpoint. Immediate reclamation is simpler and keeps space available sooner.

---

### R-010: Non-Power-of-Two Buddy Initialization

**Decision**: Initialize the buddy allocator by decomposing the usable disk size (total minus superblock) into a sum of decreasing power-of-two blocks. Each block is placed on the appropriate free list. Blocks from the tail decomposition have no right buddy and can never merge upward.

**Rationale**: A standard buddy allocator rounds down to the largest power of two, wasting the remainder. Decomposing the remainder into smaller power-of-two blocks (corresponding to the '1' bits in the binary representation of the block count) recovers all usable space. The only cost is that tail blocks cannot coalesce past their natural boundary, which the merge logic handles by checking whether the buddy address falls within the allocator's total range.

**Implementation**: Compute `usable_blocks = (disk_size - superblock_size) / block_size`. For each set bit at position `k` in `usable_blocks`, place a free block of size `block_size * 2^k` at the appropriate offset on free_list[k]. The merge operation adds a bounds check: `if buddy_offset >= total_usable_size { do not merge }`.

**Alternatives Considered**:
- **Round down to power of two**: Simple but wastes up to 50% of the last order's coverage on unlucky sizes.
- **Separate small-block free list for remainder**: Adds a second allocator; the decomposition approach is simpler and uses the same buddy machinery.
