# block-device-spdk-nvme (v2)

**Crate**: `block-device-spdk-nvme-v2`
**Path**: `components/block-device-spdk-nvme/v2/`
**Version**: 0.2.0
**Features**: `telemetry` (IO statistics), `spdk-test`

## Description

Version 2 of the NVMe block device component. Architecture is identical to v1 -- one actor thread per controller, SPSC client channels, same interface and messaging API. Both v1 and v2 can be used concurrently in applications with a runtime `--driver v1|v2` flag.

## Component Definition

```
BlockDeviceSpdkNvmeComponentV2 {
    version: "0.2.0",
    provides: [IBlockDevice, IBlockDeviceAdmin],
    receptacles: { spdk_env: ISPDKEnv, logger: ILogger },
    fields: { ... },  // same structure as v1
}
```

## Interfaces Provided

| Interface | Key Methods |
|-----------|------------|
| `IBlockDevice` | `connect_client()`, `sector_size()`, `num_sectors()`, `max_queue_depth()`, `num_io_queues()`, `max_transfer_size()`, `block_size()`, `numa_node()`, `nvme_version()`, `telemetry()` |
| `IBlockDeviceAdmin` | `set_pci_address()`, `set_actor_cpu()`, `initialize()`, `shutdown()` |

## Receptacles

| Name | Interface | Required | Purpose |
|------|-----------|----------|---------|
| `spdk_env` | `ISPDKEnv` | Yes | SPDK environment must be initialized first |
| `logger` | `ILogger` | No | Optional debug/info logging |

## Messaging API

Same `Command`/`Completion` protocol as v1. See [block-device-spdk-nvme-v1.md](block-device-spdk-nvme-v1.md) for the full messaging reference.
