# spdk-sys

Raw unsafe FFI bindings to [SPDK](https://spdk.io/) (Storage Performance Development Kit) C libraries, generated at build time by [bindgen](https://github.com/rust-lang/rust-bindgen). Part of the Certus project.

## What Gets Generated

The build script generates Rust bindings for a subset of the SPDK C API:

- **Environment**: `spdk_env_opts_init`, `spdk_env_init`, `spdk_env_fini`
- **PCI**: `spdk_pci_*` enumeration functions
- **NVMe**: `spdk_nvme_probe`, `spdk_nvme_detach`, controller/namespace/qpair operations, IO submission and completion
- **DMA**: `spdk_dma_zmalloc`, `spdk_dma_free`, `spdk_zmalloc`, `spdk_free`
- **Types**: `spdk_env_opts`, `spdk_pci_addr`, `spdk_nvme_ctrlr`, `spdk_nvme_ns`, `spdk_nvme_qpair`, `spdk_nvme_transport_id`, and associated structs

## Prerequisites

SPDK must be pre-built before this crate can compile:

```bash
# Install system dependencies (RHEL/Fedora)
deps/install_deps.sh

# Build SPDK to deps/spdk-build/
deps/build_spdk.sh
```

The build script expects:
- SPDK source at `deps/spdk/` (relative to workspace root)
- Pre-built SPDK libraries at `deps/spdk-build/`

## Linked Libraries

The build script statically links:

- **SPDK**: `spdk_env_dpdk`, `spdk_log`, `spdk_util`, `spdk_nvme` (+whole-archive), `spdk_trace`, `spdk_dma`, `spdk_keyring`, `spdk_json`, `spdk_rpc`, `spdk_sock`, `spdk_sock_posix` (+whole-archive), `spdk_thread`
- **DPDK**: 28 `rte_*` static libraries (EAL, mempool, ring, PCI, VFIO, etc.)
- **System**: `pthread`, `dl`, `numa`, `uuid`, `ssl`, `crypto`, `m`, `fuse3`

## Build

```bash
cargo build -p spdk-sys
```

This crate is excluded from the workspace `default-members` and must be built explicitly.

## Test

```bash
cargo test -p spdk-sys
```

## Source Layout

```
build.rs     Build script: locates SPDK, runs bindgen, emits linker flags
wrapper.h    C header includes for bindgen (spdk/env.h, spdk/nvme.h, etc.)
src/lib.rs   Includes the generated bindings via include! macro
```
