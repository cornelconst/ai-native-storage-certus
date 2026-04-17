# Alignment Tasks

Generated: 2026-04-16
Source: drift resolution decisions

## Task 1: Unify ILogger Interface

**Spec Requirement**: 001/FR-014
**Decision**: Remove duplicate ILogger from example_logger; use interfaces::ILogger everywhere
**Current Code**: `example_logger` defines its own `ILogger` via `define_interface!`. `interfaces` crate also defines `ILogger`. These produce different `TypeId`s, preventing cross-component binding.
**Required Change**: Make `example_logger::LoggerComponent` implement `interfaces::ILogger` instead of its own copy.
**Estimated Effort**: medium

### Files to Modify

1. `components/example-logger/Cargo.toml` — add `interfaces` dependency
2. `components/example-logger/src/lib.rs` — remove local `define_interface! { pub ILogger { ... } }`, import and use `interfaces::ILogger`, update `LoggerComponent` to implement `interfaces::ILogger`
3. `components/block-device-spdk-nvme/v1/src/lib.rs` — change `use example_logger::ILogger` to `use interfaces::ILogger`
4. `components/spdk-env/src/lib.rs` — if it imports `example_logger::ILogger`, change to `interfaces::ILogger`
5. `apps/extent-benchmark/src/main.rs` — re-add logger→extent_mgr binding
6. `apps/iops-benchmark/src/main.rs` — verify still compiles (may need import change)

### Acceptance Criteria

- [ ] Only one `ILogger` definition exists (in `interfaces` crate)
- [ ] `LoggerComponent` implements `interfaces::ILogger`
- [ ] All components that declare an `ILogger` receptacle can be bound to `LoggerComponent`
- [ ] `cargo build -p extent-benchmark` succeeds with logger bound to all 4 components
- [ ] `cargo build -p iops-benchmark` succeeds
- [ ] `cargo test -p example-logger` passes
- [ ] `cargo test -p extent-manager` passes

---

## Task 2: Implement iterate_extents on IExtentManager

**Spec Requirement**: 001/FR-006, FR-015, SC-004
**Decision**: Add public iteration API with exclusive lock semantics
**Current Code**: `IExtentManager` has `extent_count()` but no iteration method. `open()` rebuilds the index internally by scanning slab bitmaps.
**Required Change**: Add `iterate_extents` to `IExtentManager` trait and implement it.
**Estimated Effort**: medium

### Files to Modify

1. `components/interfaces/src/iextent_manager.rs` — add `fn iterate_extents(&self, callback: &dyn Fn(u64, &[u8]) -> bool) -> Result<u64, ExtentManagerError>` to `IExtentManager`
2. `components/extent-manager/v1/src/lib.rs` — implement `iterate_extents`: acquire write lock, iterate `state.index`, call callback with (key, meta.serialize()), return count. Stop early if callback returns false.
3. `components/extent-manager/v1/tests/api_operations.rs` — add iteration tests: empty iteration, iterate N extents, early termination via callback
4. `components/extent-manager/v1/tests/thread_safety.rs` — add test: iteration blocks concurrent create/remove
5. `components/extent-manager/v1/benches/iterate_benchmark.rs` — update to use public `iterate_extents` API

### Acceptance Criteria

- [ ] `iterate_extents` is in the `IExtentManager` trait definition
- [ ] Implementation holds write lock (exclusive access) for the duration
- [ ] Callback receives (key: u64, serialized_metadata: &[u8]) for each extent
- [ ] Callback returning false stops iteration early
- [ ] Returns total count of extents visited
- [ ] Empty iteration returns Ok(0)
- [ ] Concurrent create/remove is blocked during iteration
- [ ] `cargo test -p extent-manager` passes with new tests
- [ ] `cargo clippy -p extent-manager -- -D warnings` clean
- [ ] `cargo bench -p extent-manager --no-run` compiles
