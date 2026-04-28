# iops-benchmark

**Crate**: `iops-benchmark`
**Path**: `apps/iops-benchmark/`
**Type**: Application (not a component)

## Description

Multi-threaded NVMe IOPS/throughput/latency benchmark. Each worker thread gets its own `ClientChannels` from `IBlockDevice::connect_client()`. Worker and actor threads are NUMA-pinned. Measures and reports IOPS, MB/s, and latency percentiles (min, mean, p50, p99, max) per thread and aggregate.

## CLI Arguments

- `--pci-addr <BDF>` -- target NVMe controller
- `--driver <v1|v2>` -- block device driver version
- `--op <read|write>` -- operation type
- `--queue-depth <N>` -- async queue depth
- `--transfer-size <bytes>` -- I/O transfer size
- `--threads <N>` -- number of worker threads
- `--duration <secs>` -- benchmark duration
- `--random` -- random LBA access (vs sequential)
- `--quiet` -- suppress per-thread output

## Component Wiring

```
SPDKEnvComponent ---[ISPDKEnv]---> BlockDeviceSpdkNvmeComponent
                                        |
                                   [IBlockDevice] ---> N x ClientChannels (one per worker)
                                   [IBlockDeviceAdmin] ---> initialize
```

## Build

```bash
cargo build -p iops-benchmark --release
```
