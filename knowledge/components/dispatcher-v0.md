# dispatcher (v0)

**Crate**: `dispatcher`
**Path**: `components/dispatcher/v0/`
**Version**: 0.1.0

## Description

Orchestrator component that ties together the block device admin lifecycle and the dispatch map. Manages the initialization and shutdown sequencing of the storage stack.

On `initialize`, verifies that both `block_device_admin` and `dispatch_map` receptacles are connected. On `shutdown`, performs orderly teardown and logging.

## Component Definition

```
DispatcherComponentV0 {
    version: "0.1.0",
    provides: [IDispatcher],
    receptacles: {
        logger: ILogger,
        block_device_admin: IBlockDeviceAdmin,
        dispatch_map: IDispatchMap,
    },
}
```

## Interfaces Provided

| Interface | Methods |
|-----------|---------|
| `IDispatcher` | `initialize() -> Result<(), DispatcherError>` -- verify receptacles, start subsystems |
|              | `shutdown() -> Result<(), DispatcherError>` -- orderly teardown |

## Receptacles

| Name | Interface | Required | Purpose |
|------|-----------|----------|---------|
| `logger` | `ILogger` | No | Optional logging |
| `block_device_admin` | `IBlockDeviceAdmin` | Yes | Block device lifecycle management |
| `dispatch_map` | `IDispatchMap` | Yes | Extent-to-location dispatch |

## Status

In progress. The `IDispatcher` and `DispatcherError` types are referenced from the `interfaces` crate but may not yet be fully integrated.
