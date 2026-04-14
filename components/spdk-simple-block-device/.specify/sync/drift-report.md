# Spec Drift Report

Generated: 2026-04-14T00:00:00Z
Project: spdk-simple-block-device

## Summary

| Category | Count |
|----------|-------|
| Specs Analyzed | 0 |
| Requirements Checked | 0 |
| Aligned | 0 (0%) |
| Drifted | 0 (0%) |
| Not Implemented | 0 (0%) |
| Unspecced Code | 8 |

**No formal spec files found.** The `.specify` framework is initialized (`.specify/init-options.json` present, templates available) but no `specs/*/spec.md` files have been authored. The constitution (`.specify/memory/constitution.md`) contains only the default template placeholders.

All implemented features are therefore **unspecced** — there are no formal requirements to drift against.

## Detailed Findings

### No Specs Found

Search locations checked:
- `specs/` directory: does not exist
- `*.spec.md` files: none found
- `.specify/memory/constitution.md`: default template (unfilled)

## Unspecced Code

The following implemented features have no corresponding specification. Requirements are inferred from the `IBasicBlockDevice` interface definition and CLAUDE.md documentation.

### Feature Inventory

| # | Feature | Location | Lines | Suggested Spec |
|---|---------|----------|-------|----------------|
| 1 | `IBasicBlockDevice` interface (component path) | `src/lib.rs` | 185 | spec-001-basic-block-device |
| 2 | Low-level NVMe I/O operations | `src/io.rs` | 490 | spec-002-nvme-io-operations |
| 3 | Actor-based block device (handler + client) | `src/actor.rs` | 353 | spec-003-actor-block-device |
| 4 | Error types | `src/error.rs` | 193 | spec-001-basic-block-device |
| 5 | Multi-qpair support (`alloc_qpair`, `free_qpair`) | `src/io.rs:280-317` | 38 | spec-004-multi-qpair |
| 6 | Async I/O primitives (`submit_read`, `submit_write`, `poll_completions`) | `src/io.rs:322-416` | 95 | spec-005-async-io |
| 7 | IOPS benchmark example | `examples/iops_bench.rs` | 372 | spec-006-iops-benchmark |
| 8 | Basic I/O example (component wiring) | `examples/basic_io.rs` | 128 | (example, no spec needed) |
| 9 | Actor I/O example | `examples/actor_io.rs` | 186 | (example, no spec needed) |

### Inferred Requirements (from `IBasicBlockDevice` interface)

These are implicit requirements derived from the interface contract in `src/lib.rs:62-92` and `components/interfaces/src/iblock_device.rs`:

| ID | Inferred Requirement | Implementation Status |
|----|----------------------|----------------------|
| IR-001 | `open()` MUST probe NVMe, attach first controller, open NS 1 | Implemented (`io::open_device`) |
| IR-002 | `read_blocks(lba, buf)` MUST perform zero-copy read via DMA buffer | Implemented (`io::read_blocks`) |
| IR-003 | `write_blocks(lba, buf)` MUST perform zero-copy write via DMA buffer | Implemented (`io::write_blocks`) |
| IR-004 | `close()` MUST free qpair and detach controller | Implemented (`io::close_device`) |
| IR-005 | `sector_size()` MUST return sector size in bytes, 0 if not open | Implemented |
| IR-006 | `num_sectors()` MUST return total sectors, 0 if not open | Implemented |
| IR-007 | `is_open()` MUST report open/closed state | Implemented |
| IR-008 | Buffer length MUST be a positive multiple of sector size | Validated in `io::read_blocks` and `io::write_blocks` |
| IR-009 | `open()` MUST fail with `LoggerNotConnected` if logger not wired | Implemented + tested |
| IR-010 | `open()` MUST fail with `EnvNotInitialized` if spdk_env not wired | Implemented + tested |
| IR-011 | `open()` MUST fail with `AlreadyOpen` if already open | Implemented |
| IR-012 | `close()` MUST fail with `NotOpen` if not open | Implemented + tested |
| IR-013 | Drop MUST clean up if device still open | Implemented (`Drop for SimpleBlockDevice`) |
| IR-014 | Component MUST provide `IBasicBlockDevice` via `query` | Implemented + tested |
| IR-015 | Actor path MUST satisfy single-thread-per-qpair invariant | Implemented (dedicated actor thread) |
| IR-016 | Actor `on_stop` MUST close device if still open | Implemented |

### Dual Interface Note

The crate provides **two** access paths to NVMe I/O:
1. **Component path** (`SimpleBlockDevice` implementing `IBasicBlockDevice`) — `Mutex`-serialized
2. **Actor path** (`BlockDeviceHandler` + `BlockDeviceClient`) — dedicated thread, channel-based

Both share the same `io.rs` low-level operations. No spec formally defines when to use which path or their respective guarantees.

## Inter-Spec Conflicts

N/A — no specs exist to conflict.

## Recommendations

1. **Create `spec-001-basic-block-device`**: Formalize the `IBasicBlockDevice` interface contract including open/close lifecycle, zero-copy semantics, error conditions (IR-001 through IR-013), and thread safety guarantees. This is the most critical spec — the interface is the primary API surface.

2. **Create `spec-003-actor-block-device`**: Spec the actor-based path — `BlockDeviceHandler`, `BlockDeviceClient`, `BlockIoRequest` message types, DMA buffer ownership transfer, and shutdown semantics. Clarify when callers should prefer actor vs. component path.

3. **Create `spec-004-multi-qpair`**: Spec the multi-qpair API (`alloc_qpair`, `free_qpair`) including safety invariants (shared ctrlr/ns, independent qpairs, thread ownership).

4. **Create `spec-005-async-io`**: Spec the non-blocking I/O primitives (`submit_read`, `submit_write`, `poll_completions`) and their completion callback contract. These are public APIs used by the IOPS benchmark.

5. **Fill in the constitution**: `.specify/memory/constitution.md` is still the default template. Define the project's core principles (zero-copy first, unsafe-but-sound FFI, single-thread-per-qpair, etc.) to guide future spec authoring.

6. **Add interface spec cross-reference**: The `IBasicBlockDevice` interface is defined in **two** places — `components/interfaces/src/iblock_device.rs` and `src/lib.rs` (via `define_interface!`). A spec should clarify the canonical definition and ensure they stay synchronized. Currently both copies appear identical.
