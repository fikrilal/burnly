# 2026-06-26 Strip 05 — Remove Export And History

Part of phase `2026-06-26_strip-to-tray-only`. Queued. Depends on chunk 2.

## Objective

Delete the CSV export subsystem, the import/refresh history listing, and history
deletion now that no IPC command references them.

## Acceptance Criteria

- Deleted: `application/export.rs`, `application/history.rs`,
  `application/history_deletion.rs`.
- Deleted stores: `infrastructure/database/export_store.rs`,
  `history_store.rs`, `history_deletion_store.rs`.
- Deleted ports: `application/ports/export_store.rs`, `export_writer.rs`,
  `history_store.rs`, `history_deletion_store.rs`.
- Deleted platform: `platform/export.rs` (CSV writer).
- `bootstrap.rs` no longer wires export/history services.
- Gate passes: `cargo test`, `pnpm architecture:check`.

## Risk Class

`medium`

Isolated removal; coupling is `bootstrap.rs` wiring and module declarations. Run
collector/migration checks since the run-history schema is shared with
reconciliation.

## Impact Areas

- `src-tauri/src/application/` (export, history, history_deletion + `mod.rs`)
- `src-tauri/src/infrastructure/database/` (export/history stores)
- `src-tauri/src/application/ports/`
- `src-tauri/src/platform/` (`export.rs`, `mod.rs`)
- `src-tauri/src/bootstrap.rs`

## Design Review

- Pure removal; no new abstraction.
- The import/refresh run records remain written by reconciliation; only the
  history _read_ and _delete_ paths are removed.
- Confirm reconciliation does not depend on the history read store.

## Checklist

- [ ] Delete export/history/history_deletion application modules.
- [ ] Delete the corresponding stores and ports.
- [ ] Delete `platform/export.rs`; update `platform/mod.rs`.
- [ ] Remove export/history wiring from `bootstrap.rs`.
- [ ] Run the gate.

## Test Plan

- Behavior and invariants to prove: reconciliation still writes run records;
  tracker spine unaffected.
- Lowest stable test layer: reconciliation store tests, bootstrap tests.
- Failure paths: startup succeeds without export/history services.
- Fixtures or fakes: existing reconciliation tests.
- Runtime or platform evidence: not required.
- Relevant commands: `cargo test`, `pnpm architecture:check`.

## Decisions

- Export and history are removed from local; not deferred to web in any local
  form.

## Verification

- Command: `pnpm verify`
- Outcome: not run yet

## Runtime Evidence

- Not required yet.

## Follow-Up Debt

- None.
