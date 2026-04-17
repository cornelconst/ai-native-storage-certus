use std::time::Duration;

use crate::stats::{self, LatencyStats};

pub struct WorkerResult {
    pub thread_id: usize,
    pub ops_completed: u64,
    pub latency: LatencyStats,
}

pub struct PhaseResult {
    pub phase_name: String,
    pub total_ops: u64,
    pub elapsed: Duration,
    pub ops_per_sec: f64,
    pub latency: LatencyStats,
    pub per_thread: Vec<WorkerResult>,
}

pub fn aggregate_results(
    phase_name: &str,
    mut worker_latencies: Vec<(usize, Vec<Duration>)>,
    elapsed: Duration,
) -> PhaseResult {
    let mut all_samples: Vec<Duration> = Vec::new();
    let mut per_thread = Vec::new();
    let mut total_ops: u64 = 0;

    for (thread_id, ref mut latencies) in &mut worker_latencies {
        let ops = latencies.len() as u64;
        total_ops += ops;
        all_samples.extend(latencies.iter());
        let thread_stats = stats::compute_stats(latencies);
        per_thread.push(WorkerResult {
            thread_id: *thread_id,
            ops_completed: ops,
            latency: thread_stats,
        });
    }

    let ops_per_sec = total_ops as f64 / elapsed.as_secs_f64();
    let latency = stats::compute_stats(&mut all_samples);

    PhaseResult {
        phase_name: phase_name.to_string(),
        total_ops,
        elapsed,
        ops_per_sec,
        latency,
        per_thread,
    }
}
