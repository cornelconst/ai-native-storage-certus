use crate::config::BenchmarkConfig;
use crate::worker::PhaseResult;

pub fn print_header(config: &BenchmarkConfig, total_size: u64) {
    println!("=== Extent Manager Benchmark ===");
    println!("Device: {} (ns_id={})", config.device, config.ns_id);
    println!(
        "Threads: {} | Count: {} | Size class: {}",
        config.threads, config.count, config.size_class
    );
    println!(
        "Slab size: {} | Total size: {}",
        config.slab_size, total_size
    );
    println!();
}

pub fn print_phase(result: &PhaseResult) {
    println!("--- {} Phase ---", result.phase_name);
    println!("  Total ops:   {}", result.total_ops);
    println!("  Elapsed:     {:.3}s", result.elapsed.as_secs_f64());
    println!("  Throughput:  {:.0} ops/sec", result.ops_per_sec);
    println!("  Latency ({} samples):", result.latency.count);
    println!("    min:  {:>8} us", result.latency.min.as_micros());
    println!("    mean: {:>8} us", result.latency.mean.as_micros());
    println!("    p50:  {:>8} us", result.latency.p50.as_micros());
    println!("    p99:  {:>8} us", result.latency.p99.as_micros());
    println!("    max:  {:>8} us", result.latency.max.as_micros());

    if result.per_thread.len() > 1 {
        println!("  Per-thread:");
        for w in &result.per_thread {
            println!(
                "    thread {}: {} ops, p50={} us, p99={} us",
                w.thread_id,
                w.ops_completed,
                w.latency.p50.as_micros(),
                w.latency.p99.as_micros(),
            );
        }
    }
    println!();
}

pub fn print_summary(count: u64) {
    println!("=== Summary ===");
    println!("Total extents created:  {}", count);
    println!("Total extents removed:  {}", count);
}
