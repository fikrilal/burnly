# 2026-06-29 Windows Release 03 Runtime Evidence

## Objective

Verify Burnly runs correctly on Windows from the packaged `.exe` installer,
including tray behavior, refresh, launch-at-login, bundled `ccusage`, SQLite
storage, and updater execution.

## Acceptance Criteria

- Windows `.exe` installs Burnly on a real Windows machine or VM.
- Burnly launches and opens the tray panel.
- Packaged `ccusage` sidecar is found and can refresh usage data.
- SQLite database is created under the expected Windows app data location.
- Launch-at-login can be toggled and does not create a broken startup entry.
- Manual update check can detect a newer release.
- Update install and restart works from one released test version to another.
- Runtime evidence is recorded in this plan.

## Risk Class

`high`

## Impact Areas

- Windows runtime packaging
- Tray integration
- Sidecar execution
- SQLite paths
- Autostart integration
- Updater runtime

## Design Review

- What complexity is being introduced?
  - Windows shell/runtime behavior differs from Linux for tray, startup, paths,
    process execution, and installer security prompts.
- Which decisions are hidden inside the owning module?
  - Platform-specific path and launch-at-login behavior should remain behind
    Rust infrastructure/platform modules.
- Is each new interface simpler than its implementation?
  - The frontend should continue using the same IPC contracts.
- What special cases exist, and can the design eliminate them?
  - Windows filesystem paths and startup registration are platform-specific and
    should not leak into application/domain code.
- Why is each new abstraction needed now?
  - No new abstraction is expected unless runtime evidence reveals repeated
    platform branching.
- Can an existing module absorb this responsibility cleanly?
  - Existing platform modules and runtime services should absorb fixes.

## Checklist

- [ ] Install the packaged Windows `.exe`.
- [ ] Launch Burnly from Start menu or installed shortcut.
- [ ] Open tray panel and verify runtime is available.
- [ ] Trigger refresh and inspect refresh/import run state.
- [ ] Verify packaged `ccusage` sidecar path and execution.
- [ ] Verify database path and schema health.
- [ ] Toggle launch-at-login and verify restart/login behavior.
- [ ] Test updater from an older Windows build to a newer one.
- [x] Add a concrete Windows evidence contract and validator.
- [x] Correct platform behavior policy for Windows x64 updater evidence and
      deferred Windows ARM64 support.
- [ ] Record real Windows evidence and any platform fixes.
- [ ] Run relevant gates after fixes.

## Test Plan

- Behavior and invariants to prove:
  - Packaged app refresh succeeds on Windows.
  - Sidecar lookup does not rely on Linux resource paths.
  - Tray panel can reach desktop runtime.
  - Update install restarts into the newer version.
  - Launch-at-login points at the installed app, not a dev server or stale path.
- Lowest stable test layer:
  - Manual Windows runtime evidence plus targeted tests for any code fixes.
- Failure paths:
  - Sidecar missing.
  - Runtime unavailable.
  - Refresh stuck in running state.
  - Updater unavailable or bad signature.
  - Launch-at-login broken after reboot.
- Fixtures or fakes:
  - Older and newer Windows release artifacts.
- Runtime or platform evidence:
  - Required. Record OS version, install path, app version, update path, and
    refresh status.
- Relevant commands:
  - `pnpm verify`
  - `pnpm verify:windows` if available on the Windows environment
  - Windows package install/update commands recorded during evidence collection

## Decisions

- Windows public release cannot proceed without real Windows runtime/update
  evidence.
- Windows x64 is the only Windows release target in this phase.
- Windows ARM64 remains deferred until it has a release workflow target and
  runtime evidence.
- Runtime evidence must be validated by
  `pnpm windows-runtime:evidence:check <evidence.json>`.

## Verification

- Command: `pnpm windows-runtime:evidence:check <temporary passing fixture>`
- Outcome: passed
- Command:
  `pnpm windows-runtime:evidence:check docs/engineering/evidence/windows-runtime-evidence.template.json`
- Outcome: failed as expected because the template contains pending
  placeholders, not real runtime evidence
- Command: `pnpm platform-behavior:test && pnpm platform-behavior:check`
- Outcome: passed
- Command: `pnpm format:check`
- Outcome: passed
- Command: `pnpm verify`
- Outcome: passed; duplication report printed existing non-failing clones

## Runtime Evidence

- Required before completing this phase. Not collected in this Linux
  environment.

## Follow-Up Debt

- Phase 4 should resolve code-signing and public install documentation.
