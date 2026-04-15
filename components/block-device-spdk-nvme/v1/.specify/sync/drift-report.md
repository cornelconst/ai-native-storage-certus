# Spec Drift Report

Generated: 2026-04-14
Project: block-device-spdk-nvme v1
Spec: 001-spdk-nvme-block-device

## Summary

| Category | Count |
|----------|-------|
| Specs Analyzed | 1 |
| Requirements Checked | 20 FR + 8 SC = 28 |
| Aligned | 18 (64%) |
| Drifted | 10 (36%) |
| Not Implemented | 0 |
| Unspecced Code | 2 |

## Detailed Findings

### Spec: 001-spdk-nvme-block-device - SPDK NVMe Block Device Component

#### Aligned

- **FR-001**: IBlockDevice interface with `connect_client()` `src/lib.rs:337`
- **FR-002**: Two SPSC channels per client (ingress + callback) `src/lib.rs:349-357`
- **FR-007**: Batch submission via `Command::BatchSubmit` dispatched in actor `src/actor.rs:266-281`
- **FR-010**: Device info queries (capacity, max queue depth, IO queue count, max transfer size, block size, NUMA id, NVMe version) all implemented `src/lib.rs:380-424`
- **FR-012**: Single NVMe controller per instance, attached at `initialize()` `src/lib.rs:147-228`
- **FR-013**: Actor thread NUMA-pinned to controller's node `src/lib.rs:201-216`
- **FR-014**: Actor polls all client ingress channels on every `handle()` call `src/actor.rs:127-141`
- **FR-015**: QueuePairPool with depths [4, 16, 64, 256] and batch-size-based selection heuristic `src/qpair.rs:136-283`
- **FR-016**: `logger: ILogger` receptacle defined; `LoggerComponent` used in integration tests `src/lib.rs:73`, `tests/integration.rs:29`
- **FR-017**: `spdk_env: ISPDKEnv` receptacle checked in `initialize()` `src/lib.rs:148-153`
- **FR-018**: Commands use `Arc<Mutex<DmaBuffer>>` (read) and `Arc<DmaBuffer>` (write) for zero-copy `interfaces/src/iblock_device.rs:186-225`
- **FR-020**: All namespace operations dispatch through actor's `handle()` method `src/actor.rs:288-340`
- **SC-001**: Sync read/write round-trip implemented; latency benchmarks in `benches/latency.rs`
- **SC-003**: Batch dispatch with throughput benchmarks in `benches/throughput.rs`
- **SC-005**: Device info queries return hardware properties via `ControllerSnapshot`
- **SC-007**: NUMA pinning implemented in `initialize()` `src/lib.rs:203-216`
- **SC-008**: Unit tests in every module, doc tests on public types, two Criterion benchmark suites

#### Drifted

- **FR-003**: Spec says sync read/write take "timeout" parameter, but `Command::ReadSync` and `Command::WriteSync` have no timeout field.
  - Location: `interfaces/src/iblock_device.rs:186-203`
  - Severity: minor (sync ops block to completion; timeout is arguably unnecessary)

- **FR-004**: Async read/write exist in the `Command` enum with `timeout_ms`, but the actor executes them synchronously. Pending ops are inserted and immediately removed, so timeout tracking never fires.
  - Location: `src/actor.rs:194-250` (comment: "Execute synchronously for now")
  - Severity: major

- **FR-005**: `AbortOp` handler removes from pending and sends `AbortAck`, but since async ops complete synchronously and pending ops are immediately removed, there is never anything to abort.
  - Location: `src/actor.rs:282-287`
  - Severity: major (depends on FR-004 being fixed)

- **FR-006**: `do_write_zeros()` returns `NvmeBlockError::NotSupported` instead of performing the operation.
  - Location: `src/actor.rs:492-504`
  - Severity: moderate (spec says MUST support; could be implemented via zero-filled write buffer)

- **FR-008**: Only `NsProbe` works. `create()`, `format()`, and `delete()` all return `NvmeBlockError::NotSupported` due to missing SPDK bindings.
  - Location: `src/namespace.rs:71-110`
  - Severity: moderate (blocked on extended `spdk-sys` bindings)

- **FR-009**: `ControllerReset` cancels pending ops but returns `NotSupported` for the actual reset.
  - Location: `src/actor.rs:341-357`
  - Severity: moderate (blocked on extended `spdk-sys` bindings)

- **FR-011 / SC-006**: Telemetry infrastructure exists (`TelemetryStats` with atomic CAS min/max, `record()`, `snapshot()`), but `record()` is never called from the IO dispatch path in `actor.rs`. Statistics are always zero even with the `telemetry` feature enabled.
  - Location: `src/actor.rs:163-170` (`_telemetry` parameter unused), `src/telemetry.rs:42-72`
  - Severity: major

- **FR-019**: Client disconnect requires an explicit `ControlMessage::DisconnectClient` message. Automatic detection of channel drop (when client drops `command_tx`) is not implemented. Also, the disconnect handler sends error completions to the callback channel rather than silently discarding them as the spec requires.
  - Location: `src/actor.rs:528-541`
  - Severity: moderate

- **SC-002**: Async timeout reporting does not work because async operations complete synchronously (see FR-004).
  - Severity: major (depends on FR-004)

- **SC-004**: Only namespace probe works; create/format/delete return `NotSupported` (see FR-008).
  - Severity: moderate

### Unspecced Code

| Feature | Location | Description | Suggested Spec |
|---------|----------|-------------|----------------|
| `flush_io()` | `src/lib.rs:322-333` | Sends `ControlMessage::Poll` to wake the actor for IO processing. Required because the actor only polls client channels when handling a control message. | Should be documented as part of the client API contract in FR-001/FR-002. |
| `ControllerSnapshot` | `src/lib.rs:110-128` | Caches controller properties at init time so device info queries avoid actor round-trips. | Internal optimization; no spec change needed, but could be mentioned in architecture notes. |

## Inter-Spec Conflicts

None identified. Only one spec exists for this component.

## Recommendations

1. **Wire telemetry recording into the IO path** (FR-011, SC-006): Call `telemetry.record(latency_ns, bytes)` in `do_sync_read()` and `do_sync_write()` when the `telemetry` feature is enabled. This is the lowest-effort highest-value fix.

2. **Implement true async IO** (FR-004, FR-005, SC-002): Replace the synchronous execution in `ReadAsync`/`WriteAsync` handlers with actual SPDK async submission using completion callbacks. This unblocks timeout tracking and abort functionality.

3. **Implement write-zeros** (FR-006): Use a zero-filled DMA buffer with a regular `spdk_nvme_ns_cmd_write` call as a workaround until `spdk_nvme_ns_cmd_write_zeroes` is available in the bindings.

4. **Auto-detect client disconnect** (FR-019): Check for closed ingress channels during `poll_clients()` (e.g., `try_recv()` returns a disconnected error). On detection, cancel pending ops and remove the client without sending error completions.

5. **Extend `spdk-sys` bindings** (FR-008, FR-009): Add bindings for `spdk_nvme_ctrlr_create_ns`, `spdk_nvme_ctrlr_delete_ns`, `spdk_nvme_ctrlr_format`, and `spdk_nvme_ctrlr_reset` to unblock namespace management and controller reset.

6. **Document `flush_io()` contract**: Since clients must call `flush_io()` after sending commands to ensure prompt processing, this should be documented in the IBlockDevice interface contract or the client API usage examples.
