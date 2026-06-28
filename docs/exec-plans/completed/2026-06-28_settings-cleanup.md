# 2026-06-28 Settings Cleanup

## Objective

Remove obsolete editable settings for reporting timezone, project-path retention,
and refresh interval while keeping automatic timezone behavior and deterministic
privacy policy enforcement.

## Acceptance Criteria

- Settings API and frontend settings client expose only active editable settings.
- Reporting timezone is resolved automatically from the OS/frontend runtime where
  usage summaries or refresh aggregation need it.
- Project paths are not user-configurable and startup clears retained raw paths.
- Refresh interval is not user-configurable; background refresh uses the backend
  default until the scheduler policy changes later.
- Contract harness, lint, and verification gates pass.

## Risk Class

`medium`

Settings, IPC contracts, database-backed privacy behavior, and refresh scheduling
are touched.

## Impact Areas

- `src-tauri/src/domain/settings.rs`
- `src-tauri/src/application/settings.rs`
- `src-tauri/src/infrastructure/settings_store.rs`
- `src-tauri/src/bootstrap.rs`
- `src-tauri/src/ipc/settings.rs`
- `src/ipc/client.ts`
- `src/ipc/generated/contracts.ts`
- `scripts/harness/check-contracts.mjs`

## Design Review

- Complexity is reduced by removing settings that no longer have product value.
- Automatic timezone resolution stays at platform/UI edges that already own local
  runtime context.
- The settings store keeps compatibility with existing schema columns but hides
  removed storage details from the domain and IPC contract.
- Project-path privacy has one policy now: raw paths are not retained.
- No new abstraction is needed; existing settings and bootstrap modules absorb
  the narrower settings surface.

## Checklist

- [x] Inspect partial prior-agent changes and identify gaps.
- [x] Remove remaining stale contract/client/test references.
- [x] Make project-path retention deterministic after removing the option.
- [x] Run formatting, lint, and verification gates.
- [x] Record command outcomes.

## Test Plan

- Behavior and invariants to prove: removed fields are absent from settings
  responses and update requests; old retained raw paths are cleared on startup.
- Lowest stable test layer: Rust settings-store tests, IPC/client tests, contract
  harness.
- Failure paths: stale settings revision still conflicts; invalid close behavior
  still validates.
- Fixtures or fakes: existing SQLite temp DBs and IPC invoker fakes.
- Runtime or platform evidence: not required for this backend/API cleanup.
- Relevant commands: `pnpm lint`, `pnpm verify:fast`, `pnpm verify`.

## Decisions

- Keep legacy database columns for migration compatibility, but do not expose
  them through domain settings, bootstrap settings, IPC, or frontend client
  settings.
- Always enforce project-path non-retention on startup.
- Use a fixed 15-minute background scheduler interval until backend automatic
  refresh policy replaces this path.

## Verification

- Command: `pnpm lint`
- Outcome: passed with 15 pre-existing warnings.
- Command: `pnpm verify:fast`
- Outcome: passed.
- Command: `pnpm verify`
- Outcome: passed.
- Command: `pnpm verify:runtime`
- Outcome: passed; internally ran `pnpm evidence:desktop`.
- Command: `pnpm evidence:desktop`
- Outcome: covered by `pnpm verify:runtime`; desktop runtime evidence passed.

## Runtime Evidence

- Covered by `pnpm verify:runtime`; desktop runtime evidence passed.

## Follow-Up Debt

- Consider a future migration that drops obsolete `app_settings` columns after
  the storage compatibility window is intentionally closed.
