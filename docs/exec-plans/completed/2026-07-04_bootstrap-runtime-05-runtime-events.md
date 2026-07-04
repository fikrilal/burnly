# 2026-07-04 Bootstrap Runtime 05 Runtime Events

## Objective

Move Tauri run-event, tray-icon-event, menu-event, reopen, quit, and tray panel
open behavior from `bootstrap.rs` into a focused runtime events module without
changing desktop lifecycle behavior.

## Acceptance Criteria

- `src-tauri/src/bootstrap/runtime_events.rs` owns runtime event handling.
- `ExitGuard` remains available to setup and exit-request handling.
- Resume events still request resume refresh.
- Menu refresh still requests manual refresh.
- Quit still sets explicit exit before exiting.
- Exit requests remain prevented unless explicit quit was requested.
- macOS reopen still opens the tray panel.
- Windows/macOS tray icon click behavior remains unchanged.

## Risk Class

`medium`

## Impact Areas

- `src-tauri/src/bootstrap.rs`
- `src-tauri/src/bootstrap/runtime_events.rs`
- `src-tauri/src/bootstrap/tray_runtime.rs`
- `src-tauri/src/platform/lifecycle.rs`
- `src-tauri/src/platform/tray.rs`

## Design Review

- What complexity is being introduced?
  - One module for Tauri event handling at the composition edge.
- Which decisions are hidden inside the owning module?
  - How OS/runtime events map to Burnly refresh, panel, and exit actions.
- Is each new interface simpler than its implementation?
  - Yes if `run` delegates to one `handle_run_event` function and setup manages
    an explicit exit guard.
- What special cases exist, and can the design eliminate them?
  - macOS reopen and Windows/macOS tray icon click handling are platform-gated.
    Preserve those cfgs.
- Why is each new abstraction needed now?
  - Runtime event handling is unrelated to service construction and makes
    bootstrap harder to scan.
- Can an existing module absorb this responsibility cleanly?
  - No. Platform lifecycle owns window operations; bootstrap owns mapping Tauri
    events to application runtime actions.

## Checklist

- [x] Create `src-tauri/src/bootstrap/runtime_events.rs`.
- [x] Move `ExitGuard` if visibility remains clean.
- [x] Move run-event handler.
- [x] Move tray icon click handler.
- [x] Move menu event handler.
- [x] Move tray panel open helper or delegate to tray runtime.
- [x] Preserve platform cfg gates.
- [x] Run bootstrap/lifecycle tests and fast verification.
- [x] Record verification outcomes before completion.

## Test Plan

- Behavior and invariants to prove:
  - Resume requests refresh.
  - Exit is prevented unless explicit quit was requested.
  - Menu refresh and quit actions still map to expected runtime actions.
  - Tray panel open still goes through lifecycle helpers.
- Lowest stable test layer:
  - Existing platform lifecycle tests.
  - Add small unit tests only if behavior can be tested without brittle Tauri
    event construction.
- Failure paths:
  - missing coordinator state on menu/resume is harmless
  - tray panel open errors remain ignored at event boundary
- Fixtures or fakes:
  - Existing lifecycle tests and Tauri mocks if needed.
- Runtime or platform evidence:
  - Required if event behavior changes beyond moving code.
- Relevant commands:
  - `cargo test --manifest-path src-tauri/Cargo.toml platform::lifecycle::`
  - `cargo test --manifest-path src-tauri/Cargo.toml bootstrap::`
  - `pnpm verify:fast`

## Decisions

- Keep event-to-action mapping at the Tauri composition edge.
- Do not move application behavior into platform lifecycle helpers.

## Verification

- Command: `cargo test --manifest-path src-tauri/Cargo.toml platform::lifecycle::`
- Outcome: passed; 6 passed, 0 failed.
- Command: `cargo test --manifest-path src-tauri/Cargo.toml bootstrap::`
- Outcome: passed; 23 passed, 0 failed.
- Command: `pnpm verify:fast`
- Outcome: passed; existing ESLint warnings and duplication report remain
  non-fatal.

## Runtime Evidence

- Not required unless event behavior changes.

## Follow-Up Debt

- None.
