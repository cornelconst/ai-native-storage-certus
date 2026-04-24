# Drift Resolution Proposals

Generated: 2026-04-23 (updated from 2026-04-16)
Based on: drift-report from 2026-04-23 (v2)

## Summary

| Resolution Type | Count | Applied |
|-----------------|-------|---------|
| Backfill (Code -> Spec) | 3 | 2 applied (P2, P3, P4); P1 deferred |
| Align (Spec -> Code) | 0 | (FR-009, WriteAsync fixed in v2) |
| Cleanup | 1 | 1 applied (stale comment) |
| Implement | 3 | 3 applied (placeholder tests) |

---

## Proposal 1: 001/FR-003 — Sync timeout parameter

**Direction**: BACKFILL
**Status**: DEFERRED (user chose not to apply)

Spec says sync ops have "timeout" but code has none. Sync ops poll SPDK in a tight loop; timeout only on async variants. The spec could be updated to clarify this, but the user deferred the change.

---

## Proposal 2: 001/SC-008 — Benchmark coverage

**Direction**: BACKFILL
**Status**: APPLIED (2026-04-23)

Updated SC-008 wording to allow unit-level benchmarks for internal algorithms (qpair selection is `pub(crate)`).

---

## Proposal 3: 001/NEW — Add IBlockDeviceAdmin to spec (FR-021)

**Direction**: BACKFILL
**Status**: APPLIED (2026-04-23)

Added FR-021 specifying the `IBlockDeviceAdmin` interface: `set_pci_address`, `set_actor_cpu`, `initialize`. Updated FR-012 to reference FR-021.

---

## Proposal 4: 002/NEW — Add `--io-mode` to spec (FR-022)

**Direction**: BACKFILL
**Status**: APPLIED (2026-04-23)

Added FR-022 covering the `--io-mode sync|async` flag to spec 002.

---

## Proposal 5: Cleanup stale WriteAsync bug comment

**Direction**: CLEANUP
**Status**: APPLIED (2026-04-23)

Removed the stale comment at `tests/integration.rs:634-638` documenting a WriteAsync use-after-free bug that was fixed in v2.

---

## Proposal 6: SC-001/SC-002/SC-006 placeholder tests

**Direction**: IMPLEMENT
**Status**: APPLIED (2026-04-23)

Added three `#[ignore]` hardware-dependent integration tests:
- `sc001_sync_latency_envelope` — 100 sync round-trips, assert p50 < 100us
- `sc002_timeout_accuracy` — async read with 50ms timeout, validate timing
- `sc006_telemetry_accuracy` — N sync writes, compare telemetry vs independent measurement (requires `--features telemetry`)

---

## Previously resolved (v1 -> v2)

These items from the original 2026-04-16 proposals were already fixed in v2:

- **Proposal 2 (original): FR-009 controller reset scope** — `handle_controller_reset()` now iterates all clients
- **Proposal 9 (original): WriteAsync buffer lifetime** — `PendingOp.write_buf` now pins `Arc<DmaBuffer>`
