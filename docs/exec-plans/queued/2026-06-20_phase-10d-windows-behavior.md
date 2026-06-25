# 2026-06-20 Phase 10D-Windows Windows Platform Behavior

## Objective

Validate and correct Burnly's native Windows behavior after Linux Phase 10D is
complete.

## Acceptance Criteria

- Windows x64 and ARM64 NSIS artifacts install, launch, reinstall, and uninstall
  with explicit data-retention outcomes.
- Windows paths with spaces and non-ASCII characters are handled correctly.
- Tray, close/reopen, notifications, log reveal, export dialogs, recovery, and
  packaged sidecar behavior have recorded supported/unavailable outcomes.
- Windows-specific behavior remains isolated behind platform adapters and
  release tooling.

## Risk Class

`high`

## Impact Areas

- Windows installer behavior
- Path and process handling
- Tray/lifecycle integration
- Native notifications
- File dialogs and opener
- Packaged sidecar execution

## Design Review

- Complexity introduced: Windows shell, NSIS, profile paths, process creation,
  notification permission variance, and tray behavior.
- Owning modules: `platform/` adapters and release smoke harnesses own native
  differences.
- Avoid moving Windows branches into domain or application services.

## Checklist

- [ ] Validate Windows x64 installed smoke behavior.
- [ ] Validate Windows ARM64 installed smoke behavior.
- [ ] Record explicit unsupported/unavailable outcomes.
- [ ] Fix adapter or packaging issues exposed by evidence.

## Test Plan

- Lowest stable test layer: adapter tests followed by installed smoke tests.
- Runtime evidence: installed NSIS artifacts from a successful release workflow.
- Relevant commands: platform smoke scripts, `pnpm verify:windows`, release
  workflow evidence.

## Decisions

- Start only after Linux Phase 10D completes.

## Verification

- Command: not run yet
- Outcome: pending

## Runtime Evidence

- Required on Windows x64 and ARM64.

## Follow-Up Debt

- None.
