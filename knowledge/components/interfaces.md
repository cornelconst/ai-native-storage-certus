# interfaces

**Crate**: `interfaces`
**Path**: `components/interfaces/`
**Version**: 0.1.0

## Description

Centralized repository for all component interface trait definitions. Allows components to depend on interface definitions without coupling to implementation crates. SPDK-dependent interfaces are gated behind `features = ["spdk"]`.

## Interfaces Defined

| Interface | Feature | Methods |
|-----------|---------|---------|
| `IGreeter` | -- | `greeting_prefix(&self) -> &str` |
| `ILogger` | -- | `error`, `warn`, `info`, `debug` (all `&self, msg: &str`) |
| `ISPDKEnv` | `spdk` | `init()`, `devices()`, `device_count()`, `is_initialized()` |
| `IBlockDevice` | `spdk` | `connect_client()`, `sector_size(ns_id)`, `num_sectors(ns_id)`, `max_queue_depth()`, `num_io_queues()`, `max_transfer_size()`, `block_size()`, `numa_node()`, `nvme_version()`, `telemetry()` |
| `IBlockDeviceAdmin` | `spdk` | `set_pci_address(addr)`, `set_actor_cpu(cpu)`, `initialize()`, `shutdown()` |
| `IExtentManager` | `spdk` | `set_dma_alloc`, `format`, `initialize`, `reserve_extent`, `lookup_extent`, `get_extents`, `for_each_extent`, `remove_extent`, `checkpoint`, `get_instance_id` |
| `IDispatchMap` | `spdk` | `set_dma_alloc`, `initialize`, `create_staging`, `lookup`, `convert_to_storage`, `take_read`, `take_write`, `release_read`, `release_write`, `downgrade_reference`, `remove` |
| `IDispatcher` | `spdk` | `initialize()`, `shutdown()` |

## Key Shared Types

### General
- `PciAddress` -- PCI BDF address (`domain`, `bus`, `dev`, `func`)
- `PciId` -- vendor/device/class IDs
- `VfioDevice` -- SPDK-discovered NVMe device with address, id, numa_node

### Block Device
- `DmaBuffer` -- DMA-safe hugepage buffer with pluggable allocator/deallocator
- `DmaAllocFn` -- `Arc<dyn Fn(usize, usize, Option<i32>) -> Result<DmaBuffer, String> + Send + Sync>`
- `NvmeBlockError` -- error enum: `FeatureNotEnabled`, `NotInitialized`, `Timeout`, `Aborted`, `InvalidNamespace`, `NotSupported`, `BlockDevice`, `SpdkEnv`, `LbaOutOfRange`, `ClientDisconnected`
- `TelemetrySnapshot` -- `{total_ops, min/max/mean_latency_ns, mean_throughput_mbps, elapsed_secs}`
- `OpHandle(u64)` -- async operation handle
- `NamespaceInfo` -- `{ns_id, num_sectors, sector_size}`
- `ClientChannels` -- `{command_tx: Sender<Command>, completion_rx: Receiver<Completion>}`

### Messaging Protocol
- `Command` enum: `ReadSync`, `WriteSync`, `ReadAsync`, `WriteAsync`, `WriteZeros`, `BatchSubmit`, `AbortOp`, `NsProbe`, `NsCreate`, `NsFormat`, `NsDelete`, `ControllerReset`
- `Completion` enum: `ReadDone`, `WriteDone`, `WriteZerosDone`, `AbortAck`, `Timeout`, `NsProbeResult`, `NsCreated`, `NsFormatted`, `NsDeleted`, `ResetDone`, `Error`

### Extent Manager
- `ExtentKey = u64`
- `Extent { key, size, offset }`
- `FormatParams`, `WriteHandle`
- `ExtentManagerError`

### Dispatch Map
- `CacheKey = u64`
- `LookupResult`
- `DispatchMapError`

## Receptacles

None (trait definition crate only).
