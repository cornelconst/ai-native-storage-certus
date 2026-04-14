# Feature Specification: IBasicBlockDevice Interface

**Feature Branch**: `001-basic-block-device`
**Created**: 2026-04-14
**Status**: Complete
**Input**: Backfill specification for the existing IBasicBlockDevice component interface

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Open, Read/Write, Close an NVMe Device (Priority: P1)

A system integrator wires the `SimpleBlockDevice` component to an SPDK
environment and logger, opens the device, performs zero-copy block I/O
using caller-allocated DMA buffers, and closes the device cleanly.

**Why this priority**: This is the primary use case — the entire reason
the component exists.

**Independent Test**: Wire components, init SPDK env, open device, write
a test pattern, read it back, verify match, close. Requires real NVMe
hardware.

**Acceptance Scenarios**:

1. **Given** an SPDK environment is initialized with NVMe devices bound
   to vfio-pci, **When** the integrator calls `open()` on a wired
   `SimpleBlockDevice`, **Then** the device opens successfully and
   `is_open()` returns true.
2. **Given** an open block device and a DMA buffer filled with a test
   pattern, **When** the integrator calls `write_blocks(lba, &buf)`,
   **Then** the write completes without error.
3. **Given** data written at a specific LBA, **When** the integrator
   calls `read_blocks(lba, &mut buf)` into a fresh DMA buffer, **Then**
   the buffer contents match the original pattern byte-for-byte.
4. **Given** an open block device, **When** the integrator calls
   `close()`, **Then** the device closes, `is_open()` returns false,
   and `sector_size()` / `num_sectors()` return 0.

---

### User Story 2 - Query Device Geometry (Priority: P1)

A caller queries the block device's sector size and total sector count
after opening, to correctly size DMA buffers and compute LBA ranges.

**Why this priority**: Every I/O operation depends on knowing the sector
size to allocate correctly-sized buffers.

**Independent Test**: Open device, call `sector_size()` and
`num_sectors()`, verify both are non-zero and plausible.

**Acceptance Scenarios**:

1. **Given** an open block device, **When** the caller queries
   `sector_size()`, **Then** a non-zero value (typically 512 or 4096) is
   returned.
2. **Given** an open block device, **When** the caller queries
   `num_sectors()`, **Then** the total sector count is returned.
3. **Given** a closed block device, **When** the caller queries
   `sector_size()` or `num_sectors()`, **Then** 0 is returned.

---

### User Story 3 - Wire Components via Receptacles (Priority: P2)

A system integrator connects the `SimpleBlockDevice` component's
receptacles (`spdk_env` and `logger`) to provider components, enabling
the block device to function within the component framework.

**Why this priority**: Receptacle wiring is the component framework's
composition mechanism and must work correctly for the block device to
be usable in any Certus assembly.

**Independent Test**: Instantiate component, query `IBasicBlockDevice`
via `IUnknown`, connect logger and spdk_env receptacles, verify
connection state.

**Acceptance Scenarios**:

1. **Given** a freshly created `SimpleBlockDevice`, **When** the
   integrator queries for `IBasicBlockDevice`, **Then** a valid
   interface reference is returned.
2. **Given** an unwired component, **When** the integrator connects the
   `logger` receptacle, **Then** `logger.is_connected()` returns true.
3. **Given** an unwired component, **When** `open()` is called without
   connecting the logger, **Then** `LoggerNotConnected` error is
   returned.
4. **Given** a component with logger connected but spdk_env not,
   **When** `open()` is called, **Then** `EnvNotInitialized` error is
   returned.

---

### User Story 4 - Graceful Cleanup on Drop (Priority: P3)

If a caller forgets to call `close()` before dropping the component,
the `Drop` implementation cleans up hardware resources automatically
and logs a warning.

**Why this priority**: Prevents resource leaks, but callers should
prefer explicit `close()`.

**Independent Test**: Create and drop a component that was never opened
— verify no panic. (Full drop-while-open test requires hardware.)

**Acceptance Scenarios**:

1. **Given** an open block device, **When** the component is dropped
   without calling `close()`, **Then** the qpair is freed and the
   controller is detached automatically, with a warning logged to stderr.
2. **Given** a closed (or never-opened) block device, **When** the
   component is dropped, **Then** no cleanup action is taken and no
   panic occurs.

---

### Edge Cases

- What happens when `open()` is called on an already-open device?
  MUST return `AlreadyOpen` error without disturbing the existing session.
- What happens when `read_blocks` / `write_blocks` is called on a
  closed device? MUST return `NotOpen` error.
- What happens when the buffer size is not a multiple of the sector
  size? MUST return `BufferSizeMismatch` error.
- What happens when the buffer is empty (zero length)? MUST return
  `BufferSizeMismatch` error.
- What happens when no NVMe controllers are found during probe? MUST
  return `ProbeFailure` with guidance about vfio-pci binding.
- What happens when namespace 1 is not active? MUST return
  `NamespaceNotFound` error.
- What happens when qpair allocation fails? MUST return
  `QpairAllocationFailed` and detach the controller to avoid leaks.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The component MUST probe the local PCIe bus for NVMe
  controllers and attach the first controller found during `open()`.
- **FR-002**: The component MUST open namespace 1 on the attached
  controller and allocate an I/O queue pair.
- **FR-003**: `read_blocks(lba, buf)` MUST perform zero-copy read by
  passing the caller's `DmaBuffer` pointer directly to
  `spdk_nvme_ns_cmd_read`.
- **FR-004**: `write_blocks(lba, buf)` MUST perform zero-copy write by
  passing the caller's `DmaBuffer` pointer directly to
  `spdk_nvme_ns_cmd_write`.
- **FR-005**: Both read and write MUST use synchronous completion:
  submit the NVMe command, then busy-poll
  `spdk_nvme_qpair_process_completions` until the completion callback
  fires.
- **FR-006**: `close()` MUST free the I/O queue pair and detach the
  NVMe controller, in that order.
- **FR-007**: `sector_size()` MUST return the device sector size in
  bytes when open, or 0 when closed.
- **FR-008**: `num_sectors()` MUST return the total sector count when
  open, or 0 when closed.
- **FR-009**: `is_open()` MUST return `true` when the device is open,
  `false` otherwise.
- **FR-010**: `open()` MUST fail with `LoggerNotConnected` if the
  logger receptacle is not connected.
- **FR-011**: `open()` MUST fail with `EnvNotInitialized` if the
  spdk_env receptacle is not connected or the environment is not
  initialized.
- **FR-012**: `open()` MUST fail with `AlreadyOpen` if the device is
  already open.
- **FR-013**: `read_blocks` and `write_blocks` MUST fail with `NotOpen`
  if the device is not open.
- **FR-014**: `read_blocks` and `write_blocks` MUST fail with
  `BufferSizeMismatch` if the buffer length is zero or not a positive
  multiple of the sector size.
- **FR-015**: `close()` MUST fail with `NotOpen` if the device is not
  open.
- **FR-016**: The `Drop` implementation MUST clean up hardware resources
  (free qpair, detach controller) if the device is still open, logging
  a warning to stderr.
- **FR-017**: The component MUST provide `IBasicBlockDevice` via the
  component framework's `query` mechanism.
- **FR-018**: Access to the qpair MUST be serialized via `Mutex` to
  satisfy SPDK's single-thread-per-qpair requirement.
- **FR-019**: NVMe read/write completion status MUST be checked. A
  non-zero completion status MUST be reported as `ReadFailed` or
  `WriteFailed` with the status code.

### Key Entities

- **IBasicBlockDevice**: The interface trait (7 methods) defining the
  block device contract. Defined via `define_interface!` macro.
- **SimpleBlockDevice**: The component implementing `IBasicBlockDevice`.
  Has receptacles for `ISPDKEnv` and `ILogger`. Uses
  `Mutex<Option<InnerState>>` for thread-safe access.
- **InnerState**: Opaque state holding raw SPDK pointers (`ctrlr`, `ns`,
  `qpair`) and device geometry (`sector_size`, `num_sectors`). Valid
  between `open_device` and `close_device`.
- **BlockDeviceError**: Error enum with 11 variants covering all failure
  modes. Implements `Display`, `Debug`, `Clone`, `std::error::Error`.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: All 7 `IBasicBlockDevice` methods are implemented and
  callable through the component framework's `query` mechanism.
- **SC-002**: Unit tests pass for all pre-flight error conditions
  (logger not connected, env not connected, close when not open, drop
  when not open) without requiring NVMe hardware.
- **SC-003**: The `basic_io` example demonstrates a successful
  write-read-verify cycle on real NVMe hardware.
- **SC-004**: All 11 `BlockDeviceError` variants have `Display` tests
  verifying human-readable output.
- **SC-005**: `cargo clippy -- -D warnings` produces no warnings.

## Clarifications

### Dual Interface Definition (2026-04-14)

`IBasicBlockDevice` is currently defined in **two** places:

1. `components/interfaces/src/iblock_device.rs` — the intended
   canonical definition, gated behind `cfg(feature = "spdk")`.
2. `src/lib.rs` — a local `define_interface!` call in this crate.

These are **separate Rust traits** despite identical signatures. The
`spdk-simple-block-device` crate does not depend on the `interfaces`
crate and uses its own local definition. This means code depending on
`interfaces::IBasicBlockDevice` cannot use this component's
implementation through that trait.

The same duplication applies to `BlockDeviceError` (defined in both
`interfaces/src/spdk_types.rs` and `src/error.rs`) and `DmaBuffer`
(defined in `interfaces/src/spdk_types.rs` and `spdk_env`).

**Resolution**: Unifying these definitions is a separate code task.
Until then, changes to the interface MUST be applied to both locations.

## Assumptions

- The component targets a single NVMe controller (the first found
  during probe) and a single namespace (namespace 1).
- Multi-device support is out of scope for this spec.
- Callers are responsible for allocating and managing `DmaBuffer`
  memory via the `spdk_env` crate.
- The SPDK environment must be initialized exactly once per process
  before any block device operations.
