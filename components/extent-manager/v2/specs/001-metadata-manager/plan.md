# Implementation Plan: Metadata Manager V2

**Branch**: `001-metadata-manager` | **Date**: 2026-04-20 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/001-metadata-manager/spec.md`

## Summary

The metadata manager component provides extent-based storage allocation for a flat-namespace, SPDK-backed file system. It implements a reserve/publish/abort write model with deferred key conflict detection, checkpoint-based durability via a CRC32-protected superblock and doubly-linked metadata chunk chains, and crash recovery with fallback to the previous checkpoint. The allocator uses a two-level architecture: a binary buddy allocator manages disk space, and per-size-class slab allocators provide O(1) extent allocation via bitmaps.

## Technical Context

**Language/Version**: Rust stable (MSRV 1.75)
**Primary Dependencies**: component-framework, component-core, component-macros, interfaces (spdk feature), crc32fast, parking_lot (atomic RwLock downgrade)
**Storage**: Block device via IBlockDevice (channel-based SPDK), DMA-compatible buffers via DmaAllocFn
**Testing**: cargo test (unit/integration/doc tests), Criterion benchmarks
**Target Platform**: Linux
**Project Type**: Library (component implementing IExtentManagerV2)
**Performance Goals**: <10 µs in-memory round trip (SC-001), 1M extents at <2x lookup degradation (SC-002)
**Constraints**: All I/O via DMA buffers, concurrent access from 8+ threads without data races (SC-004)
**Scale/Scope**: 1M+ published extents, dynamic size classes with no cap

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

### Pre-Design Gate

| Principle | Status | Evidence |
|-----------|--------|----------|
| I. Code Correctness First | ✅ Pass | Unit tests for all public APIs (SC-005). Doc tests on all public types (constitution §I). Unsafe limited to DMA buffer interop — justified by SPDK requirement (FR-003). Clippy clean (CI gate). |
| II. Comprehensive Testing | ✅ Pass | Unit, integration, doc tests required (SC-005). Stress tests under thread sanitizer (SC-004). TDD workflow. |
| III. Performance Accountability | ✅ Pass | Quantitative targets in spec (SC-001: <10 µs, SC-002: 1M extents). Criterion benchmarks required (SC-006). |
| IV. Documentation as Contract | ✅ Pass | Doc comments with runnable examples on all public types/functions. Quickstart guide in quickstart.md. cargo doc --no-deps clean. |
| V. Maintainability and Simplicity | ✅ Pass | Minimal API surface (IExtentManagerV2 — 10 methods). Justified dependencies: crc32fast (checksums), framework crates (required). Single-responsibility modules. |

**Gate Result**: PASS — no violations. Proceeding to Phase 0.

### Post-Design Gate

| Principle | Status | Evidence |
|-----------|--------|----------|
| I. Code Correctness First | ✅ Pass | WriteHandle RAII ensures no leaked reservations (FR-012). Deferred conflict detection is correct by design (R-004). CRC32 integrity on all on-disk structures (FR-020). |
| II. Comprehensive Testing | ✅ Pass | Test plan covers: lifecycle (reserve/publish/abort/remove), checkpoint/recovery, concurrent stress, corruption fallback, edge cases (out-of-space, duplicate key, drop-without-publish). |
| III. Performance Accountability | ✅ Pass | In-memory operations use HashMap (O(1) lookup) + bitmap (O(1) allocate). Checkpoint is bounded by index size (~20 bytes × N entries). Benchmarks target reserve/publish/lookup hot paths. |
| IV. Documentation as Contract | ✅ Pass | contracts/public-api.md defines full API contract. data-model.md defines all entities. quickstart.md provides runnable examples. |
| V. Maintainability and Simplicity | ✅ Pass | Two-level allocator (buddy + slab) is the simplest design that satisfies dynamic size classes with O(1) allocation. Full-index checkpoint avoids WAL complexity (R-003). RwLock with atomic downgrade avoids actor overhead while keeping lookups unblocked during checkpoint I/O (R-008). Slab reclamation (R-009) and non-power-of-two buddy (R-010) add bounded complexity for tangible resource efficiency. parking_lot justified for atomic downgrade (not available in std at MSRV 1.75). |

**Gate Result**: PASS — no violations.

### Key Design Decisions

- **R-001**: Two-level allocator (buddy + slab) for dynamic size classes with O(1) allocation
- **R-003**: Full-index checkpoint (not incremental/WAL) — simple, correct, bounded cost
- **R-005**: Doubly-linked metadata chunks with per-page CRC32 for integrity
- **R-006**: New `IExtentManagerV2` interface replacing v0's `IExtentManager`
- **R-008**: RwLock with write-then-downgrade checkpoint (write lock for chunk alloc, read lock for serialization + I/O)
- **R-009**: Slab reclamation — empty slabs returned to buddy allocator on free
- **R-010**: Non-power-of-two buddy initialization — tail space decomposed into smaller blocks

## Project Structure

### Documentation (this feature)

```text
specs/001-metadata-manager/
├── plan.md              # This file
├── research.md          # Phase 0: architectural decisions and alternatives
├── data-model.md        # Phase 1: entity definitions, state machines, on-disk formats
├── quickstart.md        # Phase 1: usage guide with code examples
├── contracts/
│   └── public-api.md    # Phase 1: IExtentManagerV2 interface contract
└── tasks.md             # Phase 2: implementation tasks (created by /speckit.tasks)
```

### Source Code (repository root)

```text
src/
├── lib.rs              # Component definition (define_component!), IExtentManagerV2 impl
├── bitmap.rs           # AllocationBitmap — per-slab bit vector (adapted from v0)
├── block_io.rs         # BlockDeviceClient — channel-based I/O wrapper (adapted from v0)
├── buddy.rs            # BuddyAllocator — binary buddy for slab allocation
├── checkpoint.rs       # Checkpoint write/read — serialize index to metadata chunk chain
├── error.rs            # Error helper functions
├── recovery.rs         # Initialize/recover — superblock read, chain traversal, state rebuild
├── slab.rs             # Slab — size class management, slot allocation/free
├── state.rs            # ManagerState — in-memory index, slab list, buddy, dirty tracking
├── superblock.rs       # Superblock — struct, serialize/deserialize, CRC32 validation
└── write_handle.rs     # WriteHandle — RAII type with publish/abort/Drop semantics

tests/
├── lifecycle.rs        # Reserve → publish → lookup → remove round trips
├── checkpoint.rs       # Checkpoint, recovery, fallback to previous index
├── concurrent.rs       # Multi-threaded stress tests (8+ threads, thread sanitizer)
└── edge_cases.rs       # Out-of-space, duplicate key, corrupt superblock, drop-without-publish

benches/
└── benchmarks.rs       # Criterion: reserve, publish, lookup, checkpoint latency
```

**Structure Decision**: Single-crate library component following the established pattern from extent-manager/v0 and block-device-spdk-nvme/v1. Modules map 1:1 to key entities in the data model for navigability.

## Complexity Tracking

No constitution violations to justify — all five principles pass cleanly.
