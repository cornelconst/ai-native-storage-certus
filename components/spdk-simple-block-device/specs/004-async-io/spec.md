# Feature Specification: Async (Non-Blocking) I/O Primitives

**Feature Branch**: `004-async-io`
**Created**: 2026-04-14
**Status**: Complete
**Input**: Backfill specification for the non-blocking I/O submit/poll API

## User Scenarios & Testing *(mandatory)*

### User Story 1 - High-Throughput I/O with Queue Depth (Priority: P1)

A performance-oriented caller submits multiple NVMe I/O commands without
waiting for each to complete, then polls for completions in a tight
loop. This achieves high queue depth and maximizes device throughput.

**Why this priority**: The synchronous `read_blocks`/`write_blocks` API
(spec-001) submits one I/O and waits. For benchmarks and
performance-critical code, maintaining multiple outstanding I/Os per
qpair is essential.

**Independent Test**: Submit N reads/writes using `submit_read`/
`submit_write`, poll with `poll_completions`, verify all complete
successfully. Requires hardware.

**Acceptance Scenarios**:

1. **Given** an open device, **When** the caller calls
   `submit_read(state, lba, buf, cb, cb_arg)`, **Then** the NVMe read
   command is submitted to the qpair and the function returns
   immediately without waiting for completion.
2. **Given** one or more submitted I/Os, **When** the caller calls
   `poll_completions(state, 0)`, **Then** all completed I/Os fire their
   callbacks and the function returns the number of completions
   processed.
3. **Given** a submitted read with a `CompletionContext` as `cb_arg`,
   **When** the completion callback fires, **Then** `ctx.done` is set
   to `true` and `ctx.status` contains the NVMe completion status
   (0 = success).

---

### User Story 2 - IOPS Benchmark Pattern (Priority: P1)

A benchmark tool pre-allocates I/O slots (each with a `DmaBuffer` and
a `CompletionContext`), submits an initial burst to fill the queue, then
enters a poll-and-resubmit loop for the duration of the measurement.

**Why this priority**: This is the primary consumer of the async API
and validates the full submit/poll/resubmit cycle under sustained load.

**Independent Test**: Run the `iops_bench` example with configurable
threads, queue depth, block size, duration, read/write mix, and access
pattern.

**Acceptance Scenarios**:

1. **Given** `QD=32` and `THREADS=1`, **When** the benchmark runs for
   10 seconds, **Then** it reports IOPS, bandwidth, and average latency
   without errors.
2. **Given** `THREADS=4`, **When** each thread runs with its own qpair
   (via `alloc_qpair`, spec-003), **Then** total IOPS scales
   approximately linearly with thread count (up to device saturation).
3. **Given** `READ_PCT=0` (all writes), **When** the benchmark runs,
   **Then** write IOPS are reported without data corruption.

---

### User Story 3 - Custom Completion Callback (Priority: P3)

An advanced caller provides a custom NVMe completion callback (not
`io_completion_cb`) to implement specialized completion handling such
as latency histograms or error counters.

**Why this priority**: The built-in callback covers most cases, but
advanced users need the flexibility to hook completions.

**Independent Test**: Submit I/O with a custom callback that records
the completion timestamp, verify the callback fires.

**Acceptance Scenarios**:

1. **Given** a custom completion callback, **When** the caller passes
   it to `submit_read` or `submit_write`, **Then** the custom callback
   fires on completion with the correct `cb_arg` and NVMe completion
   status.

---

### Edge Cases

- What happens when `submit_read` or `submit_write` is called with a
  misaligned buffer? MUST return `BufferSizeMismatch` error without
  submitting to the qpair.
- What happens when the qpair is full (too many outstanding I/Os)?
  SPDK's `spdk_nvme_ns_cmd_read/write` returns a non-zero error code,
  and `submit_read`/`submit_write` returns `ReadFailed`/`WriteFailed`.
- What happens if `poll_completions` is never called? Submitted I/Os
  remain in-flight indefinitely. The caller is responsible for polling.
- What happens if `cb_arg` is freed before the completion fires?
  Undefined behavior — the function is `unsafe` and the caller must
  ensure `cb_arg` outlives the I/O.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: `submit_read(state, lba, buf, cb, cb_arg)` MUST submit an
  NVMe read command to the qpair and return immediately without waiting
  for completion.
- **FR-002**: `submit_write(state, lba, buf, cb, cb_arg)` MUST submit
  an NVMe write command to the qpair and return immediately without
  waiting for completion.
- **FR-003**: Both `submit_read` and `submit_write` MUST validate that
  the buffer length is a positive multiple of the sector size before
  submitting. Invalid buffers MUST return `BufferSizeMismatch`.
- **FR-004**: Both `submit_read` and `submit_write` MUST be marked
  `unsafe` because `cb_arg` must remain valid until the completion
  callback fires.
- **FR-005**: Both functions MUST return `ReadFailed`/`WriteFailed` if
  SPDK's `spdk_nvme_ns_cmd_read/write` returns a non-zero error code.
- **FR-006**: `poll_completions(state, max_completions)` MUST call
  `spdk_nvme_qpair_process_completions` and return the number of
  completions processed.
- **FR-007**: `poll_completions` with `max_completions = 0` MUST
  process all available completions.
- **FR-008**: `CompletionContext` MUST be a public struct with two
  fields: `done: &AtomicBool` and `status: &AtomicI32`.
- **FR-009**: `io_completion_cb` MUST be a public `unsafe extern "C"`
  function that sets `ctx.done = true` and stores the NVMe completion
  status (with phase and DNR bits masked off) in `ctx.status`.
- **FR-010**: The completion status extraction MUST mask off the phase
  bit (bit 0) and DNR bit (bit 15) from the raw NVMe status word,
  yielding bits 1-14 as the effective status.
- **FR-011**: If the NVMe completion pointer is null, `io_completion_cb`
  MUST store status `-1`.

### Key Entities

- **submit_read / submit_write**: Unsafe functions that submit NVMe
  commands without waiting. Accept a caller-provided completion callback
  and context pointer. Located in `src/io.rs`.
- **poll_completions**: Safe function that drives the qpair's
  completion queue. Returns the number of completions processed.
  Located in `src/io.rs`.
- **CompletionContext**: Public struct holding `&AtomicBool` (done flag)
  and `&AtomicI32` (status). Lifetime-parameterized (`'a`) to tie to
  the caller's stack. Located in `src/io.rs`.
- **io_completion_cb**: Public `unsafe extern "C"` NVMe completion
  callback. Casts `cb_arg` to `&CompletionContext` and signals
  completion. Located in `src/io.rs`.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: The `iops_bench` example runs to completion with
  configurable thread count, queue depth, block size, duration, read
  mix, and access pattern.
- **SC-002**: With `THREADS=1 QD=32`, the benchmark achieves device-
  level IOPS (hardware dependent, but significantly higher than QD=1
  synchronous I/O).
- **SC-003**: No I/O errors occur during sustained benchmark runs
  (all completion statuses are 0).
- **SC-004**: `cargo clippy -- -D warnings` produces no warnings for
  the async I/O code.

## Assumptions

- The caller is responsible for ensuring `cb_arg` remains valid until
  the completion callback fires. Stack-allocated `CompletionContext` is
  safe as long as the caller polls before the function returns.
- The caller is responsible for calling `poll_completions` — submitted
  I/Os do not complete autonomously.
- The async API is lower-level than the synchronous `read_blocks`/
  `write_blocks` (spec-001) and the actor API (spec-002). It is
  intended for performance-critical code and benchmarks.
- `CompletionContext` uses `Acquire`/`Release` ordering, which is
  sufficient because the qpair is accessed from a single thread.
