/// Human-readable output formatting for the benchmark.
use crate::config::OpType;
use crate::stats::FinalReport;
use interfaces::NamespaceInfo;

use crate::config::BenchConfig;

/// Print the configuration summary to stdout.
pub fn print_config(config: &BenchConfig, pci_addr_str: &str, ns_info: &NamespaceInfo) {
    println!("=== IOPS Benchmark ===");
    println!("Device:       {}", pci_addr_str);
    println!(
        "Namespace:    {} ({} sectors, {}B sectors)",
        ns_info.ns_id, ns_info.num_sectors, ns_info.sector_size
    );
    println!("Operation:    {}", config.op);
    println!("Pattern:      {}", config.pattern);
    println!("Block size:   {} bytes", config.block_size);
    println!("Queue depth:  {}", config.queue_depth);
    println!("Threads:      {}", config.threads);
    println!("Duration:     {} seconds", config.duration);
}

/// Print a per-second progress line to stderr.
pub fn print_progress(elapsed_secs: u64, instant_iops: u64) {
    eprintln!("[{:3}s] {} IOPS", elapsed_secs, instant_iops);
}

/// Print the final benchmark report to stdout.
pub fn print_final(report: &FinalReport, op_type: OpType) {
    println!("=== Results ===");
    println!("Duration:     {:.2} seconds", report.duration_secs);

    if op_type == OpType::ReadWrite {
        println!(
            "Read ops:     {}  ({} IOPS)",
            format_count(report.total_read_ops),
            format_count(report.read_iops as u64)
        );
        println!(
            "Write ops:    {}  ({} IOPS)",
            format_count(report.total_write_ops),
            format_count(report.write_iops as u64)
        );
    }

    let total_ops = report.total_read_ops + report.total_write_ops;
    println!("Total ops:    {}", format_count(total_ops));
    println!("Total IOPS:   {}", format_count(report.total_iops as u64));
    println!("Throughput:   {:.1} MB/s", report.throughput_mbps);
    println!("Errors:       {}", report.total_errors);
    println!();
    println!("Latency (us):");
    println!("  min:    {:.1}", report.lat_min_us);
    println!("  mean:   {:.1}", report.lat_mean_us);
    println!("  p50:    {:.1}", report.lat_p50_us);
    println!("  p99:    {:.1}", report.lat_p99_us);
    println!("  max:    {:.1}", report.lat_max_us);
}

/// Format a number with comma separators.
fn format_count(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_count_small() {
        assert_eq!(format_count(0), "0");
        assert_eq!(format_count(999), "999");
    }

    #[test]
    fn format_count_with_commas() {
        assert_eq!(format_count(1_000), "1,000");
        assert_eq!(format_count(1_000_000), "1,000,000");
        assert_eq!(format_count(1_478_234), "1,478,234");
    }
}
