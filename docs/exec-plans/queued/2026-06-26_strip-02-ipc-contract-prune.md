# 2026-06-26 Strip 02 — Prune The IPC Contract Surface

Part of phase `2026-06-26_strip-to-tray-only`. Queued. Depends on chunk 1.

## Objective

Remove every IPC command and event that the tray-only app no longer needs,
across the Rust IPC layer, the contract registry, the generated TypeScript
contracts, and the frontend IPC client. End state: a green build whose IPC
surface is tray-only.

## Acceptance Criteria

- Deleted commands: `app_open_details`; `diagnostics_get_status`,
  `diagnostics_get_history`, `diagnostics_reveal_logs`;
  `database_get_maintenance_status`, `database_integrity_check`,
  `database_checkpoint`, `database_vacuum`, `database_restore_migration_backup`;
  `history_get_export_preview`, `history_export`, `history_get_delete_preview`,
  `history_delete`; `budgets_list/get/create/update/enable/disable/delete/get_progress`;
  `usage_get_overview`, `usage_get_calendar`, `usage_get_day_detail`,
  `usage_get_sessions`, `usage_get_session_detail`.
- Deleted event: `burnly://v1/open-details`.
- Kept commands: `__burnly_contract_probe`, `app_get_bootstrap`,
  `app_get_capabilities`, `app_hide_tray_panel`, `settings_get`,
  `settings_update`, `settings_update_project_path_retention`,
  `refresh_get_state`, `refresh_request`, `refresh_cancel`,
  `usage_get_tray_summary`.
- `ipc/mod.rs` invoke handler and `ipc/contract.rs` COMMANDS/EVENTS updated.
- IPC modules deleted: `ipc/budgets.rs`, `ipc/diagnostics.rs`,
  `ipc/database_maintenance.rs`, `ipc/export.rs`, `ipc/history_deletion.rs`;
  `ipc/usage.rs` trimmed to tray summary only.
- `src/ipc/generated/contracts` regenerated; `client.ts` and `client.test.ts`
  wrappers/cases for deleted commands removed.
- Gate passes: `cargo test`, `pnpm architecture:check`, contract harness,
  `pnpm test`.

## Risk Class

`high`

The contract registry is harness-checked and the generated TS is produced from
it. Rust and TS must change together or the build breaks. This is one actionable
unit for that reason.

## Impact Areas

- `src-tauri/src/ipc/` (mod, contract, commands, usage, deleted modules)
- `src/ipc/generated/contracts`, `src/ipc/client.ts`, `src/ipc/client.test.ts`
- Contract harness (`scripts/harness/check-contracts.mjs`) inputs/outputs

## Design Review

- Removes commands; introduces no new abstraction.
- The application services behind deleted commands become unreferenced and are
  removed in chunks 3-6; this chunk only severs the IPC boundary.
- `WindowActions::open_details` may remain temporarily unused after removing
  `app_open_details`; it is removed in chunk 7.

## Checklist

- [ ] Remove deleted commands from `ipc/mod.rs` invoke handler.
- [ ] Remove deleted entries from `ipc/contract.rs` COMMANDS and the
      `open-details` EVENT.
- [ ] Delete the dead IPC modules; trim `ipc/usage.rs` to tray summary.
- [ ] Remove `app_open_details` from `ipc/commands.rs`.
- [ ] Decide and apply the contract version (see Decisions).
- [ ] Regenerate `src/ipc/generated/contracts`.
- [ ] Remove dead wrappers from `client.ts` and cases from `client.test.ts`.
- [ ] Run the gate.

## Test Plan

- Behavior and invariants to prove: kept commands still round-trip; contract
  registry uniqueness/version tests pass; generated TS matches the registry.
- Lowest stable test layer: Rust IPC contract tests + contract harness; frontend
  `client.test.ts`.
- Failure paths: ensure removed commands are absent from generated output.
- Fixtures or fakes: existing IPC response fixtures.
- Runtime or platform evidence: not required.
- Relevant commands: `cargo test`, `pnpm architecture:check`, contract harness,
  `pnpm test`.

## Decisions

- Contract version: keep at `1` (local-only; frontend and backend ship
  together; no external consumers). Revisit only if a compatibility signal is
  needed.
- Keep `update-progress` and `platform-state-changed` events only if still
  emitted; remove if not.

## Verification

- Command: `pnpm verify`
- Outcome: not run yet

## Runtime Evidence

- Not required yet.

## Follow-Up Debt

- Unreferenced application services/stores removed in chunks 3-6.
