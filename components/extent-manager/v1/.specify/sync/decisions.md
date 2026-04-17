# Drift Resolution Decisions

Decided: 2026-04-16 (interactive session)
Based on: proposals from 2026-04-16

## Decision Summary

| # | Spec | Requirement | Direction | Decision |
|---|------|-------------|-----------|----------|
| 1 | 001 | FR-007 | BACKFILL | APPROVED — Update to dynamic slab model |
| 2 | 001 | SC-005 | BACKFILL | APPROVED — Update to 256-slab capacity model |
| 3 | 001 | FR-014 | CUSTOM | Unify ILogger: remove example_logger's duplicate, use interfaces::ILogger everywhere |
| 4 | 001 | FR-006 | ALIGN | APPROVED — Implement iterate_extents on IExtentManager |
| 5 | 001 | FR-015 | BACKFILL | APPROVED — Update to mention open() slab-scanning |
| 6 | 002 | FR-009 | BACKFILL | APPROVED — Bind logger to all components once ILogger is unified |
| 7 | 002 | SC-003 | BACKFILL | APPROVED — Revise for sub-microsecond lookup reality |
| 8 | 001 | Entities | BACKFILL | APPROVED — Full entity/edge-case/assumption rewrite for slab model |

## Action Items

### Spec Updates (Backfill)

1. Update `specs/001-extent-management/spec.md`:
   - FR-007: Rewrite for dynamic slab allocation
   - SC-005: Rewrite for 256-slab capacity
   - FR-015: Add open() slab-scanning note
   - US7: Rename and rewrite acceptance scenarios
   - Key Entities: Redefine Size Class, add Slab, update Slot/Superblock
   - Edge Cases: Replace static-model cases with slab-model cases
   - Assumptions: Update immutability assumption

2. Update `specs/002-extent-benchmark/spec.md`:
   - FR-009: Logger bound to all components (after ILogger unification)
   - SC-003: Revise multi-thread scaling criterion

### Code Changes (Align)

3. **Unify ILogger** (Proposal 3 — custom):
   - Remove `define_interface! { pub ILogger { ... } }` from `example_logger`
   - Make `LoggerComponent` implement `interfaces::ILogger`
   - Add `interfaces` dependency to `example_logger` Cargo.toml
   - Update all consumers (block-device-spdk-nvme, spdk-env) to use `interfaces::ILogger`
   - Bind logger to extent manager in benchmark app

4. **Implement iterate_extents** (Proposal 4):
   - Add `fn iterate_extents(&self, callback: &dyn Fn(u64, &[u8]) -> bool) -> Result<u64, ExtentManagerError>` to IExtentManager
   - Implement in lib.rs with write lock (exclusive access per spec)
   - Add unit tests and integration tests
   - Add doc tests
   - Update iterate_benchmark.rs to use public API
