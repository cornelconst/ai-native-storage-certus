# Feature Specification: Actor-Based Block Device

**Feature Branch**: `002-actor-block-device`
**Created**: 2026-04-14
**Status**: Complete
**Input**: Backfill specification for the existing actor-based block device API

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Actor-Based Write-Read-Verify (Priority: P1)

A caller creates a block device actor, opens the device, performs
zero-copy write and read operations by transferring DMA buffer ownership
through the actor channel, and verifies data integrity.

**Why this priority**: This is the primary use case for the actor path —
performing I/O without the caller needing to manage thread safety.

**Independent Test**: Create actor, open device, write test pattern,
read back, verify match, close, shutdown. Requires real NVMe hardware.

**Acceptance Scenarios**:

1. **Given** an initialized SPDK environment, **When** the caller
   creates a `BlockDeviceHandler`, wraps it in an `Actor`, activates it,
   and creates a `BlockDeviceClient`, **Then** the client is ready to
   send requests.
2. **Given** an active client, **When** the caller sends `open()`,
   **Then** a `DeviceInfo` is returned with non-zero `sector_size` and
   `num_sectors`.
3. **Given** an open device, **When** the caller sends
   `write_blocks(lba, buf)` with a filled DMA buffer, **Then** the
   buffer is returned to the caller after the write completes (zero-copy
   round-trip).
4. **Given** data written at a specific LBA, **When** the caller sends
   `read_blocks(lba, buf)` with a fresh DMA buffer, **Then** the buffer
   is returned containing the written data.

---

### User Story 2 - DMA Buffer Allocation via Client (Priority: P2)

A caller uses the client's convenience method to allocate a correctly-sized
DMA buffer based on the device's sector size, without needing to query
device geometry manually.

**Why this priority**: Simplifies buffer management — the most
error-prone part of zero-copy I/O.

**Independent Test**: Open device, call `alloc_dma_buffer(1)`, verify
buffer length equals sector size.

**Acceptance Scenarios**:

1. **Given** an open device with sector size 512, **When** the caller
   calls `alloc_dma_buffer(4)`, **Then** a DMA buffer of 2048 bytes is
   returned.
2. **Given** a device that has not been opened, **When** the caller
   calls `alloc_dma_buffer(1)`, **Then** a `NotOpen` error is returned.

---

### User Story 3 - Multi-Sector I/O with Buffer Reuse (Priority: P2)

A caller writes and reads multiple sectors in a single operation, and
reuses the returned DMA buffer for subsequent operations without
reallocating.

**Why this priority**: Demonstrates that the actor path supports
multi-sector I/O and that buffer reuse works correctly.

**Independent Test**: Allocate multi-sector buffer, write, read back
into same buffer, verify.

**Acceptance Scenarios**:

1. **Given** an open device, **When** the caller writes 4 sectors
   starting at a given LBA, **Then** the write completes and the buffer
   is returned.
2. **Given** the returned buffer, **When** the caller zeroes it and
   sends it back for a read at the same LBA, **Then** the buffer
   contains the originally-written data.
3. **Given** a buffer returned from a write, **When** the caller sends
   it for a read at a different LBA, **Then** the buffer is correctly
   filled with the new data.

---

### User Story 4 - Graceful Shutdown (Priority: P2)

A caller closes the device and shuts down the actor, ensuring all
resources are released. If the caller shuts down without closing, the
actor cleans up automatically.

**Why this priority**: Resource cleanup is essential for correctness in
systems that open/close devices repeatedly.

**Independent Test**: Open, close, shutdown — verify no error. Open,
shutdown without close — verify no panic or leak.

**Acceptance Scenarios**:

1. **Given** an open device, **When** the caller sends `close()` then
   `shutdown()`, **Then** both complete without error and the actor
   thread joins.
2. **Given** an open device, **When** the caller calls `shutdown()`
   without sending `close()`, **Then** the actor's `on_stop` handler
   closes the device automatically.
3. **Given** a shut-down client, **When** the caller attempts to send
   any request, **Then** an appropriate error is returned.

---

### Edge Cases

- What happens when `open()` is sent to an already-open actor? MUST
  return `AlreadyOpen` error.
- What happens when `read_blocks` / `write_blocks` is sent before
  `open()`? MUST return `NotOpen` error.
- What happens when `close()` is sent to a not-open actor? MUST return
  `NotOpen` error.
- What happens when the actor thread panics? The `send` on the client
  side returns an error (channel disconnected).

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: `BlockDeviceHandler` MUST implement `ActorHandler<BlockIoRequest>`
  and process all message variants on the actor's dedicated thread.
- **FR-002**: All NVMe operations (open, read, write, close) MUST
  execute exclusively on the actor thread, satisfying the
  single-thread-per-qpair invariant without requiring the caller to
  manage thread affinity.
- **FR-003**: `BlockIoRequest::Read` and `BlockIoRequest::Write` MUST
  transfer `DmaBuffer` ownership to the actor thread through the
  channel and return it to the caller through the reply channel — no
  buffer contents are copied.
- **FR-004**: Each `BlockIoRequest` variant MUST carry a one-shot
  `mpsc::SyncSender` reply channel so the caller can block
  synchronously for the result.
- **FR-005**: `BlockDeviceClient` MUST provide blocking methods
  (`open`, `read_blocks`, `write_blocks`, `close`, `info`, `shutdown`)
  that send a request and wait for the reply.
- **FR-006**: `BlockDeviceClient::open()` MUST return a `DeviceInfo`
  struct containing `sector_size` and `num_sectors` on success.
- **FR-007**: `BlockDeviceClient::alloc_dma_buffer(num_sectors)` MUST
  allocate a DMA buffer sized to `num_sectors * sector_size` bytes,
  using the sector size from the most recent `info()` query.
- **FR-008**: `BlockDeviceClient::alloc_dma_buffer()` MUST return
  `NotOpen` if the device has not been opened.
- **FR-009**: `BlockDeviceHandler::on_stop()` MUST close the device
  (free qpair, detach controller) if it is still open when the actor
  is stopped.
- **FR-010**: `BlockDeviceClient::shutdown()` MUST deactivate the
  actor and join its thread.
- **FR-011**: If the actor thread is not running (panicked or shut
  down), client methods MUST return errors rather than blocking
  indefinitely.

### Key Entities

- **BlockIoRequest**: Message enum with 5 variants (`Open`, `Read`,
  `Write`, `Close`, `GetInfo`). Each carries a reply channel.
- **BlockDeviceHandler**: Actor handler owning an
  `Arc<dyn ISPDKEnv + Send + Sync>` and `Option<InnerState>`. Processes
  messages on a dedicated thread.
- **BlockDeviceClient**: Synchronous client wrapping an
  `ActorHandle<BlockIoRequest>`. Usable from any thread.
- **DeviceInfo**: Value type with `sector_size: u32` and
  `num_sectors: u64`. Returned by `open()` and `info()`.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: The `actor_io` example demonstrates a successful
  write-read-verify cycle, multi-sector I/O, buffer reuse, device info
  query, and clean shutdown on real NVMe hardware.
- **SC-002**: `DeviceInfo` unit tests verify `Debug`, `Clone`, `Copy`,
  `PartialEq`, and `Eq` derive behavior.
- **SC-003**: DMA buffer ownership transfers through the channel
  without any intermediate copies (verified by the fact that
  `read_blocks` and `write_blocks` return the same `DmaBuffer`).
- **SC-004**: `cargo clippy -- -D warnings` produces no warnings.

## Assumptions

- The actor uses `component_framework::actor::Actor` with the `simple`
  configuration (unbounded channel, single handler thread).
- The `ISPDKEnv` passed to `BlockDeviceHandler::new()` must already be
  initialized — the handler does not call `init()`.
- `mpsc::sync_channel(0)` (rendezvous channel) is used for replies,
  meaning the actor thread blocks until the caller receives the reply.
  This is acceptable because NVMe I/O is fast and the caller is
  expected to consume the reply promptly.
- Callers who need the component-framework's receptacle wiring model
  should use `SimpleBlockDevice` (spec-001) instead.
