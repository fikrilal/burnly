# Wayland Tray Panel Positioning Runtime Evidence

Date: 2026-10-11
Plan: `docs/exec-plans/active/2026-10-11_linux-tray-panel-positioning.md`
Proposal: `docs/planning/_WIP/linux-tray-panel-positioning-engineering-proposal.md`

Platform: Ubuntu GNOME, Wayland session with XWayland (`XDG_SESSION_TYPE=wayland`,
`WAYLAND_DISPLAY=wayland-0`, `DISPLAY=:0`), single 3440x1440 monitor at scale 1.

Build: development working tree on top of `1192fee`, debug build run via
`pnpm tauri dev` with isolated `XDG_DATA_HOME` / `XDG_CACHE_HOME` /
`XDG_CONFIG_HOME`. The installed v0.1.34 AppImage kept running as the
native-Wayland control.

## Automated backend selection

- The dev process selected the X11 backend without an external `GDK_BACKEND`
  value: its tray panel window appears in the X server window tree.
- X server window tree while only the control (installed, native Wayland)
  instance was running: no Burnly windows. After the dev instance started, the
  X tree contained the panel window (`"Burnly" 440x540+2988+48`) and GTK's
  InputOnly helper.
- Startup diagnostic row (isolated database):

  ```text
  runtime|info|platform.display_backend|{"backend":"x11","reason":"wayland_x11_selected"}
  ```

## Panel placement

Opened via the tray "Open Summary" D-Bus menu event
(`com.canonical.dbusmenu.Event` on the dev instance's
`/org/ayatana/NotificationItem/tray_icon_tray_app_burnly_tray/Menu`, item id 2):

- Observed geometry: `Position: 2988,48`, `Geometry: 440x540`,
  `Map State: IsViewable`.
- Expected top-right corner: `3440 - 440 - 12 = 2988`, `y = 48`.
- Screen center would be approximately `1500,450`; the panel is not centered.
- Repeated menu events toggle visibility; every open lands at the same
  top-right position.

## Opt-out path (`GDK_BACKEND=wayland`)

A second isolated run of the same binary with `GDK_BACKEND=wayland`:

- Started successfully as a native Wayland client and added no X server windows.
- Startup diagnostic:

  ```text
  runtime|info|platform.display_backend|{"backend":"wayland","reason":"manual_env_value"}
  ```

## Limitations

- No screenshot tooling was available (`import`, `xwd`, `gnome-screenshot`, and
  `scrot` are absent); an ffmpeg x11grab capture of the XWayland region rendered
  black, and `org.gnome.Shell.Screenshot` denies non-shell callers. Placement is
  proven by window geometry and X-window identity; visual confirmation of the
  rendered panel remains a manual step.
- Single-monitor session; multi-monitor pointer precedence is covered by unit
  tests only.
- Only GNOME Wayland was exercised. X11-session regression is covered by unit
  tests; KDE was not tested.
