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

- [x] Build debug Debian package.
- [x] Reinstall package.
- [x] Launch installed app.
- [x] Open tray panel through Linux tray/menu behavior.
- [x] Confirm compact panel displays real data.
- [x] Confirm auto-refresh/freshness state.
- [x] Confirm no primary refresh button in tray panel.
- [x] Confirm `Open details` lands on Summary.
- [x] Record commands, screenshots/logs, and database evidence.

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

- Command: `pnpm rust:test platform::lifecycle`
- Outcome: passed. Covered lifecycle behavior, including creating the main
  details window when missing.
- Command: `pnpm tauri build --debug --bundles deb`
- Outcome: passed. Built
  `src-tauri/target/debug/bundle/deb/Burnly_0.1.0_amd64.deb`.
- Command:
  `pkexec /usr/bin/apt-get install -y "$PWD/src-tauri/target/debug/bundle/deb/Burnly_0.1.0_amd64.deb"`
- Outcome: passed. Installed `burnly 0.1.0 amd64`.
- Command:
  `pnpm linux-smoke:deb src-tauri/target/debug/bundle/deb/Burnly_0.1.0_amd64.deb`
- Outcome: passed. Verified desktop entry, executable, sidecar manifest, and
  `ccusage 20.0.14` sidecar hash
  `dfcd0ea98fc56d71cff77db000d307b011fe218333ac93f7697d242e1f587e35`.
- Command: `pnpm verify:fast`
- Outcome: passed. Existing ESLint complexity/size warnings remained warnings.
- Command: `pnpm verify:runtime`
- Outcome: passed. Desktop runtime evidence passed on Linux/X11.

## Runtime Evidence

- Platform:
  - OS: Ubuntu 24.04, Linux `6.17.0-35-generic`, `x86_64`.
  - Desktop: Ubuntu GNOME on X11.
  - Display: `:1`.
- Privilege/install:
  - `sudo -n true` was unavailable; install used `pkexec`.
  - `dpkg-query` confirmed `burnly 0.1.0 amd64 install ok installed`.
- Installed sidecar:
  - `/usr/lib/Burnly/sidecars/ccusage/ccusage --version` returned
    `ccusage 20.0.14`.
  - Sidecar SHA-256:
    `dfcd0ea98fc56d71cff77db000d307b011fe218333ac93f7697d242e1f587e35`.
- Real local data:
  - Active daily rows: `300`.
  - Active daily token total: `39116891701`.
  - Today token total: `28816885`.
  - Today model usage: `gpt-5.5|28816885`.
- Auto-refresh/freshness:
  - Latest launch refresh completed as
    `12|succeeded|launch|1782438843927|1782438878592`.
  - Tray panel displayed `Current` and
    `Updated Jun 26, 2026, 8:54 AM`.
- Tray/menu interaction:
  - GNOME StatusNotifier registered
    `:1.180@/org/ayatana/NotificationItem/tray_icon_tray_app_burnly_tray`.
  - DBus menu layout exposed `Open Summary`, `Open Details`, status,
    `Refresh now`, and `Quit Burnly`.
  - Triggering the `Open Summary` menu event opened the compact
    `360x520` tray panel window.
- Visual evidence:
  - Tray panel:
    `docs/runtime-evidence/2026-06-26-tray-linux/tray-panel-after-reinstall.png`.
  - Main window after `Open details`:
    `docs/runtime-evidence/2026-06-26-tray-linux/main-window-after-open-details.png`.
- `Open details` issue found and fixed:
  - The details action previously only activated an existing `main` window.
  - Tray-first runtime can have no existing main window, so the action could
    fail from the compact panel.
  - Lifecycle now creates the main details window when missing, focuses it, and
    hides the tray panel.
  - Runtime evidence after the fix:
    - active window became the `1180x760` main Burnly window,
    - tray panel map state became `IsUnMapped`,
    - main window landed on `Overview`.

## Follow-Up Debt

- Add Windows/macOS tray-specific evidence after Linux is stable.
