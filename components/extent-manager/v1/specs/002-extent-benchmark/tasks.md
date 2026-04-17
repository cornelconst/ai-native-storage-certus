# Tasks: Extent Manager Benchmark Application

**Input**: Design documents from `/specs/002-extent-benchmark/`
**Prerequisites**: plan.md (required), spec.md (required), research.md, data-model.md, contracts/cli.md, quickstart.md

**Tests**: Not requested — this is a benchmark tool, not a library component.

**Organization**: Tasks grouped by user story for independent implementation.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1, US2, US3)
- Exact file paths included in all descriptions

---

## Phase 1: Setup (Project Initialization)

**Purpose**: Create the binary crate and establish project structure

- [X] T001 Create binary crate directory and manifest at apps/extent-benchmark/Cargo.toml with dependencies: block-device-spdk-nvme, clap (derive), component-core, component-framework, example-logger, extent-manager, interfaces (spdk feature), spdk-env
- [X] T002 Add apps/extent-benchmark to workspace members in Cargo.toml (workspace root)
- [X] T003 Create source file stubs: apps/extent-benchmark/src/main.rs, config.rs, worker.rs, stats.rs, report.rs

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core modules that all user stories depend on

**CRITICAL**: No user story work can begin until this phase is complete

- [X] T004 Implement BenchmarkConfig struct with clap derive in apps/extent-benchmark/src/config.rs — fields: device (String, required), ns_id (u32, default 1), threads (usize, default 1), count (u64, default 10000), size_class (u32, default 131072), slab_size (u32, default 1073741824), total_size (Option<u64>, default None)
- [X] T005 [P] Implement LatencyStats struct and computation (from sorted Vec<Duration>) in apps/extent-benchmark/src/stats.rs — fields: count, min, max, mean, p50, p99; function compute_stats(samples: &mut Vec<Duration>) -> LatencyStats
- [X] T006 [P] Implement report formatting functions in apps/extent-benchmark/src/report.rs — print_header(config), print_phase(name, phase_result), print_summary(results); output format per contracts/cli.md
- [X] T007 Implement component wiring in apps/extent-benchmark/src/main.rs — create Logger, SPDKEnv, BlockDeviceSpdkNvme, ExtentManagerComponentV1 components; bind receptacles using component_core::binding::bind(); query ISPDKEnv and call init()/devices(); query IBlockDeviceAdmin and call set_pci_address()/initialize(); query IBlockDevice; query IExtentManagerAdmin and call set_dma_alloc()/initialize(total_size, slab_size, ns_id); detect total_size from IBlockDevice::num_sectors() * IBlockDevice::sector_size() if not specified

**Checkpoint**: Foundation ready — all modules stubbed and wiring works

---

## Phase 3: User Story 1 — Single-Threaded Benchmark (Priority: P1) MVP

**Goal**: Run create/lookup/remove phases sequentially on a single thread and report latency/throughput

**Independent Test**: Run `extent-benchmark --device <addr> --threads 1` and verify three phases complete with statistics output

### Implementation for User Story 1

- [X] T008 [US1] Implement run_create_phase function in apps/extent-benchmark/src/worker.rs — takes Arc<ExtentManagerComponentV1>, key_start, count, size_class; returns WorkerResult with Vec<Duration> latency samples; calls create_extent per key with Instant timing
- [X] T009 [US1] Implement run_lookup_phase function in apps/extent-benchmark/src/worker.rs — takes Arc<ExtentManagerComponentV1>, key_start, count; returns WorkerResult with Vec<Duration>; calls lookup_extent per key with Instant timing
- [X] T010 [US1] Implement run_remove_phase function in apps/extent-benchmark/src/worker.rs — takes Arc<ExtentManagerComponentV1>, key_start, count; returns WorkerResult with Vec<Duration>; calls remove_extent per key with Instant timing
- [X] T011 [US1] Implement PhaseResult aggregation in apps/extent-benchmark/src/worker.rs — struct PhaseResult { phase_name, total_ops, elapsed, ops_per_sec, latency: LatencyStats, per_thread: Vec<WorkerResult> }; function aggregate_results(phase_name, worker_results, elapsed) -> PhaseResult
- [X] T012 [US1] Wire single-threaded benchmark orchestration in apps/extent-benchmark/src/main.rs — after component wiring: run create phase (measure wall time), run lookup phase, run remove phase; call report functions for each PhaseResult; handle mid-phase errors gracefully (report partial results, continue to next phase)

**Checkpoint**: Single-threaded benchmark runs end-to-end with latency/throughput output

---

## Phase 4: User Story 2 — Multi-Threaded Scalability (Priority: P2)

**Goal**: Support --threads N with per-thread key ranges, barrier synchronization, and aggregate reporting

**Independent Test**: Run `extent-benchmark --device <addr> --threads 4` and verify per-thread + aggregate stats

### Implementation for User Story 2

- [X] T013 [US2] Implement key range partitioning in apps/extent-benchmark/src/worker.rs — function compute_key_ranges(total_count, num_threads) -> Vec<(u64, u64)> distributing keys evenly (handle remainder)
- [X] T014 [US2] Implement multi-threaded phase runner in apps/extent-benchmark/src/worker.rs — function run_phase_threaded(component: Arc<..>, phase_fn, key_ranges, barrier) -> (Vec<WorkerResult>, Duration); spawns N threads each running the phase function on their key range; Barrier at start for synchronized begin; collects results via channels or join handles
- [X] T015 [US2] Update main.rs orchestration to dispatch single-threaded or multi-threaded path based on config.threads in apps/extent-benchmark/src/main.rs — if threads == 1 use direct calls; if threads > 1 use run_phase_threaded for each phase
- [X] T016 [US2] Update report.rs to print per-thread latency breakdown in multi-threaded mode in apps/extent-benchmark/src/report.rs — for each thread: thread_id, ops, latency stats; then aggregate line

**Checkpoint**: Multi-threaded benchmark works with correct key distribution and aggregate stats

---

## Phase 5: User Story 3 — Configurable Parameters (Priority: P3)

**Goal**: All CLI options (size-class, slab-size, total-size) are respected and validated

**Independent Test**: Run with `--size-class 262144 --slab-size 2147483648` and verify initialization uses those values

### Implementation for User Story 3

- [X] T017 [US3] Add input validation in apps/extent-benchmark/src/config.rs — validate size_class (128KiB-5MiB, multiple of 4KiB), slab_size (>= 8KiB, multiple of 4KiB), total_size (if specified, > slab_size); print clear error messages and exit(1) on invalid input
- [X] T018 [US3] Wire configurable parameters into component initialization in apps/extent-benchmark/src/main.rs — pass config.slab_size, config.total_size (or auto-detected), config.size_class to extent manager initialize() and create_extent() calls
- [X] T019 [US3] Print configuration summary in report header in apps/extent-benchmark/src/report.rs — display device, ns_id, threads, count, size_class, slab_size, total_size in the header block

**Checkpoint**: All configurable parameters work correctly

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Documentation, code quality, final validation

- [X] T020 [P] Write README.md at apps/extent-benchmark/README.md — include: overview, prerequisites (hugepages, VFIO setup, SPDK), build instructions (cargo build -p extent-benchmark --release), usage examples (single-thread, multi-thread, custom params), CLI reference table, output format description
- [X] T021 [P] Add .gitignore for apps/extent-benchmark/ if needed (target/, *.log)
- [X] T022 Verify cargo build -p extent-benchmark compiles without errors
- [X] T023 Verify cargo clippy -p extent-benchmark -- -D warnings passes clean
- [X] T024 Verify cargo fmt -p extent-benchmark --check passes clean
- [ ] T025 Run quickstart.md validation scenarios (requires real NVMe hardware)

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — start immediately
- **Foundational (Phase 2)**: Depends on Setup (T001-T003)
- **US1 (Phase 3)**: Depends on Foundational (T004-T007)
- **US2 (Phase 4)**: Depends on US1 (T008-T012) — extends worker functions
- **US3 (Phase 5)**: Depends on Foundational (T004-T007) — can run in parallel with US2
- **Polish (Phase 6)**: Depends on all user stories complete

### User Story Dependencies

- **User Story 1 (P1)**: Foundational complete → can start. No other story dependencies.
- **User Story 2 (P2)**: Depends on US1 worker functions existing (extends them with threading).
- **User Story 3 (P3)**: Depends on Foundational only — adds validation and wiring, independent of US2.

### Within Each User Story

- Worker functions (T008-T010) can be developed in parallel [P]
- Aggregation (T011) depends on worker functions
- Main orchestration (T012) depends on all above

### Parallel Opportunities

- T005 (stats) and T006 (report) can run in parallel with T004 (config)
- T008, T009, T010 (worker functions) can run in parallel
- T020 (README) and T021 (gitignore) can run in parallel with any phase
- US3 (Phase 5) can run in parallel with US2 (Phase 4)

---

## Parallel Example: User Story 1

```bash
# Launch all worker functions together:
Task: "Implement run_create_phase in apps/extent-benchmark/src/worker.rs"
Task: "Implement run_lookup_phase in apps/extent-benchmark/src/worker.rs"
Task: "Implement run_remove_phase in apps/extent-benchmark/src/worker.rs"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup (T001-T003)
2. Complete Phase 2: Foundational (T004-T007)
3. Complete Phase 3: User Story 1 (T008-T012)
4. **STOP and VALIDATE**: Build and test single-threaded benchmark on real hardware
5. Deliver MVP — single-threaded benchmark with latency/throughput reporting

### Incremental Delivery

1. Setup + Foundational → Project compiles
2. Add User Story 1 → Single-threaded benchmark works (MVP)
3. Add User Story 2 → Multi-threaded scalability
4. Add User Story 3 → Full configurability
5. Polish → README, code quality, validation

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to specific user story
- All file paths are relative to workspace root (apps/extent-benchmark/)
- Component wiring pattern follows apps/iops-benchmark/src/main.rs as reference
- DMA allocator must use DmaBuffer::new() (SPDK), not heap_dma_alloc (test-only)
- Commit after each task or logical group
