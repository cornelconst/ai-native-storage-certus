# Data Model: Extent Manager Benchmark Application

## Entities

### BenchmarkConfig

Captures all runtime parameters parsed from CLI arguments.

| Field | Type | Description | Validation |
|-------|------|-------------|------------|
| device | PciAddress | NVMe device PCI address | Must be valid PCI BDF format |
| ns_id | u32 | NVMe namespace ID | >= 1 |
| threads | usize | Number of worker threads | >= 1 |
| count | u64 | Operations per phase | >= 1 |
| size_class | u32 | Extent size class in bytes | 128KiB-5MiB, multiple of 4KiB |
| slab_size | u32 | Slab size in bytes | >= 8KiB, multiple of 4KiB |
| total_size | Option<u64> | Total managed space in bytes | None = auto-detect from device |

### LatencyStats

Computed from a sorted vector of per-operation durations.

| Field | Type | Description |
|-------|------|-------------|
| count | u64 | Number of operations measured |
| min | Duration | Minimum latency |
| max | Duration | Maximum latency |
| mean | Duration | Average latency |
| p50 | Duration | Median latency |
| p99 | Duration | 99th percentile latency |

### PhaseResult

Captures results of one benchmark phase (create, lookup, or remove).

| Field | Type | Description |
|-------|------|-------------|
| phase_name | String | "create", "lookup", or "remove" |
| total_ops | u64 | Total operations completed |
| elapsed | Duration | Wall-clock time for the phase |
| ops_per_sec | f64 | Aggregate throughput |
| latency | LatencyStats | Aggregate latency statistics |
| per_thread | Vec<WorkerResult> | Per-thread breakdown |

### WorkerResult

Per-thread statistics for one phase.

| Field | Type | Description |
|-------|------|-------------|
| thread_id | usize | Worker thread index (0-based) |
| ops_completed | u64 | Operations this thread completed |
| latencies | Vec<Duration> | Raw per-operation latency samples |

## Relationships

```
BenchmarkConfig --[configures]--> Benchmark Run
Benchmark Run --[produces]--> 3x PhaseResult (create, lookup, remove)
PhaseResult --[contains]--> N x WorkerResult (one per thread)
WorkerResult --[aggregated into]--> LatencyStats
```

## State Transitions

```
CLI Parse → Config Validation → SPDK Init → Component Wiring → 
  Initialize Extent Manager → 
  Create Phase → Lookup Phase → Remove Phase → 
  Report Results → Cleanup
```

Error at any stage prints diagnostics and exits with non-zero status, except mid-phase errors which report partial results and continue to the next phase.
