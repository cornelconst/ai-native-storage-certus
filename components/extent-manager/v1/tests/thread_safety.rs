use extent_manager::test_support::create_test_component;
use extent_manager::ExtentManagerComponentV1;
use interfaces::IExtentManager;
use std::sync::Arc;
use std::thread;

const SLAB_SIZE: u32 = 128 * 4096;
const TOTAL_SIZE: u64 = 100 * 128 * 4096;
const EXTENT_SIZE: u32 = 131072;

#[test]
fn concurrent_creates_no_duplicates() {
    let (comp, _mock) = create_test_component();
    comp.initialize(TOTAL_SIZE, SLAB_SIZE).expect("init");

    let handles: Vec<_> = (0..4)
        .map(|t| {
            let comp: Arc<ExtentManagerComponentV1> = Arc::clone(&comp);
            thread::spawn(move || {
                for i in 0..100u64 {
                    let key = t * 1000 + i;
                    comp.create_extent(key, EXTENT_SIZE, "", 0).expect("create");
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    for t in 0..4u64 {
        for i in 0..100u64 {
            comp.lookup_extent(t * 1000 + i).expect("lookup");
        }
    }
}

#[test]
fn concurrent_lookups() {
    let (comp, _mock) = create_test_component();
    comp.initialize(TOTAL_SIZE, SLAB_SIZE).expect("init");

    for i in 0..50u64 {
        comp.create_extent(i, EXTENT_SIZE, "", 0).expect("create");
    }

    let handles: Vec<_> = (0..4)
        .map(|_| {
            let comp: Arc<ExtentManagerComponentV1> = Arc::clone(&comp);
            thread::spawn(move || {
                for i in 0..50u64 {
                    comp.lookup_extent(i).expect("lookup");
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }
}

#[test]
fn concurrent_create_and_lookup() {
    let (comp, _mock) = create_test_component();
    comp.initialize(TOTAL_SIZE, SLAB_SIZE).expect("init");

    for i in 0..100u64 {
        comp.create_extent(i, EXTENT_SIZE, "", 0).expect("create");
    }

    let comp_writer: Arc<ExtentManagerComponentV1> = Arc::clone(&comp);
    let writer = thread::spawn(move || {
        for i in 100..200u64 {
            comp_writer
                .create_extent(i, EXTENT_SIZE, "", 0)
                .expect("create");
        }
    });

    let comp_reader: Arc<ExtentManagerComponentV1> = Arc::clone(&comp);
    let reader = thread::spawn(move || {
        for i in 0..100u64 {
            comp_reader.lookup_extent(i).expect("lookup");
        }
    });

    writer.join().unwrap();
    reader.join().unwrap();
    for i in 100..200u64 {
        comp.lookup_extent(i).expect("lookup created extent");
    }
}
