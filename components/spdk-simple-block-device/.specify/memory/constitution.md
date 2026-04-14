# SPDK Simple Block Device Constitution

## Core Principles

### I. Zero-Copy I/O First

All data-path operations MUST use zero-copy semantics. Callers provide
`DmaBuffer` (hugepage-backed) memory directly to NVMe commands — no
intermediate copies are permitted on the I/O path.

- Read and write operations MUST accept caller-allocated `DmaBuffer`
  references and pass them directly to SPDK NVMe commands.
- The actor path MUST transfer `DmaBuffer` ownership through channels
  without copying buffer contents.
- Any new I/O path MUST preserve the zero-copy guarantee.

### II. Unsafe-but-Sound FFI

All interactions with SPDK's C API use `unsafe` blocks that MUST be
demonstrably sound.

- Every `unsafe` block MUST have a `// SAFETY:` comment explaining why
  the invariants are upheld.
- Raw pointer lifetimes MUST be clearly bounded: between `open_device`
  and `close_device` for device pointers, between submit and completion
  for callback context pointers.
- `Send` implementations on types containing raw pointers MUST be
  justified with a safety argument about thread ownership.

### III. Single-Thread-per-Qpair

SPDK requires that each I/O queue pair is accessed from exactly one
thread. This invariant MUST be enforced architecturally.

- The component path MUST serialize qpair access with a `Mutex`.
- The actor path MUST confine all qpair operations to the actor's
  dedicated thread.
- Multi-qpair code MUST allocate one qpair per worker thread with no
  cross-thread sharing of qpair pointers.
- New I/O paths MUST document how they satisfy this invariant.

### IV. Explicit Lifecycle Management

Device resources (controllers, queue pairs, namespaces) have explicit
open/close lifecycles that MUST be respected.

- `open()` MUST validate all prerequisites (env initialized, receptacles
  connected) before acquiring hardware resources.
- `close()` MUST release all hardware resources (free qpair, detach
  controller) in the correct order.
- `Drop` MUST clean up if the caller forgets to call `close()`, logging
  a warning.
- Double-open and close-when-not-open MUST return clear errors, not
  panic.

### V. Comprehensive Error Reporting

Every failure mode MUST be represented by a specific `BlockDeviceError`
variant with an actionable message.

- Error messages MUST tell the caller what to do (e.g., "Call
  ISPDKEnv::init() first" rather than "env not ready").
- New error conditions MUST get a dedicated variant; reusing generic
  variants is not permitted.
- All error variants MUST implement `Display`, `Debug`, `Clone`, and
  `std::error::Error`.

## Platform and Toolchain Constraints

- **Target OS**: Linux only. SPDK and VFIO are Linux-specific.
- **Language**: Rust (stable toolchain).
- **Hardware**: Requires NVMe device(s) bound to `vfio-pci` and
  configured hugepages for any code that calls `open()`.
- **Dependencies**: `spdk-sys` (FFI bindings), `spdk-env` (safe SPDK
  wrapper), `component-framework` (COM-style component model),
  `example-logger` (logging interface).
- **CI gate**: `cargo clippy -- -D warnings && cargo test` MUST pass.
  Integration tests requiring hardware run manually.

## Development Workflow

- Unit tests MUST cover all error paths and pre-flight validation (no
  hardware needed).
- Integration tests (requiring real NVMe) are in `examples/` and run
  manually with hardware present.
- New public APIs MUST have doc comments with usage examples.
- Changes to the `IBasicBlockDevice` interface MUST be reflected in both
  the local `define_interface!` and the `interfaces` crate definition.

## Governance

- This constitution supersedes ad-hoc conventions for this crate.
- Amendments require a version bump and review.

**Version**: 1.0.0 | **Ratified**: 2026-04-14 | **Last Amended**: 2026-04-14
