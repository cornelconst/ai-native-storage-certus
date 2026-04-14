# Feature Specification: Multi-Qpair Support

**Feature Branch**: `003-multi-qpair`
**Created**: 2026-04-14
**Status**: Complete
**Input**: Backfill specification for the multi-qpair allocation API

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Allocate Worker Qpairs for Parallel I/O (Priority: P1)

A performance-oriented caller opens a device with `open_device` to get
the primary `InnerState`, then allocates additional I/O queue pairs via
`alloc_qpair` — one per worker thread — to drive parallel NVMe I/O at
full queue depth.

**Why this priority**: Multi-qpair is the mechanism for scaling NVMe
throughput across CPU cores. Without it, I/O is serialized on a single
queue.

**Independent Test**: Open device, allocate N additional qpairs, verify
each is non-null, free all qpairs, close device. Requires hardware.

**Acceptance Scenarios**:

1. **Given** an open device with valid `ctrlr` and `ns` pointers,
   **When** the caller calls `alloc_qpair(ctrlr, ns, sector_size,
   num_sectors)`, **Then** a new `InnerState` is returned with its own
   qpair but shared `ctrlr`/`ns` pointers.
2. **Given** a successfully allocated worker qpair, **When** the caller
   performs read/write operations using the worker's `InnerState`,
   **Then** I/O completes correctly (the worker qpair is fully
   functional).
3. **Given** multiple worker qpairs, **When** each is used on a
   separate thread, **Then** I/O operations proceed in parallel without
   interference.

---

### User Story 2 - Free Worker Qpairs Without Detaching Controller (Priority: P1)

A caller frees worker qpairs individually via `free_qpair` without
detaching the shared controller, allowing the primary device to remain
open while workers are torn down.

**Why this priority**: Worker lifecycle must be independent of the
primary device lifecycle — workers may start and stop during a
benchmark run.

**Independent Test**: Allocate worker qpair, free it, verify the
primary device is still usable for I/O.

**Acceptance Scenarios**:

1. **Given** a worker qpair, **When** the caller calls
   `free_qpair(state)`, **Then** only the qpair is freed; the
   controller is not detached.
2. **Given** all worker qpairs have been freed, **When** the caller
   uses the primary `InnerState` for I/O, **Then** I/O still works.
3. **Given** worker qpairs, **When** the caller calls
   `close_device(primary)` on the primary state, **Then** the
   controller is detached. (Worker qpairs must be freed first.)

---

### Edge Cases

- What happens when `alloc_qpair` is called with a null `ctrlr`
  pointer? Undefined behavior — the function is `unsafe` and the caller
  must ensure valid pointers.
- What happens when the device runs out of hardware queue pairs? SPDK
  returns NULL from `spdk_nvme_ctrlr_alloc_io_qpair`, and
  `alloc_qpair` returns `QpairAllocationFailed`.
- What happens when `free_qpair` is called on the primary state?
  Only the qpair is freed (no controller detach). The caller must
  call `close_device` instead for the primary state.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: `alloc_qpair(ctrlr, ns, sector_size, num_sectors)` MUST
  allocate a new I/O queue pair from the given controller and return an
  `InnerState` with its own qpair but shared `ctrlr` and `ns` pointers.
- **FR-002**: `alloc_qpair` MUST return `QpairAllocationFailed` if
  `spdk_nvme_ctrlr_alloc_io_qpair` returns NULL.
- **FR-003**: `alloc_qpair` MUST be marked `unsafe` because the caller
  must guarantee that `ctrlr` and `ns` are valid pointers from a prior
  `open_device` call.
- **FR-004**: `free_qpair(state)` MUST free only the I/O queue pair
  without detaching the controller.
- **FR-005**: Worker `InnerState` instances MUST be usable with the
  existing `read_blocks`, `write_blocks`, `submit_read`, `submit_write`,
  and `poll_completions` functions.
- **FR-006**: Each worker qpair MUST be used from exactly one thread
  (SPDK's single-thread-per-qpair requirement). This is the caller's
  responsibility.

### Key Entities

- **alloc_qpair**: Unsafe function that creates a new `InnerState` with
  an independent qpair sharing the controller/namespace from the primary
  device. Located in `src/io.rs`.
- **free_qpair**: Safe function that frees only the qpair, leaving the
  controller attached. Located in `src/io.rs`.
- **InnerState**: Reused from the primary device API. Worker instances
  have independent `qpair` but aliased `ctrlr`/`ns` pointers.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: The `iops_bench` example demonstrates multi-threaded I/O
  with one qpair per worker thread, achieving linear IOPS scaling up to
  hardware saturation.
- **SC-002**: Worker qpairs can be allocated and freed independently
  without affecting the primary device's operation.
- **SC-003**: `cargo clippy -- -D warnings` produces no warnings for
  the multi-qpair code.

## Assumptions

- The caller is responsible for ensuring that worker qpairs are freed
  before `close_device` is called on the primary state. Failing to do
  so results in undefined behavior (double-free of the controller).
- The maximum number of hardware queue pairs is device-dependent.
  Typical NVMe SSDs support 64-128 queue pairs.
- `alloc_qpair` uses SPDK's default queue pair options (NULL opts).
  Custom queue depth or priority is out of scope.
