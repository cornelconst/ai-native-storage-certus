# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is the `spdk-simple-block-device` component of the **Certus** project. It provides synchronous, zero-copy block I/O over SPDK's user-space NVMe driver, exposed both as a component-framework interface (`IBasicBlockDevice`) and as an actor-based client (`BlockDeviceClient`).

The component probes the first NVMe controller on the local PCIe bus, opens namespace 1, and wraps SPDK's async submit+poll NVMe commands into synchronous `read_blocks`/`write_blocks` calls. Callers provide `DmaBuffer` (hugepage-backed) memory directly — no intermediate copies.

## Build Commands

```bash
cargo build -p spdk-simple-block-device          # Build
cargo test -p spdk-simple-block-device            # Unit tests (no hardware needed)
cargo clippy -p spdk-simple-block-device -- -D warnings  # Lint
cargo doc -p spdk-simple-block-device --no-deps   # Docs
```

Examples require real NVMe hardware bound to vfio-pci with hugepages:
```bash
cargo run --example basic_io    # Component-based (IBasicBlockDevice)
cargo run --example actor_io    # Actor-based (BlockDeviceClient)
```

## Architecture

### Dependency Chain

```
spdk-sys          (raw FFI: env.h + nvme.h bindings via bindgen)
    |
spdk-env          (safe wrapper: ISPDKEnv init, VFIO checks, device enum, DmaBuffer)
    |
spdk-simple-block-device  (this crate: IBasicBlockDevice + actor-based BlockDeviceClient)
```

### Key Files

- `src/lib.rs` — `IBasicBlockDevice` interface and `SimpleBlockDevice` component. Receptacles: `spdk_env: ISPDKEnv`, `logger: ILogger`.
- `src/actor.rs` — Actor-based API: `BlockDeviceHandler` (processes NVMe I/O on a dedicated thread), `BlockIoRequest` message enum, `BlockDeviceClient` (synchronous client).
- `src/io.rs` — Standalone NVMe operations: `open_device`, `close_device`, `read_blocks`, `write_blocks`. Used by both the component and actor paths.
- `src/error.rs` — `BlockDeviceError` enum.
- `examples/basic_io.rs` — Component-based wiring example.
- `examples/actor_io.rs` — Actor-based example with buffer reuse.

### Zero-Copy I/O

Callers allocate `DmaBuffer` (from `spdk_env`) — hugepage-backed memory via `spdk_dma_zmalloc`. This buffer is passed directly to SPDK NVMe commands. No intermediate copies:
1. Caller allocates `DmaBuffer::new(size, align)`
2. Fills it (write) or leaves empty (read)
3. Passes to `read_blocks` / `write_blocks` — NVMe device reads/writes directly
4. Buffer returned to caller for reuse

In the actor path, the `DmaBuffer` transfers ownership through the channel to the actor thread and back — still zero-copy.

### Thread Safety

- **Component path**: `Mutex<Option<InnerState>>` serializes access to the qpair.
- **Actor path**: Dedicated actor thread owns the qpair exclusively. `DmaBuffer` is `Send`, so it transfers safely through channels.

## SPDK Prerequisites

Before running code that calls `open()`:
1. System deps: `../../deps/install_deps.sh`
2. SPDK built: `../../deps/build_spdk.sh`
3. NVMe devices bound to vfio-pci: `../../deps/spdk/scripts/setup.sh`
4. Hugepages allocated: `echo 1024 > /proc/sys/vm/nr_hugepages`
