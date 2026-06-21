# Desktop Runtime Evidence

Runtime evidence is required for behavior that static checks cannot prove.

Use `pnpm evidence:desktop` as the entry point. `pnpm verify:runtime` delegates
to the same command and is the named gate for desktop-native changes.

## Automated Gate

`pnpm evidence:desktop` records:

- Local platform, architecture, operating system, desktop session, display, and
  Wayland display values when the environment reports them.
- Tauri prerequisite output.
- IPC contract drift check.
- Frontend production build.
- Tauri IPC bridge tests.
- Phase 7 platform lifecycle, tray action mapping, and background scheduler unit
  evidence.
- Playwright desktop UI evidence.

Do not wire `pnpm verify:runtime` into `pnpm verify`. Runtime evidence has
heavier desktop prerequisites and should stay explicit.

## Manual Smoke Checklist

Use this checklist when a change touches native window lifecycle, background
refresh, tray behavior, single-instance behavior, sidecars, packaging, updates,
or any behavior the automated harness cannot inspect.

Record the platform and command results in the active execution plan:

```bash
uname -a
printf 'desktop=%s\nsessionType=%s\ndisplay=%s\nwaylandDisplay=%s\n' \
  "${XDG_CURRENT_DESKTOP:-unreported}" \
  "${XDG_SESSION_TYPE:-unreported}" \
  "${DISPLAY:-unreported}" \
  "${WAYLAND_DISPLAY:-unreported}"
pnpm verify:runtime
```

For Phase 7 desktop lifecycle and tray behavior, run Burnly in dev mode:

```bash
pnpm tauri dev
```

Then verify:

- Main window opens and renders the dashboard.
- Closing the main window follows the selected close behavior.
- With close behavior set to hide, closing the main window hides it without
  terminating the app.
- The native tray icon is visible when the current desktop environment supports
  tray icons.
- Tray Open Burnly shows, unminimizes, and focuses the main window.
- Tray Refresh now starts a refresh and does not start duplicate concurrent
  refresh jobs.
- Tray status changes while refresh is active and returns to a terminal state.
- Tray Quit terminates the app process.
- Launching a second app instance focuses the existing main window instead of
  creating a competing runtime.
- Resume/wake behavior triggers a refresh where the platform emits a supported
  resume event.

If a step cannot be observed on the current platform, record the exact
limitation. Do not extrapolate evidence to macOS, Windows, GNOME, KDE, Wayland,
or X11 unless that environment was actually tested.

## Examples

Runtime evidence is required for:

- Tauri window startup.
- Tray behavior on the target operating system.
- Packaged app sidecar lookup.
- Packaged migration startup.
- Background refresh scheduling and cancellation.
- Native close, hide, reopen, and quit behavior.
