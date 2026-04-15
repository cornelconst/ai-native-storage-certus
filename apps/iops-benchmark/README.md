# iops-benchmark

NVMe IOPS benchmark using the `block-device-spdk-nvme` component. Measures read/write IOPS, throughput (MB/s), and latency percentiles (min, mean, p50, p99, max) for NVMe devices via SPDK userspace drivers.

## Prerequisites

- Linux host with hugepages configured and NVMe device bound to VFIO/UIO
- SPDK built at `deps/spdk-build/` (run `deps/build_spdk.sh`)
- Rust stable toolchain (1.75+)

## Build

```bash
cargo build -p iops-benchmark --release
```

## Usage

```
iops-benchmark [OPTIONS]
```

### Options

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--op` | `read\|write\|rw` | `read` | Operation type |
| `--block-size` | bytes | `4096` | IO block size in bytes |
| `--queue-depth` | u32 | `32` | Outstanding IOs per thread |
| `--threads` | u32 | `1` | Number of concurrent client threads |
| `--duration` | seconds | `10` | Test duration |
| `--ns-id` | u32 | `1` | NVMe namespace ID |
| `--pci-addr` | string | (first device) | NVMe controller PCI BDF address |
| `--pattern` | `random\|sequential` | `random` | IO access pattern |
| `--quiet` | flag | off | Suppress per-second progress |

### Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Benchmark completed successfully |
| 1 | Validation error (invalid parameters) |
| 2 | Fatal error (device not found, SPDK init failed) |

## Examples

```bash
# Default: 4KB random reads, QD=32, 1 thread, 10 seconds, first available device
sudo ./target/release/iops-benchmark

# 64KB sequential writes, 4 threads, 30 seconds, specific device
sudo ./target/release/iops-benchmark \
  --op write \
  --block-size 65536 \
  --queue-depth 64 \
  --threads 4 \
  --duration 30 \
  --pci-addr 0000:03:00.0 \
  --pattern sequential

# Mixed read/write, quiet mode (no per-second progress)
sudo ./target/release/iops-benchmark --op rw --quiet
```

## Sample Output

```
=== IOPS Benchmark ===
Device:       0000:03:00.0
Namespace:    1 (953869 sectors, 512B sectors)
Operation:    read
Pattern:      random
Block size:   4096 bytes
Queue depth:  32
Threads:      1
Duration:     10 seconds

[  1s] 145230 IOPS
[  2s] 148102 IOPS
...

=== Results ===
Duration:     10.00 seconds
Total ops:    1,478,234
Total IOPS:   147,823
Throughput:   577.4 MB/s
Errors:       0

Latency (us):
  min:    2.1
  mean:   6.8
  p50:    5.9
  p99:    18.4
  max:    142.7
```

## Tests

```bash
cargo test -p iops-benchmark
```

Unit tests cover configuration validation, IOPS/throughput/latency statistics, LBA generation strategies, and output formatting. Tests run without SPDK hardware.
