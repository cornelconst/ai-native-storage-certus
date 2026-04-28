# nvme-ns-manager

**Crate**: `nvme-ns-manager`
**Path**: `apps/nvme-ns-manager/`
**Type**: Application (not a component)

## Description

Interactive text-based NVMe namespace management tool. Wires `SPDKEnvComponent` to either `BlockDeviceSpdkNvmeComponentV1` or `BlockDeviceSpdkNvmeComponentV2` (selected by `--driver` CLI flag). Initializes SPDK, selects a device by PCI address, then runs an interactive menu loop.

## Menu Operations

1. **List namespaces** -- shows ns_id, sector count, sector size, capacity
2. **Create namespace** -- specify size in sectors (uses lbaf=0)
3. **Create namespace (all remaining capacity)** -- uses all unallocated space
4. **Format namespace** -- specify ns_id and LBAF index (e.g., 2 for 4KiB sectors)
5. **Delete namespace** -- specify ns_id
6. **Quit**

## CLI Arguments

- `--pci-addr <BDF>` -- target NVMe controller (default: first device)
- `--driver <v1|v2>` -- block device driver version (default: v2)

## Component Wiring

```
SPDKEnvComponent ---[ISPDKEnv]---> BlockDeviceSpdkNvmeComponent (v1 or v2)
                                        |
                                   [IBlockDevice] ---> ClientChannels
                                   [IBlockDeviceAdmin] ---> set_pci_address, initialize
```

## Build

```bash
cargo build -p nvme-ns-manager --release
```
