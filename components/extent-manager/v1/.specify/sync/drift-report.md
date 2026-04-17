# Spec Drift Report

Generated: 2026-04-16
Project: extent-manager v1

## Summary

| Category | Count |
|----------|-------|
| Specs Analyzed | 2 |
| Requirements Checked | 31 (16 + 15) |
| Aligned | 23 (74%) |
| Drifted | 5 (16%) |
| Not Implemented | 3 (10%) |
| Unspecced Code | 1 |

## Detailed Findings

### Spec: 001-extent-management - Extent Management

#### Aligned

- FR-001: Create extents with key, size, filename, CRC -> `IExtentManager::create_extent(key, size_class, filename, data_crc, has_crc)` at `src/lib.rs:200`
- FR-002: Allocate contiguous disk space from size class -> Two-phase write allocates slot from slab at `src/lib.rs:220-232`
- FR-003: Persist metadata to block device -> Record block written at `src/lib.rs:256-259`, bitmap at `src/lib.rs:261-273`
- FR-004: Remove extents by key, freeing space -> `remove_extent` at `src/lib.rs:281`, clears bitmap and zeros record
- FR-005: Lookup extent metadata by key -> `lookup_extent` at `src/lib.rs:322`, returns serialized metadata from in-memory index
- FR-008: Crash consistency via atomic 4KiB writes -> Two-phase write: record first, then bitmap at `src/lib.rs:256-273`
- FR-009: Recovery from partial writes/corruption -> `recovery.rs` scans bitmap vs records, clears orphans
- FR-010: Thread-safe access -> `RwLock<Option<ExtentManagerState>>` at `src/lib.rs:36`; write lock for create/remove, read lock for lookup
- FR-011: Error types -> `ExtentManagerError` enum: `DuplicateKey`, `KeyNotFound`, `InvalidSizeClass`, `OutOfSpace` at `interfaces/src/iextent_manager.rs:11-18`
- FR-012: Fresh initialization and reopening -> `initialize()` at `src/lib.rs:114`, `open()` at `src/lib.rs:153`
- FR-013: Block device receptacle -> `block_device: IBlockDevice` in `define_component!` at `src/lib.rs:32`
- FR-016: Error propagation -> All I/O errors mapped via `error::nvme_to_em` and returned immediately, no retries

#### Drifted

- FR-007: Spec says "1 to 32 fixed size classes, configurable at initialization, up to 10M slots per class" but implementation uses dynamic slab-based allocation. `initialize(total_size_bytes, slab_size_bytes, ns_id)` accepts total device size and slab size — no pre-declared size classes or slot counts. Slabs allocated on-demand in `create_extent`. Max 256 slabs, not 32 size classes with 10M slots each.
  - Location: `src/lib.rs:114-151`, `interfaces/src/iblock_device.rs:459`
  - Severity: **major** — API signature and allocation model fundamentally changed

- FR-014: Spec says "System MUST use a logger receptacle for all console/diagnostic output" but the logger receptacle is declared (`src/lib.rs:33`) yet never read or used. The benchmark app cannot bind the logger due to type mismatch (`interfaces::ILogger` vs `example_logger::ILogger`). No logging output is produced by the extent manager.
  - Location: `src/lib.rs:33` (declared), no usage anywhere in `src/`
  - Severity: **moderate** — receptacle exists but is dead code

- SC-005: Spec says "at least 32 distinct size classes with up to 10M slots per class" but implementation supports max 256 slabs (dynamically allocated), each with ~262K slots per 1 GiB slab. Different model — higher flexibility but different capacity profile.
  - Location: `src/superblock.rs` (MAX_SLABS=256), `src/superblock.rs` compute_slab_layout
  - Severity: **moderate** — capacity model changed

#### Not Implemented

- FR-006: Iteration through all stored extents. The `IExtentManager` trait has `extent_count()` but no `iterate`/`iter`/`for_each` method. No way to enumerate all extents via the public interface. An `iterate_benchmark.rs` bench exists but likely uses test-internal access.
  - Severity: **major** — user story 4 (iterate all extents) has no public API

- FR-015: "Iteration performance MUST be sufficient for rebuilding in-memory indexes at startup." Without a public iteration API, this is untestable.
  - Severity: **major** — depends on FR-006

- SC-004: "Iterating all extents visits each extent exactly once." No public iteration API exists.
  - Severity: **major** — depends on FR-006

---

### Spec: 002-extent-benchmark - Extent Manager Benchmark Application

#### Aligned

- FR-001: Standalone binary at `apps/extent-benchmark/` -> `apps/extent-benchmark/Cargo.toml` exists, in workspace members
- FR-002: `--device <PCI_ADDRESS>` -> `config.rs:9` `pub device: String` with `#[arg(long)]`
- FR-003: `--ns-id` default 1 -> `config.rs:12` `default_value_t = 1`
- FR-004: `--threads` default 1 -> `config.rs:15` `default_value_t = 1`
- FR-005: `--count` default 10000 -> `config.rs:18` `default_value_t = 10000`
- FR-006: `--size-class` default 131072 -> `config.rs:21` `default_value_t = 131072`
- FR-007: `--slab-size` default 1073741824 -> `config.rs:28` `default_value_t = 1073741824`
- FR-008: `--total-size` auto-detect -> `config.rs:35` `Option<u64>`, auto-detection at `main.rs:117-127`
- FR-010: Three phases: create, lookup, remove in order -> `main.rs:160-182` single-threaded, `main.rs:190-225` multi-threaded
- FR-011: Disjoint key ranges -> `compute_key_ranges()` at `main.rs:228-237`
- FR-012: Per-phase stats: ops, elapsed, ops/sec, percentiles -> `report.rs:18-31`, `stats.rs` computes min/p50/p99/max
- FR-013: Multi-threaded per-thread + aggregate stats -> `report.rs:33-41` prints per-thread when `per_thread.len() > 1`
- FR-014: README.md -> `apps/extent-benchmark/README.md` with build, usage, prerequisites, CLI reference
- FR-015: Partial results on errors -> `run_create/run_lookup/run_remove` log errors per-op and continue, latency always recorded

#### Drifted

- FR-009: Spec says "wire full component stack: Logger, SPDKEnv, BlockDeviceSpdkNvme, ExtentManagerComponentV1" but logger is NOT bound to the extent manager due to type mismatch (`example_logger::ILogger` vs `interfaces::ILogger`). Logger is bound to spdk_env and block_dev only.
  - Location: `main.rs:34-48` — logger→extent_mgr binding was removed
  - Severity: **minor** — extent manager doesn't use logger anyway; functional behavior unaffected

- SC-003: "N threads complete approximately N times faster than 1 thread for lookup operations" — lookups are sub-microsecond (in-memory HashMap with read lock), but all operations hold a write lock for create/remove. The spec's claim about linear lookup scaling is implementable but not validated by the benchmark since lookup latency rounds to 0us at this resolution.
  - Location: benchmark output shows `p50=0 us` for lookups
  - Severity: **minor** — reporting artifact, not a correctness issue

#### Not Implemented

(None — all 15 FR requirements are implemented)

---

### Unspecced Code

| Feature | Location | Lines | Suggested Spec |
|---------|----------|-------|----------------|
| Dynamic slab allocation (`allocate_slab`) | `src/lib.rs:63-106` | 44 | Update 001-extent-management FR-007, US7 |

## Inter-Spec Conflicts

- **001 FR-007 vs implementation**: Spec describes static pre-declared size classes with slot counts at init time. Implementation uses dynamic slab allocation (total_size + slab_size at init, slabs carved on demand). The `IExtentManagerAdmin::initialize` signature change is the root divergence — spec says `(sizes: Vec<u32>, slots: Vec<u32>, ns_id)` but code says `(total_size_bytes: u64, slab_size_bytes: u32, ns_id: u32)`.

- **001 FR-014 vs 002 FR-009**: The extent manager declares a logger receptacle using `interfaces::ILogger`, but the actual `LoggerComponent` provides `example_logger::ILogger`. These are different types. This makes the logger receptacle un-bindable from external apps.

## Recommendations

1. **Update spec 001 FR-007, US7, SC-005**: Rewrite to reflect dynamic slab-based allocation model. Update entity definitions (remove "Size Class" as static config, add "Slab" entity). Update acceptance scenarios for `initialize(total_size, slab_size, ns_id)` signature.

2. **Implement FR-006 (iteration)**: Add an `iterate_extents` or `for_each_extent` method to `IExtentManager`. This blocks user story 4 and prevents FR-015/SC-004 from being testable.

3. **Resolve ILogger type mismatch**: Either change extent-manager to use `example_logger::ILogger`, or unify the interface definitions so there's one canonical `ILogger` trait.

4. **Update spec 001 edge cases**: Remove references to "maximum 32 size classes" and "10M slots per class." Replace with slab-model edge cases (max 256 slabs, slab exhaustion, multi-slab same class).

5. **Add mean latency to spec 002 FR-012**: Implementation reports mean latency (added during development) but spec only lists min/p50/p99/max.
