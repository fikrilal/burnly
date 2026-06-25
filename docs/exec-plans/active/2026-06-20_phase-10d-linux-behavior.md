# 2026-06-20 Phase 10D-Linux Linux Platform Behavior

## Objective

Validate and correct Burnly's native Linux behavior on GNOME and KDE before
moving to Windows and macOS platform behavior.

## Acceptance Criteria

- Linux tray and lifecycle behavior is validated on at least GNOME and KDE.
- Export, log reveal, notifications, recovery, and sidecar behavior have
  explicit supported/unavailable outcomes for Linux.
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
- Special cases: Linux tray hosts, GNOME/KDE differences, Wayland/X11,
  notification permissions, file-dialog portals, and headless CI.
- Add abstractions only when they remove repeated platform branches.
- Existing capability reporting must represent unavailable behavior truthfully.

## Checklist

- [x] Define supported platform/environment matrix.
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
- Runtime or platform evidence: mandatory on Linux GNOME and KDE.
- Relevant commands: `pnpm verify:runtime`, packaged smoke scripts.

## Decisions

- Do not claim Linux tray support from a single desktop environment.
- Windows and macOS behavior validation are queued as separate chunks after
  Linux behavior is proven.

## Verification

- Command: `pnpm verify`
- Outcome: not run yet
- Command: GitHub Actions release workflow dry-run `28090081218` on `main` with
  `publish=false`
- Outcome: prerequisite artifact evidence passed; all six native build targets
  built and staged unsigned artifacts before this behavior-validation chunk
  began.
- Command: `pnpm platform-behavior:test && pnpm platform-behavior:check`
- Outcome: passed; the supported platform/environment matrix now requires
  Windows x64/ARM64, macOS Apple Silicon/Intel, Linux GNOME x64/ARM64, and
  Linux KDE x64 installed-smoke evidence with explicit capability expectations.
- Command: `pnpm format:check docs/engineering/cross-platform-behavior.md docs/engineering/linux-platform-behavior.md docs/engineering/platform-behavior-matrix.json scripts/harness/check-platform-behavior.mjs scripts/smoke-linux-deb.mjs package.json docs/exec-plans/active/2026-06-20_phase-10-overview.md docs/exec-plans/active/2026-06-20_phase-10d-linux-behavior.md docs/exec-plans/completed/2026-06-20_phase-10c-packaging-metadata.md docs/exec-plans/completed/2026-06-20_phase-10e-ci-release-workflow.md docs/exec-plans/queued/2026-06-20_phase-10d-windows-behavior.md docs/exec-plans/queued/2026-06-20_phase-10d-macos-behavior.md`
- Outcome: passed.
- Command: `pnpm harness:check`
- Outcome: passed; the new platform behavior harness is wired into the
  canonical architecture/release harness gate.
- Command: `pnpm linux-smoke:deb /tmp/burnly-linux-smoke-artifact/burnly-v0.1.0-linux-x86_64.deb`
- Outcome: passed; Debian metadata, desktop entry, icon payload, app
  executable, sidecar manifest/checksum, and `ccusage 20.0.14` execution passed
  for the x64 release artifact from run `28090081218`.
- Command: `pnpm linux-smoke:deb /tmp/burnly-linux-arm64-smoke-artifact/burnly-v0.1.0-linux-aarch64.deb`
- Outcome: passed; Debian metadata, desktop entry, icon payload, app
  executable, and sidecar manifest/checksum passed for the ARM64 release
  artifact from run `28090081218`. Sidecar execution was skipped because the
  artifact architecture does not match the x64 host.
- Command: `sudo -n true`
- Outcome: failed; passwordless sudo is unavailable, so this host cannot perform
  a non-interactive system-level install smoke.
- Command: `pkexec /usr/bin/apt-get install -y /tmp/burnly-linux-smoke-artifact/burnly-v0.1.0-linux-x86_64.deb`
- Outcome: passed after Polkit authentication; `burnly 0.1.0 amd64` is
  installed on this GNOME host.
- Command: `dpkg-query -W -f='${Package} ${Version} ${Architecture} ${Status}\n' burnly`
- Outcome: passed; package status is `burnly 0.1.0 amd64 install ok installed`.
- Command: `/usr/lib/Burnly/sidecars/ccusage/ccusage --version`
- Outcome: passed; installed packaged sidecar reports `ccusage 20.0.14`.
- Command: `gtk-launch Burnly`
- Outcome: passed; installed desktop entry launched without command failure.
- Command: `/usr/bin/burnly`
- Outcome: passed for manual smoke; installed process stayed running on GNOME
  X11 for interactive desktop testing.
- Command: `pnpm verify:runtime`
- Outcome: passed on Ubuntu 24.04 x86_64, GNOME, X11; Tauri prerequisite,
  contract, frontend build, IPC bridge, platform lifecycle/tray unit, refresh
  scheduler, and 30 Playwright desktop/compact evidence tests passed.

## Runtime Evidence

- Declared matrix is recorded in
  `docs/engineering/platform-behavior-matrix.json` and explained in
  `docs/engineering/cross-platform-behavior.md`.
- Linux artifact and GNOME/X11 runtime evidence is recorded in
  `docs/engineering/linux-platform-behavior.md`.
- KDE x64 installed smoke evidence remains required before Phase 10D-Linux can
  complete.

## Follow-Up Debt

- Use the successful Phase 10E Linux artifacts as the baseline for installed
  behavior validation; do not substitute configuration-only checks for platform
  evidence.
