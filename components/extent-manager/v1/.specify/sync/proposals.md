# Drift Resolution Proposals

Generated: 2026-04-16
Based on: drift-report from 2026-04-16

## Summary

| Resolution Type | Count |
|-----------------|-------|
| Backfill (Code -> Spec) | 4 |
| Align (Spec -> Code) | 1 |
| Human Decision | 2 |
| New Specs | 0 |
| Remove from Spec | 0 |

---

## Proposal 1: 001-extent-management / FR-007

**Direction**: BACKFILL

**Current State**:
- Spec says: "System MUST support 1 to 32 fixed extent size classes, configurable at initialization time, ranging from 128KiB to 5MiB, where each size is a multiple of 4KiB. Each size class MUST support up to 10,000,000 slots."
- Code does: Dynamic slab-based allocation. `initialize(total_size_bytes, slab_size_bytes, ns_id)` accepts total device size and slab size. Slabs are allocated on-demand when `create_extent` requests a size class with no available slots. Max 256 slabs. Each slab serves one size class; multiple slabs can serve the same class (auto-grow).

**Proposed Resolution**:

Replace FR-007 with:

> **FR-007**: System MUST support dynamic slab-based allocation. At initialization, the caller specifies total managed space (in bytes) and slab size (in bytes, must be >= 8 KiB and a multiple of 4 KiB). Size classes are NOT pre-declared; the first `create_extent` call for a given size class dynamically allocates a slab. Each slab serves exactly one size class. When a slab is full, a new slab for that class is allocated automatically if space remains. Maximum 256 slabs. Valid size classes range from 128 KiB to 5 MiB and must be multiples of 4 KiB.

Also update User Story 7 title from "Initialization with Multiple Size Classes" to "Initialization with Slab-Based Allocation" and rewrite acceptance scenarios:

1. **Given** a block device and a total size of 100 GiB with 1 GiB slabs, **When** the manager is initialized, **Then** the superblock is written and the system is ready for operations with no pre-allocated slabs.
2. **Given** an initialized manager with no slabs, **When** a client creates the first extent with size=128 KiB, **Then** a new slab is allocated on-demand for that size class.
3. **Given** an initialized manager where a slab for 128 KiB is full, **When** a client creates another 128 KiB extent, **Then** a second slab is allocated for the same size class (auto-grow).
4. **Given** an initialized manager, **When** a client creates an extent with size < 128 KiB or > 5 MiB or not a multiple of 4 KiB, **Then** the system returns an appropriate error.
5. **Given** a previously initialized block device, **When** the manager is opened, **Then** existing slab table and metadata are loaded and all previously stored extents are accessible.

**Rationale**: The slab refactoring was an intentional design evolution requested by the user. The code is working, tested (30 integration tests pass), and benchmarked on real hardware. The spec should reflect the current architecture.

**Confidence**: HIGH

**Action**:
- [ ] Approve
- [ ] Reject
- [ ] Modify

---

## Proposal 2: 001-extent-management / SC-005

**Direction**: BACKFILL

**Current State**:
- Spec says: "The system supports at least 32 distinct size classes with up to 10,000,000 slots per class."
- Code does: Max 256 slabs (not size classes). A 1 GiB slab has ~262,136 slots. Total slot capacity depends on total device size and slab size. Multiple slabs per size class allowed.

**Proposed Resolution**:

Replace SC-005 with:

> **SC-005**: The system supports up to 256 dynamically allocated slabs across any mix of size classes. Each 1 GiB slab provides approximately 262,000 slots. Total capacity scales with total device size and slab configuration.

**Rationale**: Directly follows from FR-007 backfill. The slab model offers more flexibility (no need to pre-commit slot counts) at the cost of a different capacity profile.

**Confidence**: HIGH

**Action**:
- [ ] Approve
- [ ] Reject
- [ ] Modify

---

## Proposal 3: 001-extent-management / FR-014

**Direction**: HUMAN DECISION

**Current State**:
- Spec says: "System MUST use a logger receptacle for all console/diagnostic output."
- Code does: Logger receptacle is declared in `define_component!` but never read or used. Additionally, `interfaces::ILogger` and `example_logger::ILogger` are different types (different `TypeId`), making external binding impossible.

**Option A: Remove the logger receptacle** (Backfill)

Update FR-014 to:

> **FR-014**: ~~System MUST use a logger receptacle for all console/diagnostic output.~~ REMOVED — extent manager operates silently; errors are returned to the caller via Result types. No diagnostic output is produced.

Rationale: The component has no logging code. Error handling is purely via return values. Removing the unused receptacle simplifies the component API and eliminates the type mismatch issue.

Code change: Remove `logger: ILogger` from `define_component!` in `src/lib.rs:33`, remove `ILogger` import.

**Option B: Implement logging** (Align)

Keep FR-014 and add logging to the extent manager:
- Fix the type mismatch (use `example_logger::ILogger` or unify ILogger definitions)
- Add diagnostic logging for: slab allocation, recovery events, error conditions
- Bind the logger in the benchmark app

Rationale: Logging is useful for debugging production issues. The spec correctly identifies this as a need.

**Questions to resolve**:
1. Is diagnostic logging needed in the extent manager, or are Result errors sufficient?
2. If logging is needed, should the project unify `ILogger` into one definition?

**Confidence**: MEDIUM

**Action**:
- [ ] Option A: Remove logger receptacle
- [ ] Option B: Implement logging
- [ ] Modify

---

## Proposal 4: 001-extent-management / FR-006 (Not Implemented)

**Direction**: HUMAN DECISION

**Current State**:
- Spec says: "System MUST support iterating through all stored extents, visiting each extent exactly once. Iteration MUST hold an exclusive lock that blocks concurrent create and remove operations until iteration completes."
- Code does: `IExtentManager` trait has `extent_count()` but no iteration method. No public API to enumerate extents. An `iterate_benchmark.rs` bench exists but accesses internal state.

**Option A: Implement iteration API** (Align)

Add to `IExtentManager`:
```
fn iterate_extents(&self, callback: &dyn Fn(u64, &[u8]) -> bool) -> Result<u64, ExtentManagerError>;
```
Where callback receives (key, serialized_metadata) and returns true to continue. Returns count of extents visited. Implementation holds write lock for the duration.

Estimated effort: ~50 LOC in lib.rs + interface change + tests.

**Option B: Defer / remove from spec**

If iteration is not needed for the current use case (benchmark app doesn't use it), mark FR-006 as deferred and remove SC-004. The in-memory index is rebuilt during `open()` via direct bitmap/record scanning, not via a public iteration API.

**Questions to resolve**:
1. Do any current or planned consumers need to iterate all extents via the public API?
2. Is the `open()` recovery path sufficient for index rebuilding, making a public iteration API redundant?

**Confidence**: MEDIUM

**Action**:
- [ ] Option A: Implement iteration API
- [ ] Option B: Defer / remove from spec
- [ ] Modify

---

## Proposal 5: 001-extent-management / FR-015 (Not Implemented)

**Direction**: BACKFILL (contingent on Proposal 4)

**Current State**:
- Spec says: "Iteration performance MUST be sufficient for rebuilding in-memory indexes at startup."
- Code does: Index rebuilding happens inside `open()` by scanning all slab bitmaps and reading set records. This is not via a public iteration API but achieves the same goal.

**Proposed Resolution**:

Replace FR-015 with:

> **FR-015**: The `open()` operation MUST rebuild the in-memory extent index by scanning all slab bitmaps and reading valid records. Performance MUST be sufficient for startup of volumes with up to 256 slabs.

If Proposal 4 Option A is chosen (implement iteration), keep the original FR-015 and add the `open()` performance note.

**Rationale**: The spec's intent (fast startup rebuild) is satisfied by `open()`. The mechanism is different (internal scan vs public API) but the outcome is the same.

**Confidence**: HIGH

**Action**:
- [ ] Approve
- [ ] Reject
- [ ] Modify

---

## Proposal 6: 002-extent-benchmark / FR-009

**Direction**: BACKFILL

**Current State**:
- Spec says: "The application MUST wire up the full component stack: Logger, SPDKEnv, BlockDeviceSpdkNvme, and ExtentManagerComponentV1."
- Code does: Logger bound to SPDKEnv and BlockDeviceSpdkNvme. NOT bound to ExtentManagerComponentV1 due to `ILogger` type mismatch. The extent manager doesn't use its logger receptacle anyway.

**Proposed Resolution**:

Update FR-009 to:

> **FR-009**: The application MUST wire up the component stack: SPDKEnv, BlockDeviceSpdkNvme, and ExtentManagerComponentV1. Logger is bound to SPDKEnv and BlockDeviceSpdkNvme for SPDK diagnostic output. The ExtentManager's logger receptacle is left unbound as the component does not produce diagnostic output.

**Rationale**: The extent manager has no logging code. The type mismatch is a cross-cutting issue (see Proposal 3). The benchmark works correctly without the binding.

**Confidence**: HIGH

**Action**:
- [ ] Approve
- [ ] Reject
- [ ] Modify

---

## Proposal 7: 002-extent-benchmark / SC-003

**Direction**: BACKFILL

**Current State**:
- Spec says: "Multi-threaded mode correctly distributes work — N threads complete approximately N times faster than 1 thread for lookup operations (which use a read lock and should scale linearly)."
- Code does: Lookup operations complete in sub-microsecond time (in-memory HashMap), making wall-clock scaling measurement impractical at the default 10K operation count. The p50/p99 round to 0us.

**Proposed Resolution**:

Update SC-003 to:

> **SC-003**: Multi-threaded mode correctly distributes work — N threads each process their share of operations concurrently. For I/O-bound operations (create, remove), aggregate throughput improves with additional threads. For in-memory operations (lookup), per-operation latency remains sub-microsecond regardless of thread count.

**Rationale**: The original criterion assumed lookup would have measurable latency. In practice, lookups are pure in-memory HashMap reads (~150ns) that cannot demonstrate linear scaling at microsecond reporting resolution. The revised criterion is testable and matches actual behavior.

**Confidence**: HIGH

**Action**:
- [ ] Approve
- [ ] Reject
- [ ] Modify

---

## Proposal 8: 001-extent-management / Key Entities and Edge Cases

**Direction**: BACKFILL

**Current State**:
- Key Entities reference "Size Class" as a static config with fixed slot counts and "Allocation Slot" as pre-provisioned positions
- Edge cases reference "maximum number of size classes (32)" and "minimum (1 size class, 1 slot)"

**Proposed Resolution**:

Update Key Entities:

- **Size Class**: ~~A supported extent size configured at initialization.~~ -> A valid extent size (128 KiB to 5 MiB, multiples of 4 KiB). Size classes are not pre-declared; any valid size can be used in `create_extent` and will trigger slab allocation on demand.
- **Slab** (NEW): A contiguous region of device blocks serving one size class. Contains a bitmap region and a record region. Allocated dynamically when needed. Maximum 256 slabs.
- **Allocation Slot**: ~~A pre-provisioned position within a size class.~~ -> A position within a slab that can hold one extent record. Slot count per slab is determined by slab size.
- **Superblock**: ~~Contains size classes, slot counts.~~ -> Contains total device blocks, slab size, and a slab table mapping each slab to its size class and start LBA.

Update Edge Cases:
- ~~"maximum number of size classes (32)"~~ -> "maximum number of slabs (256)"
- ~~"minimum (1 size class, 1 slot)"~~ -> "minimum slab (2 blocks: 1 bitmap + 1 slot)"
- ADD: "What happens when a slab fills up and another slab for the same size class is allocated?"
- ADD: "What happens when all 256 slabs are allocated and no free space remains?"

Update Assumptions:
- ~~"Extent sizes and slot counts are immutable after initialization"~~ -> "Slab size is immutable after initialization; new slabs are allocated dynamically"

**Confidence**: HIGH

**Action**:
- [ ] Approve
- [ ] Reject
- [ ] Modify
