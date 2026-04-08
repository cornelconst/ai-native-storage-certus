//! Actor-based block I/O example (zero-copy with DMA buffers).
//!
//! Wires the logger and SPDK environment components, then uses the actor-based
//! block device client to perform a write-read-verify cycle. DMA buffers are
//! allocated by the caller and passed through the actor channel — no copies.
//!
//! All NVMe operations run on the actor's dedicated thread, satisfying SPDK's
//! single-thread-per-qpair requirement without the caller needing to worry
//! about it.
//!
//! # Prerequisites
//!
//! - NVMe device(s) bound to `vfio-pci`
//! - Hugepages allocated (e.g. `echo 1024 > /proc/sys/vm/nr_hugepages`)
//! - Run with sufficient permissions (root or vfio group member)
//!
//! ```bash
//! cargo run --example actor_io
//! ```

use component_framework::actor::Actor;
use component_framework::iunknown::query;
use example_logger::{ILogger, LoggerComponent};
use spdk_env::{ISPDKEnv, SPDKEnvComponent};
use spdk_simple_block_device::{BlockDeviceClient, BlockDeviceHandler};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, RwLock};

fn main() {
    println!("=== Actor Block Device Example (Zero-Copy) ===\n");

    // --- Instantiate and wire infrastructure components ---

    let logger = LoggerComponent::new();
    let env_comp = SPDKEnvComponent::new(RwLock::new(Vec::new()), AtomicBool::new(false));

    let ilogger = query::<dyn ILogger + Send + Sync>(&*logger).expect("ILogger not found");
    env_comp
        .logger
        .connect(ilogger)
        .expect("env logger connect failed");

    // --- Initialize SPDK environment ---

    println!("Initializing SPDK environment...");
    env_comp.init().expect("SPDK environment init failed");
    println!(
        "SPDK environment initialized. Found {} device(s).\n",
        env_comp.device_count()
    );
    for dev in env_comp.devices() {
        println!("  Device: {} ({})", dev.address, dev.device_type);
    }

    // --- Create actor-based block device ---

    let env: Arc<dyn ISPDKEnv + Send + Sync> =
        query::<dyn ISPDKEnv + Send + Sync>(&*env_comp).expect("ISPDKEnv not found");

    let handler = BlockDeviceHandler::new(env);
    let actor = Actor::simple(handler);
    let handle = actor.activate().expect("actor activation failed");
    let client = BlockDeviceClient::new(handle);

    println!("\nActor started. Opening block device...");

    // --- Open ---

    let info = client.open().expect("open failed");
    println!(
        "Block device open: sector_size={}, num_sectors={}, capacity={}MB\n",
        info.sector_size,
        info.num_sectors,
        (info.num_sectors * info.sector_size as u64) / (1024 * 1024)
    );

    // --- Write-Read-Verify cycle (zero-copy) ---

    let test_lba = info.num_sectors - 1;
    println!("Writing test pattern to LBA {test_lba}...");

    // Allocate DMA buffer and fill with test pattern.
    let mut write_buf = client.alloc_dma_buffer(1).expect("DMA alloc failed");
    for (i, byte) in write_buf.as_mut_slice().iter_mut().enumerate() {
        *byte = (i % 251) as u8;
    }

    // Keep a copy of the pattern for verification (small cost, test only).
    let pattern: Vec<u8> = write_buf.as_slice().to_vec();

    // Zero-copy write: DMA buffer goes to actor, comes back.
    let write_buf = client
        .write_blocks(test_lba, write_buf)
        .expect("write failed");
    println!("Write complete. Buffer returned to caller.");

    // Zero-copy read: send a fresh DMA buffer, get it back with data.
    println!("Reading back LBA {test_lba}...");
    let read_buf = client.alloc_dma_buffer(1).expect("DMA alloc failed");
    let read_buf = client.read_blocks(test_lba, read_buf).expect("read failed");
    println!("Read complete.");

    if read_buf.as_slice() == pattern.as_slice() {
        println!("Verification PASSED: read data matches written data.");
    } else {
        eprintln!("Verification FAILED: read data does not match written data!");
        let mismatches: Vec<_> = read_buf
            .as_slice()
            .iter()
            .zip(pattern.iter())
            .enumerate()
            .filter(|(_, (r, w))| r != w)
            .take(10)
            .collect();
        for (i, (r, w)) in &mismatches {
            eprintln!("  byte[{i}]: read={r:#04x}, expected={w:#04x}");
        }
        std::process::exit(1);
    }

    // --- Multi-sector I/O (zero-copy, buffer reuse) ---

    let multi_count: u32 = 4;
    let multi_lba = info.num_sectors - (multi_count as u64) - 1;
    println!("\nWriting {multi_count} sectors starting at LBA {multi_lba}...");

    let mut multi_buf = client
        .alloc_dma_buffer(multi_count)
        .expect("DMA alloc failed");
    for (i, byte) in multi_buf.as_mut_slice().iter_mut().enumerate() {
        *byte = ((i * 7 + 13) % 256) as u8;
    }
    let multi_pattern: Vec<u8> = multi_buf.as_slice().to_vec();

    let multi_buf = client
        .write_blocks(multi_lba, multi_buf)
        .expect("multi-sector write failed");
    println!("Multi-sector write complete.");

    // Reuse the same DMA buffer for reading back.
    println!("Reading back {multi_count} sectors (reusing write buffer)...");
    // Zero the buffer to prove the read fills it.
    let mut multi_buf = multi_buf;
    multi_buf.as_mut_slice().fill(0);
    let multi_buf = client
        .read_blocks(multi_lba, multi_buf)
        .expect("multi-sector read failed");
    println!("Multi-sector read complete.");

    if multi_buf.as_slice() == multi_pattern.as_slice() {
        println!("Multi-sector verification PASSED.");
    } else {
        eprintln!("Multi-sector verification FAILED!");
        std::process::exit(1);
    }

    // --- Device info query ---

    let queried_info = client.info().expect("info query failed");
    assert_eq!(queried_info.sector_size, info.sector_size);
    assert_eq!(queried_info.num_sectors, info.num_sectors);
    println!("\nDevice info query confirmed: {:?}", queried_info);

    // --- Demonstrate buffer reuse ---

    println!("\nDemonstrating buffer reuse across operations...");
    // write_buf was returned from write_blocks earlier — reuse it for a read.
    let reused = client
        .read_blocks(test_lba, write_buf)
        .expect("reuse read failed");
    assert_eq!(reused.as_slice(), pattern.as_slice());
    println!("Buffer reuse PASSED.");

    // --- Close and shutdown ---

    println!("\nClosing block device...");
    client.close().expect("close failed");
    println!("Block device closed.");

    println!("Shutting down actor...");
    client.shutdown().expect("shutdown failed");
    println!("Actor stopped.");

    println!("\n=== Done ===");
}
