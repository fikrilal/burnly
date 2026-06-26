# 2026-06-26 Strip 07 — Remove The Main Window

Part of phase `2026-06-26_strip-to-tray-only`. Active. Coordinates with strip-to-tray-only.

## Objective

Remove the native `main` desktop window and all `Open details` plumbing. Only
the `tray-panel` window remains.

## Acceptance Criteria

- Removed from `platform/lifecycle.rs`: `MAIN_WINDOW_LABEL`,
  `ensure_main_window`, `open_details_window`, `activate_main_window`,
  `OpenDetailsEvent`, and the `OPEN_DETAILS_EVENT` constant.
- `WindowActions` trait/port loses `open_details`; `DesktopWindowActions` keeps
  only `hide_tray_panel` (or the trait is simplified accordingly).
- `handle_close_request` no longer special-cases a main window.
- Tray "Open details" menu item/button removed from `platform/tray.rs` and the
  tray panel UI (`OpenDetailsButton`).
- Tauri config has no `main` window definition; tray-panel window unchanged.
- Gate passes: `cargo test`, `pnpm architecture:check`.

## Risk Class

`medium`

Window lifecycle is platform-specific; verify the tray panel still opens,
positions, focuses, and blurs-to-hide.

## Impact Areas

- `src-tauri/src/platform/lifecycle.rs`
- `src-tauri/src/platform/tray.rs`
- `src-tauri/src/application/ports/window_actions.rs`
- `src-tauri/src/ipc/commands.rs` (any residual open-details types)
- `src/components/burnly/status.tsx` (`OpenDetailsButton`) and tray panel usage
- `src-tauri/tauri.conf.json` (and platform conf files) window config

## Design Review

- Removal simplifies the window model to a single tray-panel window.
- Confirm no startup code expects a `main` window to exist.
- The `WindowActions` port shrinks; if only `hide_tray_panel` remains, keep the
  port rather than inlining, to preserve the React-away-from-Tauri boundary.

## Checklist

- [x] Remove main-window functions/constants from `lifecycle.rs`.
- [x] Simplify `WindowActions` and `DesktopWindowActions`.
- [x] Remove the tray "Open details" action from `tray.rs`.
- [x] Remove `OpenDetailsButton` usage from the tray panel and the component if
      unused elsewhere.
- [x] Remove the `main` window from Tauri config.
- [x] Update lifecycle/tray tests.
- [x] Run the gate.

## Test Plan

- Behavior and invariants to prove: tray panel opens/toggles/positions/focuses;
  blur hides it; close behavior no longer references a main window.
- Lowest stable test layer: platform lifecycle/tray unit tests.
- Failure paths: opening the tray panel when missing still creates it.
- Fixtures or fakes: Tauri mock app in existing lifecycle tests.
- Runtime or platform evidence: covered by chunk 8 runtime gate.
- Relevant commands: `cargo test`, `pnpm architecture:check`.

## Decisions

- The tray-panel window is the only window. No `Open details`.

## Verification

- Command: `cargo test`
- Outcome: passed cleanly (200 tests passed).
- Command: `pnpm verify:fast`
- Outcome: passed cleanly.

## Runtime Evidence

- Deferred to chunk 8.

## Follow-Up Debt

- Tray tab navigation (settings/sessions) is a future plan, not part of this
  removal.
