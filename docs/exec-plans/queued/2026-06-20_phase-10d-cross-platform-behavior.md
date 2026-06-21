# 2026-06-20 Phase 10D Cross-Platform Behavior

## Objective

Validate and correct Burnly's native behavior on Windows, macOS, GNOME, and KDE
without leaking platform rules into domain or application code.

## Acceptance Criteria

- Windows paths, process creation, dialogs, tray, and lifecycle behavior pass.
- macOS bundle paths, permissions, tray/menu-bar, reopen, and quarantine-related
  behavior pass.
- Linux tray and lifecycle behavior is validated on at least GNOME and KDE.
- Export, log reveal, notifications, recovery, and sidecar behavior have
  explicit supported/unavailable outcomes per platform.
- Platform differences remain isolated behind existing adapters.

## Risk Class

`high`

## Impact Areas

- Platform adapters
- Lifecycle and tray
- Sidecar process behavior
- File dialogs and opener
- Notifications and recovery
- Platform smoke harness

## Design Review

- Complexity introduced: real operating-system differences.
- Owning modules: `platform/` and infrastructure adapters own native variation.
- Interface depth: application services receive stable capability/outcome types.
- Special cases: Windows quoting, macOS reopen, Linux tray hosts, Wayland/X11,
  permissions, and headless CI.
- Add abstractions only when they remove repeated platform branches.
- Existing capability reporting must represent unavailable behavior truthfully.

## Checklist

- [ ] Define supported platform/environment matrix.
- [ ] Validate Windows path, process, dialog, tray, and lifecycle behavior.
- [ ] Validate macOS bundle, permission, tray, reopen, and process behavior.
- [ ] Validate Linux GNOME and KDE tray/lifecycle behavior.
- [ ] Correct platform adapters and capability reporting.
- [ ] Record unsupported environments and exact limitations.

## Test Plan

- Behavior and invariants to prove: supported native workflows behave
  consistently; unsupported workflows fail safely.
- Lowest stable test layer: adapter tests followed by installed smoke tests.
- Failure paths: missing tray host, denied notification, malformed path,
  unavailable opener/dialog, sidecar spawn denial, and resume/reopen variance.
- Fixtures or fakes: paths with spaces/non-ASCII characters and platform process
  fixtures.
- Runtime or platform evidence: mandatory on Windows, macOS, GNOME, and KDE.
- Relevant commands: `pnpm verify:runtime`, packaged smoke scripts.

## Decisions

- Do not claim Linux tray support from a single desktop environment.

## Verification

- Command: `pnpm verify`
- Outcome: not run yet

## Runtime Evidence

- Required across the declared matrix.

## Follow-Up Debt

- None.
