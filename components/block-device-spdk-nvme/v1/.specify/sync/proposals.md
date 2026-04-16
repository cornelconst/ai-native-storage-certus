# Drift Resolution Proposals

Generated: 2026-04-16
Based on: drift-report from 2026-04-16

## Summary

| Resolution Type | Count |
|-----------------|-------|
| Backfill (Code -> Spec) | 4 |
| Align (Spec -> Code) | 2 |
| Human Decision | 0 |
| New Spec Items | 2 |
| Implement | 3 |

---

## Proposal 1: 001/FR-003 — Sync timeout parameter

**Direction**: BACKFILL

**Current State**:
- Spec says: "System MUST support synchronous read and write operations with parameters for NVMe namespace id, DmaBuffer, LBA offset, and timeout."
- Code does: `Command::ReadSync`/`WriteSync` have ns_id, lba, and buf — no timeout field. Timeout only exists on async variants.

**Proposed Resolution**:

Update FR-003 to:

> **FR-003**: System MUST support synchronous read and write operations with parameters for NVMe namespace id, DmaBuffer, and LBA offset. Synchronous operations block until the underlying NVMe command completes; timeout is not applicable (use async operations with FR-004 for timeout-controlled IO).

**Rationale**: Synchronous SPDK commands poll the completion queue in a tight loop — they either complete when the NVMe device responds or hang if the device is unresponsive (which requires a controller reset per FR-009). Adding a software timeout to a blocking poll loop would add complexity without real benefit: the timeout granularity would be poor (polling is sub-microsecond) and the only recovery for a non-responding device is hardware reset anyway. The async path (FR-004) already provides proper timeout support. This is intentional design, not an oversight.

**Confidence**: HIGH

---

## Proposal 2: 001/FR-009 — Controller reset scope

**Direction**: ALIGN

**Current State**:
- Spec says: "System MUST support controller hardware reset with graceful handling of in-flight operations."
- Code does: Reset handler cancels pending ops for only the requesting client, not all connected clients.

**Proposed Resolution**:

Fix `src/actor.rs` `ControllerReset` handler to iterate ALL clients' pending ops, not just the requesting client's. The handler currently has access to `self.clients` but only uses `session` and `pending_ops` from `dispatch_command`'s parameters (the requesting client).

The fix should:
1. Move `ControllerReset` handling out of `dispatch_command` into a separate method on `BlockDeviceHandler` that has access to `self.clients`
2. Cancel all pending ops across all clients with `NvmeBlockError::Aborted("cancelled due to controller reset")`
3. Then perform the actual `spdk_nvme_ctrlr_reset` call
4. Send `Completion::ResetDone` only to the requesting client

**Rationale**: A hardware controller reset invalidates ALL in-flight NVMe commands across all queue pairs. Any pending async operations from other clients will get undefined behavior or SPDK errors when completions arrive for commands that were wiped by the reset. The spec correctly requires graceful handling of ALL in-flight operations, and User Story 6 Acceptance Scenario 2 explicitly says "pending operations are cancelled with errors." The current code is a bug.

**Confidence**: HIGH

---

## Proposal 3: 001/SC-008 — Benchmark coverage

**Direction**: BACKFILL

**Current State**:
- Spec says: "All public APIs have unit tests, documentation tests, and Criterion benchmarks for performance-sensitive paths."
- Code does: Qpair selection benchmarks removed when `qpair` module was made `pub(crate)`. Remaining benchmarks cover batch construction, sync IO latency, and batch write throughput.

**Proposed Resolution**:

Update SC-008 to:

> **SC-008**: All public interface methods have unit tests and documentation tests. Performance-sensitive paths (IO submission, batch processing, qpair selection) MUST have benchmarks — either Criterion benchmarks for public interface paths or unit-level benchmarks for internal algorithms.

**Rationale**: The qpair selection algorithm is now an internal implementation detail, not a public API. External Criterion benchmarks are inappropriate for `pub(crate)` modules. The unit tests in `qpair.rs` already exercise the selection heuristic thoroughly. The remaining Criterion benchmarks cover the actual public interface paths (batch writes, sync IO) which are the performance-sensitive surfaces that matter to consumers. The spec should reflect that internal algorithms are benchmarked through unit tests, not external Criterion harnesses.

**Confidence**: HIGH

---

## Proposal 4: 001/SC-001 — Latency envelope validation

**Direction**: IMPLEMENT

**Current State**:
- Spec says: "A client can complete a synchronous read/write round-trip within the latency envelope expected for direct NVMe access (single-digit microsecond range for 4KB blocks)."
- Code: No test asserts this. The latency bench measures but doesn't assert a threshold.

**Proposed Resolution**:

Add a hardware-dependent integration test that:
1. Performs 100 sync write+read round-trips of 4KB blocks
2. Computes p50 latency
3. Asserts p50 < 100us (generous threshold for direct NVMe; spec says "single-digit microsecond" but channel overhead adds latency)

The test should self-skip when no SPDK hardware is available (same pattern as existing hardware tests).

**Rationale**: SC-001 is a measurable outcome that can be validated. The exact threshold needs tuning for the channel-mediated architecture (actor polling adds overhead vs raw SPDK), but a hardware test provides regression detection.

**Confidence**: MEDIUM (threshold value needs tuning to hardware)

---

## Proposal 5: 001/SC-002 — Timeout accuracy validation

**Direction**: IMPLEMENT

**Current State**:
- Spec says: "Async operations that exceed their specified timeout are reported as errors within a bounded margin (no more than 10% beyond the timeout value)."
- Code: `check_timeouts()` runs with ~1ms granularity. No test validates accuracy.

**Proposed Resolution**:

Add a hardware-dependent integration test that:
1. Submits an async read with a very short timeout (e.g., 1ms) to a valid LBA
2. Measures time between submission and receiving `Completion::Timeout`
3. Asserts the timeout completion arrives within 10% + 2ms of the specified timeout (2ms margin for check_timeouts granularity)

Note: This test is inherently timing-sensitive and may be flaky on loaded systems. Consider marking it `#[ignore]` with a comment to run manually.

**Rationale**: The 1ms check_timeouts throttle means the worst-case added latency is 1ms. For timeouts > 10ms, the 10% margin absorbs this. For very short timeouts the margin is tighter, so the test should use a reasonable timeout value (e.g., 50ms).

**Confidence**: MEDIUM (timing-sensitive tests are inherently fragile)

---

## Proposal 6: 001/SC-006 — Telemetry accuracy validation

**Direction**: IMPLEMENT

**Current State**:
- Spec says: "When telemetry is enabled, latency and throughput statistics are accurate to within 5% of independently measured values."
- Code: No test. Requires `--features telemetry` build.

**Proposed Resolution**:

Add a `#[cfg(feature = "telemetry")]` integration test that:
1. Performs N sync writes, timing each independently with `Instant::now()`
2. Queries `IBlockDevice::telemetry()` for the TelemetrySnapshot
3. Asserts `total_ops == N`
4. Asserts `mean_latency_ns` is within 5% of the independently computed mean
5. Asserts `min_latency_ns <= independent_min` and `max_latency_ns >= independent_max` (telemetry may include overhead not captured by external timing)

**Rationale**: The atomic CAS-based min/max tracking in `TelemetryStats` should be accurate. The main risk is timing discrepancies between the actor's `Instant::now()` and the test's `Instant::now()` (different threads). A 5% tolerance should absorb this.

**Confidence**: MEDIUM (requires `--features telemetry` test build)

---

## Proposal 7: 002/NEW — Add `--io-mode` to spec

**Direction**: BACKFILL (unspecced feature -> spec)

**Current State**:
- Code has: `--io-mode sync|async` flag (`config.rs:28-43`) controlling whether the benchmark uses synchronous or asynchronous SPDK commands. Default: async.
- Spec: No mention of io-mode.

**Proposed Resolution**:

Add to spec 002 requirements:

> **FR-022**: The application MUST accept a command-line flag `--io-mode` with values `sync` and `async` to select the IO submission mode. Default: `async`. In `sync` mode, each IO operation blocks until completion before the next is submitted (effective queue depth 1 per thread regardless of `--queue-depth`). In `async` mode, operations are submitted asynchronously and the pipeline is kept full to `--queue-depth`.

Add to FR-012 config summary: include IO mode in the printed header.

**Rationale**: The sync/async toggle is a working, tested feature that enables comparing synchronous vs asynchronous IO performance on the same device. It's used by developers to isolate per-op latency (sync) from throughput (async). The feature follows the same pattern as other config flags and is already integrated into the worker, config, and report modules.

**Confidence**: HIGH

---

## Proposal 8: 001/NEW — Add IBlockDeviceAdmin to spec

**Direction**: BACKFILL (unspecced feature -> spec)

**Current State**:
- Code has: `IBlockDeviceAdmin` interface with `set_pci_address()` and `initialize()` methods, declared in `provides` list, used by benchmark and integration tests.
- Spec: FR-012 says "attached and initialized at instantiation" but doesn't define the admin interface.

**Proposed Resolution**:

Add to spec 001 requirements:

> **FR-021**: The component MUST provide an `IBlockDeviceAdmin` interface (defined via `define_interface!`) with two methods: `set_pci_address(addr: PciAddress)` to configure the target NVMe controller, and `initialize() -> Result<(), NvmeBlockError>` to attach to the controller and start the actor thread. `set_pci_address` MUST be called before `initialize`. The admin interface MUST be queryable via the component framework's `query<IBlockDeviceAdmin>()`.

Update FR-012 to reference FR-021:

> **FR-012**: Each component instance MUST be associated with a single NVMe controller device, configured via `IBlockDeviceAdmin::set_pci_address` and attached via `IBlockDeviceAdmin::initialize` (see FR-021).

**Rationale**: The admin lifecycle is a fundamental part of using the component. Every consumer (benchmark, integration tests, future apps) must call `set_pci_address` + `initialize` before using `IBlockDevice`. Not specifying this interface means consumers have no contract for how to set up the component. The interface is already defined via `define_interface!` in the interfaces crate and declared in the component's `provides` list.

**Confidence**: HIGH

---

## Proposal 9: 001/CONFLICT — Fix WriteAsync buffer lifetime bug

**Direction**: ALIGN (critical bug)

**Current State**:
- Spec (FR-004): Requires async write to work correctly
- Code: `WriteAsync` in `dispatch_command` does not retain the `Arc<DmaBuffer>` after submitting to SPDK. The buffer may be freed while NVMe DMA is still reading from it. Documented in `tests/integration.rs:634-638`.

**Proposed Resolution**:

Fix `src/actor.rs` WriteAsync handler to pin the buffer in `PendingOp` until the completion callback fires:

1. Add a `buffer: Option<Arc<DmaBuffer>>` field to `PendingOp`
2. In the `WriteAsync` arm, clone the `Arc<DmaBuffer>` into the `PendingOp` entry
3. The buffer is released when the pending op is removed (on completion, timeout, or abort)

This ensures the DMA memory remains valid for the entire duration of the NVMe command.

**Rationale**: This is a use-after-free bug that can cause data corruption or crashes. The spec correctly requires async writes to work. The fix is straightforward and follows the same pattern used for async reads (where the caller retains an `Arc` clone).

**Confidence**: HIGH
