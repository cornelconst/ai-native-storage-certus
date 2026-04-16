/// Per-thread IO worker for the IOPS benchmark.
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use interfaces::{ClientChannels, Command, Completion, DmaBuffer, NamespaceInfo, NvmeBlockError};

use crate::config::{BenchConfig, IoMode, OpType, Pattern};
use crate::lba::{LbaGenerator, RandomLba, SequentialLba};
use crate::stats::ThreadResult;

/// A per-thread IO worker that submits async operations and drains completions.
pub struct Worker {
    config: Arc<BenchConfig>,
    channels: ClientChannels,
    ns_info: NamespaceInfo,
    read_bufs: Vec<Arc<Mutex<DmaBuffer>>>,
    write_bufs: Vec<Arc<DmaBuffer>>,
    /// FIFO queue of (submit_time, is_read) for in-flight ops. Completions
    /// arrive in submission order per-client, so we pop from the front.
    in_flight: VecDeque<(Instant, bool)>,
    op_counter: Arc<AtomicU64>,
    stop_flag: Arc<AtomicBool>,
    submit_count: u64,
    lba_gen: Box<dyn LbaGenerator + Send>,
}

impl Worker {
    /// Create a new worker.
    ///
    /// `thread_index` is used for sequential LBA region partitioning.
    /// The caller must provide pre-connected `ClientChannels` and the `NamespaceInfo`
    /// for the target namespace.
    pub fn new(
        config: Arc<BenchConfig>,
        channels: ClientChannels,
        ns_info: NamespaceInfo,
        op_counter: Arc<AtomicU64>,
        stop_flag: Arc<AtomicBool>,
        thread_index: u32,
    ) -> Result<Self, NvmeBlockError> {
        let sector_size = ns_info.sector_size as usize;
        let blocks_per_io = (config.block_size / sector_size) as u64;

        // Pre-allocate DMA buffers for each queue depth slot.
        let mut read_bufs = Vec::with_capacity(config.queue_depth as usize);
        let mut write_bufs = Vec::with_capacity(config.queue_depth as usize);

        for _ in 0..config.queue_depth {
            let read_buf = DmaBuffer::new(config.block_size, sector_size, None)
                .map_err(NvmeBlockError::SpdkEnv)?;
            read_bufs.push(Arc::new(Mutex::new(read_buf)));

            let write_buf = DmaBuffer::new(config.block_size, sector_size, None)
                .map_err(NvmeBlockError::SpdkEnv)?;
            write_bufs.push(Arc::new(write_buf));
        }

        let lba_gen: Box<dyn LbaGenerator + Send> = match config.pattern {
            Pattern::Random => Box::new(RandomLba::new(ns_info.num_sectors, blocks_per_io)),
            Pattern::Sequential => Box::new(SequentialLba::new(
                thread_index,
                config.threads,
                ns_info.num_sectors,
                blocks_per_io,
            )),
        };

        Ok(Self {
            config,
            channels,
            ns_info,
            read_bufs,
            write_bufs,
            in_flight: VecDeque::new(),
            op_counter,
            stop_flag,
            submit_count: 0,
            lba_gen,
        })
    }

    /// Run the IO loop until the stop flag is set.
    ///
    /// Returns the collected per-thread statistics.
    pub fn run(&mut self) -> ThreadResult {
        let mut result = ThreadResult::default();
        let timeout_ms = (self.config.duration + 5) * 1000; // generous timeout

        // Fill the pipeline initially.
        while self.in_flight.len() < self.config.queue_depth as usize
            && !self.stop_flag.load(Ordering::Relaxed)
        {
            self.submit_one(timeout_ms);
        }

        // Main IO loop.
        loop {
            if self.stop_flag.load(Ordering::Relaxed) && self.in_flight.is_empty() {
                break;
            }

            // Drain completions (non-blocking).
            self.drain_completions(&mut result);

            // Re-submit to keep pipeline full.
            if !self.stop_flag.load(Ordering::Relaxed) {
                while self.in_flight.len() < self.config.queue_depth as usize {
                    self.submit_one(timeout_ms);
                }
            } else {
                // Draining: yield to let the actor process remaining in-flight ops.
                std::thread::yield_now();
            }
        }

        result
    }

    /// Submit a single IO operation (sync or async based on config).
    fn submit_one(&mut self, timeout_ms: u64) {
        let lba = self.lba_gen.next_lba();
        let slot = self.submit_count as usize % self.config.queue_depth as usize;
        let is_read = self.choose_is_read();

        let cmd = match (self.config.io_mode, is_read) {
            (IoMode::Async, true) => Command::ReadAsync {
                ns_id: self.ns_info.ns_id,
                lba,
                buf: Arc::clone(&self.read_bufs[slot]),
                timeout_ms,
            },
            (IoMode::Async, false) => Command::WriteAsync {
                ns_id: self.ns_info.ns_id,
                lba,
                buf: Arc::clone(&self.write_bufs[slot]),
                timeout_ms,
            },
            (IoMode::Sync, true) => Command::ReadSync {
                ns_id: self.ns_info.ns_id,
                lba,
                buf: Arc::clone(&self.read_bufs[slot]),
            },
            (IoMode::Sync, false) => Command::WriteSync {
                ns_id: self.ns_info.ns_id,
                lba,
                buf: Arc::clone(&self.write_bufs[slot]),
            },
        };

        if self.channels.command_tx.send(cmd).is_ok() {
            self.in_flight.push_back((Instant::now(), is_read));
            self.submit_count += 1;
        }
    }

    /// Choose whether the next op should be a read based on the configured OpType.
    fn choose_is_read(&self) -> bool {
        match self.config.op {
            OpType::Read => true,
            OpType::Write => false,
            OpType::ReadWrite => rand::random::<bool>(),
        }
    }

    /// Drain all available completions from the callback channel.
    fn drain_completions(&mut self, result: &mut ThreadResult) {
        while let Ok(completion) = self.channels.completion_rx.try_recv() {
            self.handle_completion(completion, result);
        }
    }

    /// Process a single completion message.
    fn handle_completion(&mut self, completion: Completion, result: &mut ThreadResult) {
        match completion {
            Completion::ReadDone { result: r, .. } => {
                if let Some((start, _)) = self.in_flight.pop_front() {
                    let latency_ns = start.elapsed().as_nanos() as u64;
                    result.latencies_ns.push(latency_ns);
                    if r.is_ok() {
                        result.read_ops += 1;
                        self.op_counter.fetch_add(1, Ordering::Relaxed);
                    } else {
                        result.errors += 1;
                    }
                }
            }
            Completion::WriteDone { result: r, .. } => {
                if let Some((start, _)) = self.in_flight.pop_front() {
                    let latency_ns = start.elapsed().as_nanos() as u64;
                    result.latencies_ns.push(latency_ns);
                    if r.is_ok() {
                        result.write_ops += 1;
                        self.op_counter.fetch_add(1, Ordering::Relaxed);
                    } else {
                        result.errors += 1;
                    }
                }
            }
            Completion::Timeout { .. } => {
                self.in_flight.pop_front();
                result.errors += 1;
            }
            Completion::Error { .. } => {
                self.in_flight.pop_front();
                result.errors += 1;
            }
            _ => {} // Ignore other completions
        }
    }
}
