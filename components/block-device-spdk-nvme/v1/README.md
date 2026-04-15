# block-device-spdk-nvme

High-performance NVMe block device component using SPDK for direct userspace NVMe controller access. Part of the Certus project.

## Architecture

Each component instance runs a dedicated **actor thread** pinned to a NUMA-local core that polls all attached client channels. Clients communicate with the actor through per-client SPSC channel pairs:

- **Ingress channel** (client -> actor): `Command` messages (read, write, probe, etc.)
- **Callback channel** (actor -> client): `Completion` notifications

### Key Interfaces

| Interface | Role | Description |
|-----------|------|-------------|
| `IBlockDevice` | Provided | Client connection, device info, IO operations, telemetry |
| `ILogger` | Receptacle | Debug logging via dependency injection |
| `ISPDKEnv` | Receptacle | SPDK environment initialization |

### Messaging API

- **Sync IO**: `ReadSync`, `WriteSync` — actor busy-polls until NVMe completion
- **Async IO**: `ReadAsync`, `WriteAsync` — SPDK async submission with timeout/abort
- **Admin**: `NsProbe`, `NsCreate`, `NsFormat`, `NsDelete`, `ControllerReset`
- **Batch**: `BatchSubmit` — multiple operations in a single message
- **Other**: `WriteZeros`, `AbortOp`

### DMA Buffers

IO operations use `DmaBuffer` (SPDK DMA-safe memory). Reads take `Arc<Mutex<DmaBuffer>>`, writes take `Arc<DmaBuffer>`.

### Telemetry

Enable with `--features telemetry` to collect IO latency (min/max/mean), operation count, and throughput statistics.

## Prerequisites

- Linux host with hugepages configured and IOMMU enabled
- NVMe device bound to VFIO/UIO (`deps/spdk/scripts/setup.sh`)
- SPDK built at `deps/spdk-build/` (run `deps/build_spdk.sh`)
- Rust stable toolchain (edition 2021, MSRV 1.75+)

## Build

```bash
cargo build -p block-device-spdk-nvme

# With telemetry support
cargo build -p block-device-spdk-nvme --features telemetry
```

## Tests

```bash
# All tests (unit + integration)
cargo test -p block-device-spdk-nvme

# Integration tests only
cargo test -p block-device-spdk-nvme --test integration
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
cargo bench -p block-device-spdk-nvme

# Run a specific benchmark
cargo bench -p block-device-spdk-nvme --bench latency
cargo bench -p block-device-spdk-nvme --bench throughput
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
  lib.rs          Component definition, connect_client(), flush_io()
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
