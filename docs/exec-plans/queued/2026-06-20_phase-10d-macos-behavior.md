# 2026-06-20 Phase 10D-macOS macOS Platform Behavior

## Objective

Validate and correct Burnly's native macOS behavior after Linux Phase 10D is
complete.

## Acceptance Criteria

- macOS Apple Silicon and Intel DMG artifacts install and launch.
- Menu-bar/tray, close/reopen, notifications, log reveal, export dialogs,
  recovery, and packaged sidecar behavior have recorded supported/unavailable
  outcomes.
- Quarantine, signing/notarization caveats, and permission prompts are recorded
  explicitly until Phase 10F defines signing and update policy.
- macOS-specific behavior remains isolated behind platform adapters and release
  tooling.

## Risk Class

`high`

## Impact Areas

- macOS DMG/app bundle behavior
- Menu-bar lifecycle
- Native notifications and permissions
- File dialogs and opener
- Packaged sidecar execution
- Signing/notarization caveats

## Design Review

- Complexity introduced: app bundle layout, quarantine, unsigned app behavior,
  notification permission, reopen semantics, and menu-bar integration.
- Owning modules: `platform/` adapters and release smoke harnesses own native
  differences.
- Avoid moving macOS branches into domain or application services.

## Checklist

- [ ] Validate macOS Apple Silicon installed smoke behavior.
- [ ] Validate macOS Intel installed smoke behavior.
- [ ] Record explicit unsupported/unavailable outcomes.
- [ ] Fix adapter or packaging issues exposed by evidence.

## Test Plan

- Lowest stable test layer: adapter tests followed by installed smoke tests.
- Runtime evidence: installed DMG artifacts from a successful release workflow.
- Relevant commands: platform smoke scripts, `pnpm verify:runtime`, release
  workflow evidence.

## Decisions

- Start only after Linux Phase 10D completes.

## Verification

- Command: not run yet
- Outcome: pending

## Runtime Evidence

- Required on macOS Apple Silicon and Intel.

## Follow-Up Debt

- None.
