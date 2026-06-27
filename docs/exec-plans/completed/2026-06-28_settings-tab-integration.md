# 2026-06-28 Settings Tab Integration

## Objective

Wire the tray settings tab to the existing settings IPC logic.

## Acceptance Criteria

- Settings tab loads persisted settings through `src/features/settings`.
- Close behavior is editable from the tray settings tab.
- Updates preserve non-rendered settings fields and use revision conflict
  protection.
- Loading, error, and saving states are visible and accessible.
- Launch-at-login is not presented as an active control while runtime support is
  unavailable.

## Risk Class

`low`

This is a frontend integration over an existing settings contract and service.

## Impact Areas

- `src/features/tray/TrayPanel.tsx`
- `src/features/tray/TrayPanel.test.tsx`
- `src/features/settings/use-settings.ts`

## Design Review

- Keep React feature code behind the existing IPC hook boundary.
- Do not expose hidden persistence fields as product settings.
- Avoid local duplicated settings state; use the query result and mutation
  status as the source of truth.
- Preserve `launchAtLogin` on updates because the backend command requires the
  complete settings document.

## Checklist

- [x] Move stale visual redesign plan out of `active/`.
- [x] Render close behavior options in the settings tab.
- [x] Connect option changes to `useUpdateSettings`.
- [x] Add focused tray settings tests.
- [x] Run frontend checks and relevant gates.
- [x] Record verification outcomes.

## Test Plan

- Behavior and invariants to prove: current close behavior renders; selecting a
  different option calls update with current `launchAtLogin`, new
  `closeBehavior`, and current revision; pending and error states render.
- Lowest stable test layer: React tray panel tests with IPC client fakes.
- Failure paths: settings load failure and update failure.
- Fixtures or fakes: mocked IPC client and event subscription.
- Runtime or platform evidence: not required; no native runtime behavior change.
- Relevant commands: `pnpm test -- src/features/tray/TrayPanel.test.tsx`,
  `pnpm lint`, `pnpm verify:fast`.

## Decisions

- Hide launch-at-login until runtime support exists instead of showing a
  disabled setting that cannot be changed.

## Verification

- Command: `pnpm test -- src/features/tray/TrayPanel.test.tsx`
- Outcome: passed; Vitest reported 16 files and 75 tests passed.
- Command: `pnpm lint`
- Outcome: passed with 15 existing warnings.
- Command: `pnpm verify:fast`
- Outcome: initially failed on Prettier formatting for edited TSX files; passed
  after formatting.
- Command: `pnpm verify`
- Outcome: passed; includes format, lint, typecheck, Vitest, sidecar prepare,
  Rust format, Clippy, Rust tests, and harness checks.

## Runtime Evidence

- Not required.

## Follow-Up Debt

- Add launch-at-login UI when runtime support is implemented.
