# 2026-06-28 Remove Background Refresh Setting

## Objective

Remove `backgroundRefreshEnabled` from editable settings and make refresh
scheduling automatic in the backend.

## Acceptance Criteria

- Settings domain, IPC, frontend client, generated contracts, and harness no
  longer expose `backgroundRefreshEnabled`.
- Backend refresh scheduling no longer reads or applies settings state.
- The legacy SQLite column remains for compatibility but is hidden from
  application settings.
- Relevant verification gates pass.

## Risk Class

`medium`

Settings contracts and background refresh scheduling are touched.

## Impact Areas

- `src-tauri/src/domain/settings.rs`
- `src-tauri/src/application/settings.rs`
- `src-tauri/src/bootstrap.rs`
- `src-tauri/src/infrastructure/*settings*`
- `src-tauri/src/ipc/settings.rs`
- `src/ipc/client.ts`
- `src/ipc/generated/contracts.ts`
- `scripts/harness/check-contracts.mjs`

## Design Review

- Complexity is reduced by removing a setting whose target behavior is no longer
  user-configurable.
- Automatic refresh policy is owned by the backend composition root.
- No new abstraction is introduced; existing settings interfaces become smaller.

## Checklist

- [x] Remove `backgroundRefreshEnabled` from settings domain and storage DTOs.
- [x] Make scheduler policy automatic and stop applying settings-driven policy.
- [x] Update IPC/frontend contracts and tests.
- [x] Run verification and record outcomes.

## Test Plan

- Behavior and invariants to prove: settings read/update surfaces only active
  editable settings; refresh scheduler still starts with automatic policy.
- Lowest stable test layer: Rust settings/bootstrap tests, frontend IPC client
  tests, contract harness.
- Failure paths: stale revision and invalid close behavior remain covered.
- Relevant commands: `pnpm lint`, `pnpm verify:fast`, `pnpm verify`.

## Decisions

- Keep the `app_settings.background_refresh_enabled` column for migration
  compatibility.
- Use the current fixed backend refresh interval until the later automatic
  refresh policy replaces it.

## Verification

- Command: `pnpm lint`
- Outcome: passed with 15 existing warnings.
- Command: `pnpm verify:fast`
- Outcome: passed.
- Command: `pnpm verify`
- Outcome: passed.
- Command: `pnpm verify:runtime`
- Outcome: passed; internally ran `pnpm evidence:desktop`.

## Runtime Evidence

- Covered by `pnpm verify:runtime`; desktop runtime evidence passed.

## Follow-Up Debt

- Consider dropping legacy settings columns in a future schema cleanup after the
  compatibility window is intentionally closed.
