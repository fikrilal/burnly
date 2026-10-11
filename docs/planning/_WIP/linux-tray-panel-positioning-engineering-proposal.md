# Linux Tray Panel Positioning Engineering Proposal

## Status

Draft engineering proposal, written 2026-10-11 after diagnosing the "tray panel
opens at screen center" report against the v0.1.34 codebase and a live Ubuntu
GNOME Wayland session.

**Accepted for implementation on 2026-10-11.** Implementation is tracked by
`docs/exec-plans/completed/2026-10-11_linux-tray-panel-positioning.md` and
verified with runtime evidence in
`docs/runtime-evidence/2026-10-11-wayland-panel-positioning/`.

## Summary

On Linux Wayland sessions where a usable XWayland display exists, Burnly should
select the GTK X11 backend for its process at startup, before GTK initializes.
That restores the panel's deterministic top-right placement, which is currently
a silent no-op under Wayland. When no usable X display exists — or a user has
explicitly chosen a backend via `GDK_BACKEND` — Burnly stays on native Wayland,
records a diagnostic, and accepts compositor window placement.

A companion placement change selects the monitor under the pointer for corner
placement, so the panel opens on the display whose tray indicator the user just
triggered, instead of relying on a possibly-unresolved window monitor.

Windows and macOS behavior is unchanged: the panel remains anchored to the tray
icon rectangle.

## Context

The tray panel is Burnly's primary product surface
(`docs/product/tray-panel-contract.md`). On Linux it is opened from the native
tray menu:

- `src-tauri/src/platform/tray.rs` puts the menu on left click
  (`show_menu_on_left_click(cfg!(target_os = "linux"))`).
- `src-tauri/src/bootstrap/runtime_events.rs` routes the `Open Summary` action
  to `tray_runtime::open_tray_panel(app, None)`. The icon-rectangle anchor
  variant is reached only from `RunEvent::TrayIconEvent`, which is compiled for
  Windows and macOS only. Linux AppIndicator trays provide neither click
  geometry nor icon coordinates, so icon-anchored placement is not available on
  Linux by design.
- `src-tauri/src/platform/lifecycle.rs` therefore positions the panel in the
  top-right corner of a monitor (`position_tray_panel`), and anchors to a tray
  rectangle only when the Windows/macOS event supplies one
  (`position_tray_panel_at_rect`).

Both paths call `set_position` on the hidden Tauri window before showing it.

## Problem

Under a Wayland session, `set_position` (and the GTK `gtk_window_move` beneath
it) is ignored by the compositor; Wayland deliberately gives clients no API to
place a toplevel window. Upstream issues describe this as a silent no-op:

- tao issue #566 — `set_outer_position` has no effect under Wayland.
- tauri issue #14913 — window position/order APIs silently no-op on
  GNOME+Wayland, including `WebviewWindowBuilder::position`; the same issue
  reports `always_on_top` and `skip_taskbar` as non-operational.

Measured on the affected machine (2026-10-11, Ubuntu GNOME,
`XDG_SESSION_TYPE=wayland`):

- The installed AppImage runs as a native Wayland client: no `GDK_BACKEND`
  override in the process environment, and the open panel does not appear in
  the X server window tree.
- Opening the panel from `Open Summary` leaves it wherever the compositor
  places it — reported as screen center — instead of the top-right corner
  computed by `position_tray_panel`.

Two structural gaps beyond the backend issue:

1. `position_tray_panel` silently returns when `current_monitor()` resolves to
   nothing, so placement can be skipped with no error and no diagnostic.
2. Monitor selection ignores the pointer, so a tray action on a non-primary
   monitor can target the wrong display even where positioning works.

This is not a regression from a recent change. The last commit touching
positioning is `59f8aed` (2026-06-30, Windows polish); releases v0.1.31–v0.1.34
did not modify it. The June Linux validation was captured on X11
(`docs/exec-plans/completed/2026-06-25_tray-linux-runtime-evidence.md`:
"Ubuntu GNOME on X11"), so the Wayland path was never proven.

## Goals

1. Restore deterministic panel placement on Linux Wayland sessions that have
   XWayland: top-right corner of the monitor under the pointer, on first open
   and on every re-open.
2. Never prevent startup: on Wayland-only systems (no usable X display) the app
   still launches and behaves as it does today.
3. Respect the standard GTK override: a user- or environment-provided
   `GDK_BACKEND` value always wins, in both directions.
4. Make the effective backend and placement decision visible in local
   diagnostics.
5. Leave Windows/macOS placement, the panel UI, the IPC contract, and the
   product contract unchanged.

## Non-goals

- Pixel-anchoring the panel to the tray icon on Linux. StatusNotifierItem /
  AppIndicator exposes no geometry, and the indicator surface belongs to the
  shell; there is no supported way for an application to know where its icon
  is.
- Adopting wlr-layer-shell or any compositor-specific surface protocol.
- Per-desktop corner policy (for example KDE's bottom tray) — the existing
  corner policy stays.
- Remembering a user-dragged panel position across restarts.
- Changing the Windows/macOS tray-rectangle anchor path.

## Proposed design

### 1. Display-backend selection (new `platform::display_backend`)

A new platform module selects the GTK backend for the process on Linux and is
called as the first statement of `bootstrap::run()`, before any Tauri/GTK
initialization. It only sets the process environment; beyond backend choice,
nothing about the window model changes.

Decision policy (pure function over an environment snapshot; unit-testable):

| Condition                                                        | Action                                   |
| ---------------------------------------------------------------- | ---------------------------------------- |
| `GDK_BACKEND` already set (any platform)                         | Respect it; do nothing                   |
| Not Linux                                                        | Do nothing                               |
| Linux, Wayland session signal, reachable local X display         | Set `GDK_BACKEND=x11` (run via XWayland) |
| Linux, Wayland session signal, no `DISPLAY` or unconfirmed probe | Stay Wayland; record the reason          |
| Linux, X11 session                                               | Do nothing (unchanged)                   |

Wayland session signal: `WAYLAND_DISPLAY` set or `XDG_SESSION_TYPE=wayland`.

The X-display check is deliberately conservative: parse local `:N[.M]` display
values and attempt a Unix-socket connection (abstract namespace first, then
filesystem socket). If a working display cannot be confirmed, X11 is not
forced — a false negative only keeps the panel unpositioned (and diagnosable),
while a false positive could abort GTK startup.

The decision and its reason are stored in a process-wide cell so diagnostics
can report them after startup.

### 2. Monitor selection for corner placement (`platform::lifecycle`)

Replace the single `current_monitor()` lookup with a precedence chain, used by
the corner path and as the rectangle path's fallback:

1. monitor containing the pointer (`cursor_position` + `monitor_from_point`);
2. `current_monitor()`;
3. `primary_monitor()`;
4. no-op (unchanged last resort).

The pointer is the closest available proxy for "where the icon is": the user
just triggered the tray action on that monitor. This also removes the silent
first-lookup failure mode and fixes multi-monitor placement wherever
positioning is available (X11 and XWayland, Windows, macOS fallback).

### 3. Diagnostics

After the diagnostics service initializes, record one `Runtime`-area event per
launch with the effective backend and the decision reason (auto-selected,
manual `GDK_BACKEND`, no X display, probe failed). No sensitive data; this is
the only new observability.

### Runtime flow

1. `burnly_desktop_lib::run` → `bootstrap::run`.
2. Display-backend selection runs (environment only; no GTK yet).
3. Tauri/GTK initializes on the selected backend.
4. Setup records the backend decision as a diagnostic event.
5. A tray action opens the panel:
   - Windows/macOS: anchored to the tray rectangle (unchanged);
   - Linux X11 / Wayland-with-XWayland: top-right of the pointer's monitor;
   - native Wayland: compositor placement (today's behavior).

## Platform behavior matrix

| Session                                           | GTK backend     | Panel placement                          |
| ------------------------------------------------- | --------------- | ---------------------------------------- |
| Linux X11                                         | X11 (unchanged) | top-right of pointer monitor             |
| Linux Wayland + XWayland                          | X11 (auto)      | top-right of pointer monitor             |
| Linux Wayland, no X display, or `GDK_BACKEND` set | Wayland         | compositor placement; diagnostic records |
| macOS                                             | unchanged       | anchored to tray rectangle               |
| Windows                                           | unchanged       | anchored to tray rectangle               |

Secondary effect of the X11 backend on Wayland sessions: `always_on_top` and
`skip_taskbar` for the panel start taking effect again, which matches the
panel's popover semantics.

## Alternatives considered

1. **Accept compositor placement on Wayland (status quo).** Rejected: the
   primary product surface opens away from any tray context, which is the
   reported defect.
2. **Set `GDK_BACKEND=x11` only in launchers (desktop entry, AppImage wrapper).**
   Rejected: misses `pnpm tauri dev`, direct binary launches, and updater
   relaunches; cannot be selectively disabled; splits behavior across launch
   paths instead of one code path.
3. **Re-exec the process with `GDK_BACKEND=x11`.** Rejected as unnecessary:
   GTK reads the variable when it opens the display, so in-process selection
   before Tauri init is sufficient, and it avoids argv/env passthrough, a loop
   sentinel, and interaction with single-instance and updater relaunch. Keep as
   a fallback if a future dependency initializes GTK before `bootstrap::run()`.
4. **wlr-layer-shell (gtk-layer-shell).** Rejected for now: Mutter does not
   implement layer-shell on GNOME (only third-party forks do), so it would not
   fix the primary environment; it would add FFI plus native library bundling
   for KDE/wlroots only. Revisit if Mutter adopts layer-shell or KDE-native
   anchoring becomes a requirement.
5. **GNOME Shell extension that moves our window** (the workaround discussed in
   tauri#14913). Rejected: requires each user to install and enable a
   version-locked third-party extension; not shippable inside a standalone
   utility.
6. **Transparent full-monitor overlay with the panel drawn inside it.**
   Rejected: needs raw-GTK input-region surgery for click-through, and it
   breaks blur-to-dismiss semantics; high defect risk for a cosmetic gain.
7. **Popup surface attached to a parent toplevel.** Impossible: popups are
   positioned relative to a parent surface; the app cannot position the parent
   and cannot attach to the shell-owned indicator surface.
8. **Move the panel content into the shell-rendered tray menu.** Rejected: the
   rich panel is the product surface (`docs/product/tray-panel-contract.md`); a
   DBusMenu cannot host it.
9. **`xdg-session-management-v1` (staging protocol).** Not a fix: it is a
   compositor-dependent session-restore mechanism, not anchored placement.
   Watch item only.

## Risks and tradeoffs

- **XWayland fidelity.** Running under X11 can show blur under fractional
  scaling and loses native Wayland input paths. Mitigation: the documented
  `GDK_BACKEND=wayland` override, and the tradeoff is accepted deliberately for
  deterministic placement of the primary surface.
- **Security isolation.** X11 clients share the X server trust domain
  (XWayland remains isolated from native Wayland clients). The panel displays
  aggregate token counts only; no secrets are entered there. Accepted.
- **XWayland absent or unprobeable.** The panel stays centered;
  the diagnostic and the `GDK_BACKEND=x11` manual override keep this
  supportable.
- **Startup safety.** Forcing X11 against a broken display could abort GTK
  init, so the probe is intentionally conservative: unconfirmed means "stay
  Wayland".
- **Dev/prod parity.** The shim applies in development too, which is desirable;
  one smoke lane must run with `GDK_BACKEND=wayland` to keep the native path
  honest.
- **Upstream change.** If Tauri gains Wayland positioning (tauri#14913) or
  GNOME adopts a usable protocol, remove the shim. That is the removal trigger.
- **Rollback.** No data or contract changes are involved: reverting the
  selection change restores today's behavior, and `GDK_BACKEND=wayland`
  disables the shim in the field without a release.

## Acceptance criteria

1. Installed AppImage on Ubuntu GNOME Wayland with XWayland: the panel opens
   top-right on the first open after launch and after every toggle; evidence
   shows window geometry and a screenshot (runtime evidence document).
2. Multi-monitor: triggering the tray action while the pointer is on a
   secondary monitor places the panel on that monitor.
3. The panel process is an X11 client while the shim is active (visible in the
   X server window tree), proving selection happens before GTK init.
4. X11 session regression: placement unchanged.
5. `GDK_BACKEND=wayland` pre-set: app starts, stays native Wayland, placement
   is compositor-decided, and the diagnostic records the manual override.
6. Wayland session without a reachable X display: app starts; the diagnostic
   records the fallback reason.
7. Unit tests cover the decision matrix and monitor precedence; existing
   lifecycle tests and `pnpm verify` pass.
8. `docs/engineering/known-limitations.md` gains the Wayland placement entry
   (cause, shim behavior, workaround, revisit trigger), and the desktop runtime
   evidence checklist (`docs/engineering/desktop-runtime-evidence.md`) gains a
   "panel opens top-right, not centered" step for both session types.

Implementation sequencing, exact commands, and evidence logs belong to a
follow-up execution plan, not this document.

## Open questions

1. Treat `XDG_SESSION_TYPE=wayland` alone (without `WAYLAND_DISPLAY`) as a
   Wayland session? Recommended: yes — either signal is sufficient.
2. Add a product-level opt-out flag in addition to `GDK_BACKEND`? Recommended:
   no; the standard GTK variable is documented and testable, and one override is
   enough.
3. If `DISPLAY` is set but the probe is inconclusive, force X11 anyway?
   Recommended: no (startup safety); record the reason so it is diagnosable.
4. Should KDE sessions get a bottom-right corner to match the Plasma tray?
   Out of scope here; revisit with KDE runtime evidence if it becomes a real
   complaint.

## References

- `src-tauri/src/platform/lifecycle.rs` — placement and activation today
- `src-tauri/src/bootstrap/runtime_events.rs`, `src-tauri/src/platform/tray.rs`
  — tray triggers
- `docs/exec-plans/completed/2026-06-26_tray-popover-behavior.md` —
  corner-placement decision
- `docs/exec-plans/completed/2026-06-25_tray-linux-runtime-evidence.md` — June
  validation (X11)
- `docs/product/tray-panel-contract.md` — panel product contract
- tao issue #566, tauri issue #14913 — Wayland positioning is a client-side
  no-op
