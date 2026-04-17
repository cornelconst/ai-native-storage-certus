# CLI Contract: extent-benchmark

## Command Signature

```
extent-benchmark [OPTIONS] --device <PCI_ADDRESS>
```

## Required Arguments

| Argument | Type | Description |
|----------|------|-------------|
| `--device <PCI_ADDRESS>` | String | NVMe device PCI address (e.g., `0000:03:00.0`) |

## Optional Arguments

| Argument | Type | Default | Description |
|----------|------|---------|-------------|
| `--ns-id <ID>` | u32 | 1 | NVMe namespace ID |
| `--threads <N>` | usize | 1 | Number of worker threads |
| `--count <N>` | u64 | 10000 | Operations per benchmark phase |
| `--size-class <BYTES>` | u32 | 131072 | Extent size class (128 KiB) |
| `--slab-size <BYTES>` | u32 | 1073741824 | Slab size for extent manager init (1 GiB) |
| `--total-size <BYTES>` | u64 | auto | Total managed space (auto-detect from device) |
| `-h, --help` | flag | - | Print help |

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Benchmark completed successfully |
| 1 | Configuration error (invalid arguments, device not found) |
| 2 | SPDK initialization failure |

## Output Format

```
=== Extent Manager Benchmark ===
Device: 0000:03:00.0 (ns_id=1)
Threads: 4 | Count: 10000 | Size class: 131072

--- Create Phase ---
  Total ops:   10000
  Elapsed:     1.234s
  Throughput:  8103 ops/sec
  Latency:
    min:    45 us
    p50:   112 us
    p99:   890 us
    max:  2340 us

--- Lookup Phase ---
  Total ops:   10000
  Elapsed:     0.456s
  Throughput:  21929 ops/sec
  Latency:
    min:     8 us
    p50:    42 us
    p99:   210 us
    max:   550 us

--- Remove Phase ---
  Total ops:   10000
  Elapsed:     1.567s
  Throughput:  6381 ops/sec
  Latency:
    min:    50 us
    p50:   145 us
    p99:  1100 us
    max:  2800 us

=== Summary ===
Total extents created:  10000
Total extents removed:  10000
```

In multi-threaded mode, per-thread latency breakdowns appear under each phase.
