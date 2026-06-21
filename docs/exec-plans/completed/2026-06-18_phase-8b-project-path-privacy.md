# 2026-06-18 Phase 8B Project-Path Privacy

## Objective

Make project-path retention an enforceable backend privacy policy and
atomically remove retained raw paths when the user disables it.

## Acceptance Criteria

- Disabling path retention commits the setting change and raw-path deletion in
  one transaction.
- Existing `projects.raw_path` values and approved path-bearing diagnostic
  artifacts are cleared.
- Future collection does not persist raw paths while retention is disabled.
- Re-enabling retention affects future imports only and does not reconstruct
  deleted paths.
- The UI clearly confirms the destructive effect before submission.

## Risk Class

`high`

This is a destructive, privacy-sensitive data operation.

## Impact Areas

- Settings application use case and store transaction
- Project reconciliation mapping and persistence
- Diagnostics redaction/retention boundary
- Settings IPC and confirmation UI
- Privacy-focused repository and runtime tests

## Design Review

- What complexity is being introduced? One explicit privacy transition spanning
  settings, project storage, and diagnostic cleanup.
- Which decisions are hidden inside the owning module? The privacy use case owns
  what data is path-bearing and how deletion is transacted.
- Is each new interface simpler than its implementation? The caller requests a
  retention policy change and receives a typed outcome.
- What special cases exist, and can the design eliminate them? Re-enable does
  not restore data; concurrent imports must observe one committed policy.
- Why is each new abstraction needed now? A generic settings update cannot
  safely express destructive cleanup.
- Can an existing module absorb this responsibility cleanly? A settings-owned
  privacy transition can coordinate existing stores without a generic privacy
  framework.

## Checklist

- [x] Inventory every persisted and diagnostic location that can contain paths.
- [x] Define the explicit disable-retention use case and result.
- [x] Implement atomic setting update and stored-path cleanup.
- [x] Enforce retention policy during future project reconciliation.
- [x] Add confirmation and outcome states to settings UI.
- [x] Add real SQLite tests for deletion, rollback, concurrency, and re-enable.
- [x] Add redaction tests for diagnostic artifacts in current scope.
- [x] Record privacy behavior in runtime evidence.

## Test Plan

- Behavior and invariants to prove: no raw path remains after commit; rollback
  preserves prior state; disabled policy prevents future persistence.
- Lowest stable test layer: real SQLite transaction and reconciliation tests.
- Failure paths: database failure during cleanup, stale settings revision, and
  import racing with policy change.
- Fixtures or fakes: path-bearing project fixtures; real SQLite.
- Runtime or platform evidence: settings confirmation and restart persistence.
- Relevant commands: focused Rust/UI tests, `pnpm verify`, `pnpm verify:runtime`.

## Decisions

- This operation is explicit and destructive; it is not a boolean-only update.
- Stable non-reversible grouping identifiers may remain only as allowed by the
  locked database design.
- Current persisted path-bearing storage is `projects.raw_path` plus legacy
  path-valued `projects.identity_key` rows. The current schema does not persist
  raw collector payload diagnostic artifacts, so there is no diagnostic artifact
  deletion target in this chunk.
- Startup enforces the current project-path policy so existing local databases
  are normalized even before the next user privacy transition.

## Verification

- Command: `cargo test -q settings_store::tests:: --manifest-path src-tauri/Cargo.toml`
- Outcome: passed on 2026-06-18. Covered settings-store replacement, path
  deletion, stale-revision rollback, re-enable behavior, and startup policy
  normalization.
- Command: `cargo test -q reconciliation_store::tests:: --manifest-path src-tauri/Cargo.toml`
- Outcome: passed on 2026-06-18. Covered disabled and enabled project-path
  retention during future session reconciliation.
- Command: `pnpm vitest run src/features/settings/SettingsView.test.tsx src/ipc/client.test.ts`
- Outcome: passed on 2026-06-18. Covered frontend confirmation flow, retention
  IPC request wrapping, and response validation.
- Command: `pnpm architecture:check`
- Outcome: passed on 2026-06-18.
- Command: `pnpm verify`
- Outcome: passed on 2026-06-18. This included Prettier, ESLint, TypeScript,
  Vitest, rustfmt, Clippy with warnings denied, 193 passing Rust tests with one
  opt-in collector smoke test ignored, architecture/public API/contract/migration
  harness checks, collector fixture checks, and duplicate-code reporting. ESLint
  reported only warning-level signals.

## Runtime Evidence

- `pnpm verify:runtime` passed on 2026-06-18.
- Environment: Ubuntu 24.04, Linux 6.17.0-35-generic, GNOME on X11.
- Evidence covered Tauri prerequisite reporting, generated contract drift,
  production frontend build, five Tauri IPC bridge tests, platform lifecycle and
  tray unit tests, refresh scheduler tests, and twelve Playwright desktop
  evidence tests.
- The Playwright evidence includes the project-path retention confirmation flow
  on desktop and compact viewports.

## Follow-Up Debt

- Raw diagnostic payload policy remains Phase 9. Phase 8B found no current
  persisted raw diagnostic payload store to clean.
