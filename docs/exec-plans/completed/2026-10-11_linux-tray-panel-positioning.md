# 2026-10-11 Linux Tray Panel Positioning

## Objective

Restore deterministic tray panel placement on Linux Wayland sessions by
selecting the GTK X11 backend before GTK initializes when a reachable XWayland
display exists, and by choosing the placement monitor in pointer → current →
primary precedence order.

Implements `docs/planning/_WIP/linux-tray-panel-positioning-engineering-proposal.md`.

## Acceptance Criteria

- On a Wayland session with XWayland, the panel opens at the top-right corner of
  the pointer's monitor on first open and after every toggle.
- X11 sessions keep today's placement behavior.
- A pre-set `GDK_BACKEND` is always respected; with no reachable X display the
  app still starts and stays on native Wayland.
- One `Runtime`-area diagnostic event per Linux launch records the effective
  backend and decision reason.
- Unit tests cover the decision matrix, local display parsing, and monitor
  precedence; `pnpm verify` passes.
- `docs/engineering/known-limitations.md` and the desktop runtime evidence
  checklist document the behavior and revisit trigger.
- Installed-runtime evidence shows the panel as an X11 client with top-right
  geometry, not centered.

## Risk Class

`medium`.

Startup-path behavior change on Linux with a new environment mutation before
GTK initialization; a mistake could affect app startup or panel placement for
all Linux users.

## Impact Areas

- `src-tauri/src/platform/display_backend.rs` (new)
- `src-tauri/src/platform/mod.rs`
- `src-tauri/src/platform/lifecycle.rs`
- `src-tauri/src/bootstrap.rs`
- `docs/engineering/known-limitations.md`
- `docs/engineering/desktop-runtime-evidence.md`
- `docs/planning/_WIP/linux-tray-panel-positioning-engineering-proposal.md`

## Design Review

- What complexity is being introduced? One startup-time backend selection with a
  pure decision function, plus a monitor precedence chain; no new interfaces for
  callers other than one bootstrap call and one diagnostics read.
- Which decisions are hidden inside the owning module? Environment detection,
  the X display probe, and the `GDK_BACKEND` write live in
  `platform::display_backend`; monitor choice lives in `platform::lifecycle`.
- Is each new interface simpler than its implementation? Yes — `select_display_backend()`
  is one call; `decide(environment, probe)` is pure and unit-testable.
- What special cases exist, and can the design eliminate them? Manual
  `GDK_BACKEND`, no `DISPLAY`, unreachable display, and unset session variables
  are represented as explicit decision reasons instead of scattered branches.
- Why is each new abstraction needed now? The Wayland placement defect is
  user-visible on the primary product surface; the module keeps the workaround
  removable in one place when upstream positioning arrives.
- Can an existing module absorb this responsibility cleanly? `platform` already
  owns OS/Tauri lifecycle behavior; the new module sits beside `lifecycle` and
  `tray` and keeps GTK-backend knowledge out of bootstrap.

## Checklist

- [x] Write the execution plan.
- [x] Add `platform::display_backend` with the pure decision function, X display
      probe, and `GDK_BACKEND` selection.
- [x] Wire selection into `bootstrap::run()` before Tauri/GTK init.
- [x] Record the decision as a startup diagnostic event.
- [x] Replace `current_monitor()`-only lookup with pointer → current → primary
      precedence in `platform::lifecycle`.
- [x] Add unit tests for the decision matrix, display parsing, and monitor
      precedence.
- [x] Update `known-limitations.md` and the runtime evidence checklist; mark the
      proposal accepted.
- [x] Run focused tests, clippy, fmt, `pnpm verify:fast`, `pnpm verify`.
- [x] Collect runtime evidence on this GNOME Wayland machine.
- [x] Record commands and outcomes below.

## Test Plan

- Behavior and invariants to prove:
  - manual `GDK_BACKEND` always wins and is never overwritten;
  - a non-Wayland session changes nothing;
  - a Wayland session with no `DISPLAY` stays on Wayland;
  - a Wayland session with a reachable display selects X11 (`GDK_BACKEND=x11`);
  - an unreachable display probe keeps Wayland (fail-safe);
  - `XDG_SESSION_TYPE=wayland` alone counts as a Wayland session;
  - local display strings (`:0`, `:0.1`, `unix:0`) parse; remote or malformed
    strings do not;
  - monitor precedence prefers pointer, then current, then primary.
- Lowest stable test layer: Rust unit tests for the pure decision and parsing
  functions; the probe and monitor lookup stay thin.
- Failure paths: missing `DISPLAY`, probe failure, manual override, non-Linux
  no-op.
- Fixtures or fakes: environment snapshots and injected probe closures; no
  filesystem or X server dependency in unit tests.
- Runtime or platform evidence: dev/packaged run on the GNOME Wayland session,
  panel geometry and X server window tree, diagnostic row inspection.
- Relevant commands:
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib display_backend`
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib platform::lifecycle`
  - `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
  - `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
  - `pnpm verify:fast`
  - `pnpm verify`
  - `pnpm verify:runtime`

## Decisions

- Select the backend in-process before Tauri init instead of re-executing the
  process; GTK reads `GDK_BACKEND` at display-open time, so no re-exec is
  needed.
- The X display probe is deliberately conservative (abstract then filesystem
  Unix socket, local displays only): unconfirmed means stay on Wayland, because
  forcing X11 against a broken display could abort startup.
- `GDK_BACKEND` remains the single override in both directions; no
  Burnly-specific flag is added.
- Diagnostic event uses the existing `Runtime` area with code
  `platform.display_backend`; no new area or migration.
- Monitor precedence is implemented as a tiny `first_present` helper so the
  documented order is testable.

## Verification

- Command: `cargo test --manifest-path src-tauri/Cargo.toml --lib display_backend`
  - Outcome: 8 passed, 0 failed.
- Command: `cargo test --manifest-path src-tauri/Cargo.toml --lib platform::lifecycle`
  - Outcome: 7 passed, 0 failed.
- Command: `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
  - Outcome: passed.
- Command: `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
  - Outcome: passed without warnings.
- Command: `pnpm verify:fast`
  - Outcome: passed (format, lint with pre-existing warnings only, typecheck,
    sidecar prepare, rust check, all harness checks).
- Command: `pnpm verify`
  - Outcome: passed. Frontend 119 tests passed across 20 files; Rust 801
    passed, 0 failed, 3 ignored; harness checks passed.
- Command: `pnpm verify:runtime`
  - Outcome: passed. Desktop evidence recorded `sessionType=wayland`,
    `display=:0`, `waylandDisplay=wayland-0`; Tauri IPC bridge 8 passed;
    platform 21 passed; scheduler 3 passed.

## Runtime Evidence

- On the live GNOME Wayland session, an isolated debug build
  (`pnpm tauri dev` with separate `XDG_DATA_HOME`/`XDG_CACHE_HOME`/
  `XDG_CONFIG_HOME`) auto-selected the X11 backend: its panel window appeared in
  the X server window tree (`"Burnly" 440x540+2988+48`) while the installed
  native-Wayland AppImage had no X windows.
- Opening `Open Summary` through the tray D-Bus menu produced
  `Position: 2988,48` / `Geometry: 440x540` / `Map State: IsViewable` — the
  top-right corner of the 3440x1440 monitor (expected `3440-440-12=2988`, `48`),
  not the ~`1500,450` screen center. Repeated triggers toggled visibility and
  re-opened at the same position.
- Startup diagnostics recorded `platform.display_backend` with
  `{"backend":"x11","reason":"wayland_x11_selected"}`. An opt-out run with
  `GDK_BACKEND=wayland` started as a native Wayland client with no X windows and
  recorded `{"backend":"wayland","reason":"manual_env_value"}`.
- Both temporary instances were stopped afterwards; only the installed
  v0.1.34 AppImage remains running.
- Detail: `docs/runtime-evidence/2026-10-11-wayland-panel-positioning/README.md`.
- Limitations: no screenshot tooling on this machine (`import`, `xwd`,
  `gnome-screenshot`, `scrot` absent; XWayland region capture rendered black;
  GNOME Shell screenshot denied to non-shell callers), so rendered-panel
  confirmation stays manual; single-monitor session; GNOME only.

## Follow-Up Debt

- Rebuild/reinstall the Linux artifact (or ship it in the next release) so the
  operator can visually confirm the panel placement on the installed app; the
  currently installed v0.1.34 predates this change.
- Remove the XWayland selection when tauri issue #14913 exposes usable Wayland
  positioning, per `docs/engineering/known-limitations.md`.
