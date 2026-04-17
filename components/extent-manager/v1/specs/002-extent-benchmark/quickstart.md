# Quickstart: Extent Manager Benchmark

## Prerequisites

1. Linux x86_64 system with an NVMe SSD
2. SPDK prerequisites: hugepages allocated, VFIO/UIO driver bound to the NVMe device
3. Rust stable toolchain (MSRV 1.75)

## Build

```bash
cargo build -p extent-benchmark --release
```

## Run

### Basic single-threaded benchmark (10K ops)

```bash
sudo ./target/release/extent-benchmark --device 0000:03:00.0
```

### Multi-threaded benchmark (4 threads, 50K ops)

```bash
sudo ./target/release/extent-benchmark --device 0000:03:00.0 --threads 4 --count 50000
```

### Custom size class and slab size

```bash
sudo ./target/release/extent-benchmark \
  --device 0000:03:00.0 \
  --threads 8 \
  --count 100000 \
  --size-class 262144 \
  --slab-size 2147483648
```

## Expected Behavior

1. Tool initializes SPDK, probes the NVMe device, and wires the extent manager component
2. Runs three phases in order: create → lookup → remove
3. Prints latency percentiles and throughput for each phase
4. Exits cleanly with status 0

## Validation Scenarios

| Scenario | Command | Expected Output |
|----------|---------|-----------------|
| Default single-thread | `--device <addr>` | 3 phases, 10K ops each, latency stats printed |
| Multi-thread | `--device <addr> --threads 4` | Per-thread + aggregate stats |
| Small workload | `--device <addr> --count 100` | Completes in < 1 second |
| Invalid device | `--device 9999:99:99.9` | Error message, exit code 1 |
| Invalid size class | `--device <addr> --size-class 999` | Error message, exit code 1 |
