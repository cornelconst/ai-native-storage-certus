# Research: Extent Manager Benchmark Application

## Decision 1: Latency Measurement Approach

**Decision**: Use `std::time::Instant::now()` per-operation with post-hoc percentile calculation from collected samples stored in a `Vec<Duration>`.

**Rationale**: This matches the pattern used in `iops-benchmark/src/worker.rs` and `stats.rs`. `Instant` provides monotonic timing on Linux with nanosecond resolution (via `clock_gettime(CLOCK_MONOTONIC)`). Per-operation timing is necessary for latency percentile reporting (min, p50, p99, max). Collecting all samples in a Vec and sorting post-hoc is simple and accurate for the expected operation counts (10K-1M).

**Alternatives considered**:
- HdrHistogram crate — adds an external dependency for marginal benefit at our scale; rejected per Principle 8 (minimal dependencies)
- Sampling (every Nth op) — loses tail latency accuracy; rejected

## Decision 2: CLI Argument Parsing

**Decision**: Use `clap` with derive macros, consistent with `iops-benchmark`.

**Rationale**: `clap` is already a workspace dependency used by `iops-benchmark`. Derive macros provide type-safe argument parsing with automatic help generation.

**Alternatives considered**:
- Manual argument parsing — error-prone, no help output
- `structopt` — deprecated in favor of clap derive

## Decision 3: Thread Coordination Pattern

**Decision**: Pre-compute disjoint key ranges per thread, then use `std::thread::spawn` with barrier synchronization at phase boundaries. Each thread gets its own `Arc<ExtentManagerComponentV1>` clone (shared reference to the single component). All threads share the single NVMe block device connection via the component's internal client creation.

**Rationale**: The extent manager's `create_extent`/`remove_extent` take a write lock internally, so threads contend at that lock — this is exactly what we want to measure. Key ranges are disjoint to avoid `DuplicateKey` errors. A `Barrier` at phase start ensures all threads begin simultaneously for accurate aggregate timing.

**Alternatives considered**:
- Per-thread block device connections — not needed; extent manager creates its own client internally
- Async runtime (tokio) — overkill for this use case; threads + barriers are simpler

## Decision 4: Report Format

**Decision**: Text output to stdout with structured sections per phase. Each section shows: operation name, total count, elapsed time, ops/sec, and latency percentiles table. Aggregate multi-thread summary at the end.

**Rationale**: Simple text output is easy to capture in CI logs, pipe to files, or parse with scripts. No need for JSON/CSV output in v1.

**Alternatives considered**:
- JSON output — could add as `--json` flag later; not needed for v1
- CSV — less readable for interactive use

## Decision 5: Component Wiring

**Decision**: Follow `iops-benchmark` wiring pattern exactly: Logger → SPDKEnv → BlockDeviceSpdkNvme → ExtentManagerComponentV1. Use `component_core::binding::bind()` for receptacle wiring and `component_core::iunknown::query::<dyn Trait>()` for interface access.

**Rationale**: This is the established pattern in the Certus codebase. The extent manager component is a consumer of `IBlockDevice`, so it needs the full SPDK stack underneath.

**Alternatives considered**: None — this is the only correct wiring pattern.
