# block-device-spdk-nvme

High-performance NVMe block device component using SPDK for direct userspace NVMe controller access. Part of the Certus project.

## Architecture

### Thread Model

Each component instance runs a single **actor thread** that owns the NVMe controller and all SPDK resources. The actor is pinned to the first CPU on the controller's NUMA node to ensure cache/memory locality with the NVMe device.

The actor uses a **self-polling loop** with adaptive parking:

1. **Hot path**: `try_recv()` on the control channel + `poll_clients()` on every iteration (no blocking).
2. **Idle park**: After 10M consecutive empty polls, the actor calls `thread::park_timeout(10ms)`. MPSC senders automatically unpark the actor when a new control message arrives.
3. **Timeout throttle**: `check_timeouts()` runs at most once per millisecond (not every poll) to avoid overhead from `Instant::now()` and `Vec` allocation in the hot path.

Client threads should be pinned to **different** NUMA-local cores to avoid CFS time-slicing contention with the actor. The `iops-benchmark` tool demonstrates this pattern using `component_core::numa::{NumaTopology, CpuSet, set_thread_affinity}`.

```
┌─────────────┐     SPSC ingress      ┌──────────────────────────┐
│  Client 0   │ ───── Command ──────>  │                          │
│  (CPU 1)    │ <── Completion ──────  │    Actor Thread (CPU 0)  │
└─────────────┘     SPSC callback      │                          │
                                       │  - poll_clients()        │
┌─────────────┐     SPSC ingress      │  - dispatch to SPDK      │
│  Client 1   │ ───── Command ──────>  │  - process completions   │
│  (CPU 2)    │ <── Completion ──────  │  - check timeouts (~1ms) │
└─────────────┘     SPSC callback      │                          │
                                       │  ┌──── NVMe Controller ──┐
        ...                            │  │  QP0 (depth 4)        │
                                       │  │  QP1 (depth 16)       │
┌─────────────┐     SPSC ingress      │  │  QP2 (depth 64)       │
│  Client N   │ ───── Command ──────>  │  │  QP3 (depth 256)      │
│  (CPU N+1)  │ <── Completion ──────  │  └────────────────────────┘
└─────────────┘     SPSC callback      └──────────────────────────┘
```

### Client Channels

Each client gets two shared-memory SPSC channel pairs via `connect_client()`:

- **Ingress channel** (client -> actor): `Command` messages (read, write, probe, etc.)
- **Callback channel** (actor -> client): `Completion` notifications

The actor drains all client ingress channels on every poll iteration, interleaved with SPDK completion processing. Disconnected clients (sender dropped) are detected and removed automatically.

### NVMe Queue Pair Pool

The actor allocates multiple IO queue pairs at standard depths (`[4, 16, 64, 256]`) to exploit NVMe hardware parallelism. Queue pair selection uses a **shallowest-fit** heuristic:

| Batch size | Selected QP | Rationale |
|------------|-------------|-----------|
| 1          | QP0 (4)     | Minimal latency for single ops |
| 5-16       | QP1 (16)    | Small batches |
| 17-64      | QP2 (64)    | Medium batches |
| 65+        | QP3 (256)   | High-throughput bulk IO |

The selection considers in-flight operations: if a shallow queue is full, the next deeper queue is chosen. This avoids head-of-line blocking while keeping small IOs on low-latency queues.

### Key Interfaces

| Interface | Role | Description |
|-----------|------|-------------|
| `IBlockDevice` | Provided | Client connection, device info, IO operations, telemetry |
| `ILogger` | Receptacle | Debug logging via dependency injection |
| `ISPDKEnv` | Receptacle | SPDK environment initialization |

### Messaging API

- **Sync IO**: `ReadSync`, `WriteSync` — actor busy-polls a single queue pair until NVMe completion
- **Async IO**: `ReadAsync`, `WriteAsync` — SPDK async submission with timeout; completions routed back via callback channel
- **Admin**: `NsProbe`, `NsCreate`, `NsFormat`, `NsDelete`, `ControllerReset`
- **Batch**: `BatchSubmit` — multiple operations in a single message
- **Other**: `WriteZeros`, `AbortOp`

### DMA Buffers

IO operations use `DmaBuffer` (SPDK DMA-safe memory). Reads take `Arc<Mutex<DmaBuffer>>`, writes take `Arc<DmaBuffer>`. Since clients are in-process, `Arc` references avoid copies.

### Telemetry

Enable with `--features telemetry` to collect IO latency (min/max/mean), operation count, and throughput statistics.

## Prerequisites

- Linux host with hugepages configured and IOMMU enabled
- NVMe device bound to VFIO/UIO (`deps/spdk/scripts/setup.sh`)
- SPDK built at `deps/spdk-build/` (run `deps/build_spdk.sh`)
- Rust stable toolchain (edition 2021, MSRV 1.75+)

## Build

```bash
cargo build -p block-device-spdk-nvme-v2

# With telemetry support
cargo build -p block-device-spdk-nvme-v2 --features telemetry
```

## Tests

```bash
# All tests (unit + integration)
cargo test -p block-device-spdk-nvme-v2

# Integration tests only
cargo test -p block-device-spdk-nvme-v2 --test integration
```

### Unit Tests

Unit tests run without SPDK hardware and cover:

| Module | Tests |
|--------|-------|
| `lib.rs` | Component metadata, receptacles, pre-init defaults, telemetry feature gate |
| `controller.rs` | NVMe version display/equality, namespace info capacity/clone |
| `namespace.rs` | Namespace ID validation, LBA range checks, type conversions |
| `qpair.rs` | Queue pair accounting, pool selection heuristics (shallow/medium/deep), in-flight pressure |
| `telemetry.rs` | Snapshot defaults, record/snapshot arithmetic (feature-gated) |
| `actor.rs` | PendingOp struct fields |

### Integration Tests

Integration tests in `tests/integration.rs` include both non-hardware and hardware tests. Hardware tests self-skip when SPDK is unavailable:

**Non-hardware:**
- Component wiring, interface queries, pre-init device info, error handling

**Hardware (require NVMe bound to VFIO):**
- `initialize_with_hardware` — device info populated after init
- `device_info_after_initialize` — block size, max transfer, NUMA node
- `namespace_probe` — discover namespaces, validate sector info
- `write_read_roundtrip` — sync write/read with data integrity check
- `sync_write_async_read_roundtrip` — async read path verification
- `write_on_one_client_read_on_another` — cross-client data visibility
- `multi_thread_concurrent_io` — 4 threads, 16 ops each, write+read+verify at non-overlapping LBAs

## Benchmarks

Benchmarks use [Criterion](https://github.com/bheisler/criterion.rs) and self-skip when hardware is unavailable.

```bash
# Run all benchmarks
cargo bench -p block-device-spdk-nvme-v2

# Run a specific benchmark
cargo bench -p block-device-spdk-nvme-v2 --bench latency
cargo bench -p block-device-spdk-nvme-v2 --bench throughput
```

### Benchmark Suites

**`latency`** — measures per-operation latency:
- `qpair_selection` (no hardware): pool selection for batch sizes 1/4/16/64
- `command_construction` (no hardware): `Command::WriteZeros` construction cost
- `sync_io_latency` (hardware): `ReadSync`/`WriteSync` 4KB at varying queue depths

**`throughput`** — measures batch IO throughput:
- `batch_construction` (no hardware): batch command assembly
- `qpair_selection_throughput` (no hardware): selection throughput at batch sizes 1/8/32/128
- `batch_write_throughput` (hardware): batched `WriteSync` at sizes 1/8/32/128, measured in bytes/sec

## Source Layout

```
src/
  lib.rs          Component definition, connect_client(), IBlockDevice impl
  actor.rs        BlockDeviceHandler, command dispatch, async completions
  command.rs      ControlMessage, ClientSession (internal types)
  controller.rs   NvmeController safe wrapper
  namespace.rs    Namespace operations and validation
  qpair.rs        QueuePair, QueuePairPool, depth-based selection
  telemetry.rs    TelemetryStats (feature-gated)
tests/
  integration.rs  Hardware-conditional integration tests
benches/
  latency.rs      Per-operation latency benchmarks
  throughput.rs   Batch IO throughput benchmarks
```

## CI Gate

All must pass before merge:

```bash
cargo fmt --check && cargo clippy -- -D warnings && cargo test --all && cargo doc --no-deps && cargo bench --no-run
```
