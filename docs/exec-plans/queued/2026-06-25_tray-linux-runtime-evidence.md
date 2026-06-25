# 2026-06-25 Tray Linux Runtime Evidence

## Objective

Validate the installed Linux app as a tray-first compact tracker using real
local data.

This chunk should prove the product shape before full desktop redesign begins.

## Acceptance Criteria

- Debug Debian package builds and installs.
- Installed Burnly starts from the system environment.
- Tray/menu-bar interaction opens the compact tray panel.
- Panel displays real local token data.
- Auto-refresh behavior is observable and does not require a primary refresh
  button.
- `Open details` opens/focuses the full desktop window on Summary.
- Diagnostics remain available for failure investigation.
- Evidence is recorded in the active execution plan or runtime evidence docs.

## Risk Class

`high`

Desktop tray behavior is platform-specific and can differ across Linux desktop
environments, X11/Wayland, and tray hosts.

## Impact Areas

- Linux packaging
- Tauri tray/window runtime
- Installed sidecar execution
- Real SQLite app data
- Runtime evidence scripts/manual checks

## Design Review

- Complexity introduced: installed-app validation of a rich tray panel.
- Owning module: platform runtime evidence should validate product behavior, not
  replace unit/integration tests.
- Interface depth: evidence should use installed package behavior rather than
  dev-server assumptions.
- Special cases: missing tray host, Wayland/X11 differences, polkit install
  prompts, existing running app instance.
- New abstraction needed now: optional smoke helper only if manual evidence is
  too repetitive or error-prone.

## Checklist

- [ ] Build debug Debian package.
- [ ] Reinstall package.
- [ ] Launch installed app.
- [ ] Open tray panel through Linux tray/menu behavior.
- [ ] Confirm compact panel displays real data.
- [ ] Confirm auto-refresh/freshness state.
- [ ] Confirm no primary refresh button in tray panel.
- [ ] Confirm `Open details` lands on Summary.
- [ ] Record commands, screenshots/logs, and database evidence.

## Test Plan

- Behavior and invariants to prove:
  - installed app uses packaged sidecar,
  - compact tray panel appears,
  - data is real persisted local usage,
  - auto-refresh does not fail with collector envelope errors,
  - full details remains accessible.
- Lowest stable test layer:
  - runtime/manual evidence; lower layers covered by previous chunks.
- Failure paths:
  - tray unavailable,
  - app already running,
  - sidecar failure,
  - no local data,
  - partial refresh.
- Fixtures or fakes:
  - none for final runtime evidence; use real local data.
- Runtime or platform evidence:
  - Ubuntu/GNOME Linux installed app evidence first.
- Relevant commands:
  - `pnpm tauri build --debug --bundles deb`
  - package reinstall command
  - installed app launch command
  - SQLite verification queries
  - screenshot/log capture commands as needed

## Decisions

- Linux is the first runtime target for tray-first validation.
- Windows and macOS follow only after Linux product shape is proven.

## Verification

- Command: `pnpm verify`
- Outcome: not run yet.

## Runtime Evidence

- Not collected yet.

## Follow-Up Debt

- Add Windows/macOS tray-specific evidence after Linux is stable.
