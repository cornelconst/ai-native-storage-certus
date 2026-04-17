use extent_manager::test_support::create_test_component;
use interfaces::{ExtentManagerError, IExtentManager};
use std::sync::Arc;

const SLAB_SIZE: u32 = 128 * 4096; // 128 blocks = 512 KiB slab
const TOTAL_SIZE: u64 = 100 * 128 * 4096; // room for ~100 slabs
const EXTENT_SIZE: u32 = 131072; // 128 KiB

fn setup() -> (
    Arc<extent_manager::ExtentManagerComponentV1>,
    Arc<extent_manager::test_support::MockBlockDevice>,
) {
    let (comp, mock) = create_test_component();
    comp.initialize(TOTAL_SIZE, SLAB_SIZE).expect("initialize");
    (comp, mock)
}

#[test]
fn initialize_creates_state() {
    let (comp, _mock) = create_test_component();
    comp.initialize(TOTAL_SIZE, SLAB_SIZE).expect("initialize");
}

#[test]
fn initialize_rejects_zero_size() {
    let (comp, _mock) = create_test_component();
    let err = comp.initialize(0, SLAB_SIZE);
    assert!(err.is_err());
}

#[test]
fn initialize_rejects_tiny_slab() {
    let (comp, _mock) = create_test_component();
    let err = comp.initialize(TOTAL_SIZE, 4096); // 1 block slab
    assert!(err.is_err());
}

#[test]
fn initialize_rejects_unaligned_slab() {
    let (comp, _mock) = create_test_component();
    let err = comp.initialize(TOTAL_SIZE, 5000);
    assert!(err.is_err());
}

#[test]
fn create_extent_basic() {
    let (comp, _mock) = setup();
    let result = comp.create_extent(1, EXTENT_SIZE, "test.dat", 0);
    assert!(result.is_ok());
    comp.lookup_extent(1).expect("extent should exist");
}

#[test]
fn create_extent_with_filename_and_crc() {
    let (comp, _mock) = setup();
    let result = comp.create_extent(42, EXTENT_SIZE, "myfile.dat", 0xDEADBEEF);
    assert!(result.is_ok());

    let extent = comp.lookup_extent(42).expect("lookup");
    assert_eq!(extent.filename, "myfile.dat");
    assert_eq!(extent.crc, 0xDEADBEEF);
}

#[test]
fn create_extent_duplicate_key_fails() {
    let (comp, _mock) = setup();
    comp.create_extent(1, EXTENT_SIZE, "", 0)
        .expect("first create");
    let err = comp.create_extent(1, EXTENT_SIZE, "", 0);
    assert!(matches!(err, Err(ExtentManagerError::DuplicateKey(_))));
}

#[test]
fn lookup_extent_basic() {
    let (comp, _mock) = setup();
    let created = comp
        .create_extent(1, EXTENT_SIZE, "hello.txt", 0xABCD)
        .expect("create");
    let looked_up = comp.lookup_extent(1).expect("lookup");
    assert_eq!(created, looked_up);
}

#[test]
fn lookup_extent_not_found() {
    let (comp, _mock) = setup();
    let err = comp.lookup_extent(999);
    assert!(matches!(err, Err(ExtentManagerError::KeyNotFound(_))));
}

#[test]
fn remove_extent_basic() {
    let (comp, _mock) = setup();
    comp.create_extent(1, EXTENT_SIZE, "", 0).expect("create");
    comp.remove_extent(1).expect("remove");

    let err = comp.lookup_extent(1);
    assert!(matches!(err, Err(ExtentManagerError::KeyNotFound(_))));
}

#[test]
fn remove_extent_not_found() {
    let (comp, _mock) = setup();
    let err = comp.remove_extent(999);
    assert!(matches!(err, Err(ExtentManagerError::KeyNotFound(_))));
}

#[test]
fn remove_then_create_reuses_slot() {
    let (comp, _mock) = setup();
    comp.create_extent(1, EXTENT_SIZE, "", 0).expect("create 1");
    comp.remove_extent(1).expect("remove 1");
    comp.create_extent(2, EXTENT_SIZE, "", 0).expect("create 2");
    comp.lookup_extent(2).expect("new extent should exist");
}

#[test]
fn multiple_extent_sizes() {
    let (comp, _mock) = setup();
    comp.create_extent(1, 131072, "", 0).expect("create 128K");
    comp.create_extent(2, 262144, "", 0).expect("create 256K");

    comp.remove_extent(1).expect("remove 128K");
    comp.lookup_extent(2).expect("256K still exists");
}

#[test]
fn out_of_space() {
    let (comp, _mock) = create_test_component();
    let block_size = 4096u64;
    comp.initialize(3 * block_size, 2 * 4096).expect("init");

    comp.create_extent(1, EXTENT_SIZE, "", 0).expect("1");
    let err = comp.create_extent(2, EXTENT_SIZE, "", 0);
    assert!(matches!(err, Err(ExtentManagerError::OutOfSpace)));
}

#[test]
fn not_initialized_errors() {
    let (comp, _mock) = create_test_component();
    let err = comp.create_extent(1, EXTENT_SIZE, "", 0);
    assert!(matches!(err, Err(ExtentManagerError::NotInitialized(_))));

    let err = comp.lookup_extent(1);
    assert!(matches!(err, Err(ExtentManagerError::KeyNotFound(_))));

    let err = comp.remove_extent(1);
    assert!(matches!(err, Err(ExtentManagerError::KeyNotFound(_))));
}

#[test]
fn dynamic_slab_allocation() {
    let (comp, _mock) = setup();
    comp.create_extent(1, 131072, "", 0).expect("128K class");
    comp.create_extent(2, 262144, "", 0).expect("256K class");
    comp.create_extent(3, 524288, "", 0).expect("512K class");
}

#[test]
fn multi_slab_same_class() {
    let (comp, _mock) = create_test_component();
    comp.initialize(100 * 4096, 4 * 4096).expect("init");

    // Fill first slab (3 slots)
    comp.create_extent(1, EXTENT_SIZE, "", 0).expect("1");
    comp.create_extent(2, EXTENT_SIZE, "", 0).expect("2");
    comp.create_extent(3, EXTENT_SIZE, "", 0).expect("3");

    // Triggers second slab allocation for the same class
    comp.create_extent(4, EXTENT_SIZE, "", 0).expect("4");
}

#[test]
fn get_extents_empty() {
    let (comp, _mock) = setup();
    assert!(comp.get_extents().is_empty());
}

#[test]
fn get_extents_returns_all() {
    let (comp, _mock) = setup();
    for i in 1..=5u64 {
        comp.create_extent(i, EXTENT_SIZE, "", 0).expect("create");
    }

    let extents = comp.get_extents();
    assert_eq!(extents.len(), 5);

    let mut keys: Vec<u64> = extents.iter().map(|e| e.key).collect();
    keys.sort();
    assert_eq!(keys, vec![1, 2, 3, 4, 5]);
}

#[test]
fn get_extents_reflects_removals() {
    let (comp, _mock) = setup();
    for i in 1..=3u64 {
        comp.create_extent(i, EXTENT_SIZE, "", 0).expect("create");
    }
    comp.remove_extent(2).expect("remove");

    let extents = comp.get_extents();
    assert_eq!(extents.len(), 2);

    let mut keys: Vec<u64> = extents.iter().map(|e| e.key).collect();
    keys.sort();
    assert_eq!(keys, vec![1, 3]);
}
