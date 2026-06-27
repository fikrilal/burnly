# 2026-06-26 Strip 04 — Remove Diagnostics And Database Recovery

Part of phase `2026-06-26_strip-to-tray-only`. Active. Coordinates with strip-to-tray-only.

## Objective

Delete the diagnostics service and the database maintenance/recovery subsystem,
including the startup recovery guard. On startup failure the tray shows an error
state with no in-app repair path.

## Acceptance Criteria

- Deleted: `application/diagnostics.rs`, `application/database_maintenance.rs`.
- Deleted stores: `infrastructure/database/diagnostics_store.rs`,
  `maintenance_store.rs`.
- Deleted ports: `application/ports/diagnostics_store.rs`,
  `database_maintenance.rs`, `log_reveal.rs`.
- Deleted platform: `platform/logs.rs` (log reveal).
- `bootstrap.rs` recovery guard (`RecoveryMaintenanceGuard` /
  `RuntimeMaintenanceGuard`) and recovery startup path removed; startup failure
  surfaces a plain error.
- Diagnostics capability removed from the bootstrap/capabilities surface.
- Gate passes: `cargo test`, `pnpm architecture:check`.

## Risk Class

`high`

Touches the startup/recovery path in `bootstrap.rs`. Must preserve normal
startup and a clean error on failure.

## Impact Areas

- `src-tauri/src/application/` (diagnostics, database_maintenance + `mod.rs`)
- `src-tauri/src/infrastructure/database/` (diagnostics/maintenance stores)
- `src-tauri/src/application/ports/`
- `src-tauri/src/platform/` (`logs.rs`, `mod.rs`)
- `src-tauri/src/bootstrap.rs` (recovery guard + startup path + tests)

## Design Review

- Removal simplifies startup: the recovery branch and its guard are deleted, not
  replaced.
- Confirm the startup-failure path still produces a stable, user-safe error for
  the tray (no recovery affordance).
- Confirm no kept code references the log-reveal capability.

## Checklist

- [x] Delete diagnostics + database_maintenance application modules.
- [x] Delete diagnostics/maintenance stores and ports.
- [x] Delete `platform/logs.rs`; update `platform/mod.rs`.
- [x] Remove the recovery guard and recovery startup branch from `bootstrap.rs`.
- [x] Remove diagnostics capability from the capabilities surface.
- [x] Update bootstrap tests to the no-recovery behavior.
- [x] Run the gate.

## Test Plan

- Behavior and invariants to prove: fresh startup migrates/seeds and runs;
  startup failure yields a stable error with no recovery path.
- Lowest stable test layer: bootstrap startup tests.
- Failure paths: unsupported newer schema / open failure still fail with a
  stable category (no recovery offered).
- Fixtures or fakes: existing bootstrap test harness.
- Runtime or platform evidence: not required.
- Relevant commands: `cargo test`, `pnpm architecture:check`.

## Decisions

- Database recovery is dropped entirely (per `tray-only-decision.md`).

## Verification

- Command: `cargo test`
- Outcome: passed cleanly (230 tests passed).
- Command: `pnpm verify:fast`
- Outcome: passed cleanly.

## Runtime Evidence

- Not required yet.

## Follow-Up Debt

- A corrupt local database is resolved by resetting local data, not in-app.
