//! Build script for spdk-sys: generates FFI bindings via bindgen and links SPDK/DPDK libraries.

use std::env;
use std::path::PathBuf;

fn main() {
    // Locate the SPDK build directory relative to the workspace root.
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let spdk_build = manifest_dir.join("../../deps/spdk-build");
    let spdk_build = spdk_build.canonicalize().expect(
        "SPDK build directory not found at deps/spdk-build/. Run deps/build_spdk.sh first.",
    );

    let include_dir = spdk_build.join("include");
    let lib_dir = spdk_build.join("lib");

    // Emit link search path.
    println!("cargo:rustc-link-search=native={}", lib_dir.display());

    // Link SPDK libraries we need (static).
    println!("cargo:rustc-link-lib=static=spdk_env_dpdk");
    println!("cargo:rustc-link-lib=static=spdk_log");
    println!("cargo:rustc-link-lib=static=spdk_util");

    // Link DPDK libraries (static).
    // These are the libraries that spdk_env_dpdk depends on, discovered from
    // the spdk_env_dpdk.pc and libdpdk.pc files. We link them manually because
    // DPDK's .pc files use -l:libfoo.a syntax which the Rust pkg-config crate
    // cannot handle.
    let dpdk_libs = [
        "rte_eal",
        "rte_kvargs",
        "rte_log",
        "rte_telemetry",
        "rte_argparse",
        "rte_mempool_ring",
        "rte_mempool",
        "rte_ring",
        "rte_bus_pci",
        "rte_bus_vdev",
        "rte_pci",
        "rte_power",
        "rte_timer",
        "rte_power_acpi",
        "rte_power_amd_pstate",
        "rte_power_cppc",
        "rte_power_intel_pstate",
        "rte_power_intel_uncore",
        "rte_power_kvm_vm",
        "rte_vhost",
        "rte_ethdev",
        "rte_meter",
        "rte_cryptodev",
        "rte_dmadev",
        "rte_hash",
        "rte_net",
        "rte_mbuf",
        "rte_rcu",
        "rte_cmdline",
        "rte_stack",
    ];

    for lib in &dpdk_libs {
        println!("cargo:rustc-link-lib=static={lib}");
    }

    // Link system libraries that SPDK/DPDK depend on.
    println!("cargo:rustc-link-lib=dylib=pthread");
    println!("cargo:rustc-link-lib=dylib=dl");
    println!("cargo:rustc-link-lib=dylib=numa");
    println!("cargo:rustc-link-lib=dylib=uuid");
    println!("cargo:rustc-link-lib=dylib=ssl");
    println!("cargo:rustc-link-lib=dylib=crypto");
    println!("cargo:rustc-link-lib=dylib=m");

    // Generate bindings with bindgen.
    let builder = bindgen::Builder::default()
        .header("wrapper.h")
        .clang_arg(format!("-I{}", include_dir.display()));

    let bindings = builder
        .allowlist_function("spdk_env_opts_init")
        .allowlist_function("spdk_env_init")
        .allowlist_function("spdk_env_fini")
        .allowlist_function("spdk_pci_enumerate")
        .allowlist_function("spdk_pci_for_each_device")
        .allowlist_function("spdk_pci_get_driver")
        .allowlist_function("spdk_pci_device_get_addr")
        .allowlist_function("spdk_pci_device_get_id")
        .allowlist_function("spdk_pci_device_get_domain")
        .allowlist_function("spdk_pci_device_get_bus")
        .allowlist_function("spdk_pci_device_get_dev")
        .allowlist_function("spdk_pci_device_get_func")
        .allowlist_function("spdk_pci_device_get_vendor_id")
        .allowlist_function("spdk_pci_device_get_device_id")
        .allowlist_function("spdk_pci_device_get_subvendor_id")
        .allowlist_function("spdk_pci_device_get_subdevice_id")
        .allowlist_function("spdk_pci_device_get_numa_id")
        .allowlist_function("spdk_pci_device_get_serial_number")
        .allowlist_type("spdk_env_opts")
        .allowlist_type("spdk_pci_addr")
        .allowlist_type("spdk_pci_id")
        .allowlist_type("spdk_pci_device")
        .allowlist_type("spdk_pci_driver")
        .allowlist_var("SPDK_PCI_.*")
        .derive_debug(true)
        .derive_default(true)
        .generate()
        .expect("Failed to generate SPDK bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Failed to write bindings.rs");

    // Tell cargo to re-run if the wrapper header changes.
    println!("cargo:rerun-if-changed=wrapper.h");
    println!(
        "cargo:rerun-if-changed={}",
        spdk_build.join("lib").display()
    );
}
