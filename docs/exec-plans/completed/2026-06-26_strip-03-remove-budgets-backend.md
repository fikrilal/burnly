# 2026-06-26 Strip 03 — Remove Budgets Backend

Part of phase `2026-06-26_strip-to-tray-only`. Active. Coordinates with strip-to-tray-only.

## Objective

Delete all budget application, infrastructure, port, and domain code now that no
IPC command references it. Leave migration `0003` and the budgets table in
place.

## Acceptance Criteria

- Deleted: `application/budgets.rs`, `application/budget_evaluation.rs`,
  `application/budget_notifications.rs`, `application/budget_progress.rs`.
- Deleted stores: `infrastructure/database/budget_store.rs`,
  `budget_notification_store.rs`, `budget_usage_store.rs`.
- Deleted ports: `application/ports/budget_store.rs`,
  `budget_notification_store.rs`, `budget_usage_store.rs`.
- Deleted domain: `domain/budget.rs`.
- `bootstrap.rs` no longer wires budget services; budget capability removed from
  the bootstrap/capabilities surface.
- Migration `0003` and the budgets/budget-threshold/notification tables remain.
- Gate passes: `cargo test`, `pnpm architecture:check`.

## Risk Class

`medium`

Isolated feature removal; main coupling is `bootstrap.rs` wiring and module
declarations.

## Impact Areas

- `src-tauri/src/application/` (budget modules + `mod.rs`)
- `src-tauri/src/infrastructure/database/` (budget stores + `mod.rs`)
- `src-tauri/src/application/ports/` (budget ports + `mod.rs`)
- `src-tauri/src/domain/` (`budget.rs` + `mod.rs`)
- `src-tauri/src/bootstrap.rs`

## Design Review

- Pure removal; no new abstraction.
- Confirm no kept module imports budget types (e.g. bootstrap capabilities).
- The budgets table staying is intentional: dropping an applied migration is
  riskier than an unused table.

## Checklist

- [x] Delete budget application modules and update `application/mod.rs`.
- [x] Delete budget stores and update `infrastructure/database/mod.rs`.
- [x] Delete budget ports and update `application/ports/mod.rs`.
- [x] Delete `domain/budget.rs` and update `domain/mod.rs`.
- [x] Remove budget wiring/capability from `bootstrap.rs`.
- [x] Run the gate.

## Test Plan

- Behavior and invariants to prove: bootstrap still builds the tracker spine;
  migrations still apply (including `0003`).
- Lowest stable test layer: Rust unit/integration tests, migration tests.
- Failure paths: startup succeeds without budget services.
- Fixtures or fakes: existing migration tests.
- Runtime or platform evidence: not required.
- Relevant commands: `cargo test`, `pnpm architecture:check`.

## Decisions

- Keep migration `0003` and budget tables.

## Verification

- Command: `cargo test`
- Outcome: passed cleanly (237 tests passed).
- Command: `pnpm verify:fast`
- Outcome: passed cleanly.

## Runtime Evidence

- Not required yet.

## Follow-Up Debt

- Revisit the unused budgets table if a schema reset is ever done.
