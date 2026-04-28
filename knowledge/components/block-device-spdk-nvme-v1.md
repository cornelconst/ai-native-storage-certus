# block-device-spdk-nvme (v1)

**Crate**: `block-device-spdk-nvme`
**Path**: `components/block-device-spdk-nvme/v1/`
**Version**: 0.1.0
**Features**: `telemetry` (IO statistics), `spdk-test`

## Description

High-performance NVMe block device component using SPDK for direct userspace NVMe controller access. One instance per physical controller. The actor thread runs on a dedicated OS thread pinned to a core in the same NUMA zone as the NVMe controller.

Each client gets two SPSC channels (capacity 64):
- **Ingress**: client sends `Command` messages to the actor
- **Callback**: actor sends `Completion` notifications back to the client

Supports synchronous and asynchronous read/write, write-zeros, batch submission, abort, namespace management (probe/create/format/delete), and controller reset.

## Component Definition

```
BlockDeviceSpdkNvmeComponentV1 {
    version: "0.1.0",
    provides: [IBlockDevice, IBlockDeviceAdmin],
    receptacles: { spdk_env: ISPDKEnv, logger: ILogger },
    fields: {
        pci_address: RwLock<Option<PciAddress>>,
        actor_cpu: Mutex<Option<usize>>,
        controller_info: RwLock<Option<ControllerSnapshot>>,
        actor_handle: Mutex<Option<ActorHandle<ControlMessage>>>,
        next_client_id: AtomicU64,
        telemetry_stats: Mutex<Option<Arc<dyn Any + Send + Sync>>>,
    },
}
```

## Interfaces Provided

| Interface | Key Methods |
|-----------|------------|
| `IBlockDevice` | `connect_client()` -- returns `ClientChannels` for command/completion messaging |
|               | `sector_size(ns_id)`, `num_sectors(ns_id)` -- namespace geometry |
|               | `max_queue_depth()`, `num_io_queues()`, `max_transfer_size()` |
|               | `block_size()`, `numa_node()`, `nvme_version()` |
|               | `telemetry()` -- returns `TelemetrySnapshot` (requires `telemetry` feature) |
| `IBlockDeviceAdmin` | `set_pci_address(addr)` -- configure target controller |
|                     | `set_actor_cpu(cpu)` -- pin actor thread |
|                     | `initialize()` -- probe controller, start actor |
|                     | `shutdown()` -- stop actor, detach controller |

## Receptacles

| Name | Interface | Required | Purpose |
|------|-----------|----------|---------|
| `spdk_env` | `ISPDKEnv` | Yes | SPDK environment must be initialized before `initialize()` |
| `logger` | `ILogger` | No | Optional debug/info logging |

## Messaging API

Commands sent via `ClientChannels.command_tx`:
- `ReadSync` / `WriteSync` -- synchronous block I/O
- `ReadAsync` / `WriteAsync` -- async with `OpHandle`, timeout, callback completion
- `WriteZeros` -- zero a range of blocks
- `BatchSubmit` -- submit multiple operations atomically
- `AbortOp` -- cancel an async operation by handle
- `NsProbe` -- list active namespaces
- `NsCreate { size_sectors }` -- create namespace (lbaf=0)
- `NsFormat { ns_id, lbaf }` -- format with specified LBA format
- `NsDelete { ns_id }` -- delete namespace
- `ControllerReset` -- hardware controller reset

Completions received via `ClientChannels.completion_rx`:
- `ReadDone`, `WriteDone`, `WriteZerosDone`, `AbortAck`, `Timeout`
- `NsProbeResult`, `NsCreated`, `NsFormatted`, `NsDeleted`, `ResetDone`
- `Error { handle, error }`
