# spdk-env

**Crate**: `spdk-env`
**Path**: `components/spdk-env/`
**Version**: 0.1.0

## Description

SPDK/DPDK userspace environment initialization component. Performs pre-flight checks (VFIO device availability, permissions, hugepages), calls `spdk_env_init`, enumerates VFIO-attached PCIe NVMe devices, and cleans up via `spdk_env_fini` on drop.

## Component Definition

```
SPDKEnvComponent {
    version: "0.1.0",
    provides: [ISPDKEnv],
    fields: {
        discovered_devices: RwLock<Vec<VfioDevice>>,
        initialized: AtomicBool,
    },
}
```

## Interfaces Provided

| Interface | Methods |
|-----------|---------|
| `ISPDKEnv` | `init() -> Result<(), SpdkEnvError>` -- initialize SPDK environment |
|           | `devices() -> Vec<VfioDevice>` -- list discovered NVMe devices |
|           | `device_count() -> usize` |
|           | `is_initialized() -> bool` |

## Receptacles

None.

## Internal Modules

- `checks` -- pre-flight validation (hugepages, VFIO, permissions)
- `device` -- PCI/VFIO device discovery
- `dma` -- `DmaBuffer` safe wrapper around SPDK DMA allocation
- `env` -- SPDK init/fini lifecycle
- `error` -- `SpdkEnvError` definitions

## Key Public Types

- `SPDKEnvComponent` -- the component struct
- `ISPDKEnv` -- the interface trait (local definition, mirrors `interfaces` crate)
- `PciAddress`, `PciId`, `VfioDevice` -- device metadata
- `DmaBuffer` -- DMA-safe hugepage buffer
- `SpdkEnvError` -- environment error enum
