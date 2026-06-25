# 2026-06-25 Tray Auto-Refresh And Compact Window

## Objective

Implement tray/menu-bar behavior for opening a compact tray panel and refreshing
automatically without exposing a primary refresh button.

This chunk should wire platform behavior. It should not implement final visual
polish beyond the minimal shell needed for validation.

## Acceptance Criteria

- Tray/menu-bar interaction opens a compact Burnly panel instead of only opening
  the full desktop window.
- Opening the tray panel requests refresh only when data is stale.
- Tray-open refresh is throttled.
- Refresh requests never overlap; existing refresh coordinator coalescing is
  preserved.
- Existing background scheduler remains the owner of interval refresh.
- Manual refresh is not a primary tray-panel action.
- `Open details` opens/focuses the full desktop window on Summary.

## Risk Class

`high`

Tray/window behavior varies by platform and desktop environment. Incorrect
behavior can make the primary product surface inaccessible.

## Impact Areas

- Tauri tray/platform code
- Window lifecycle
- Refresh scheduler/coordinator triggers
- IPC/bootstrap capabilities
- React routing or window entry points
- Linux runtime evidence

## Design Review

- Complexity introduced: a dedicated compact window or panel lifecycle.
- Owning module: platform layer owns window/tray mechanics; refresh coordinator
  owns refresh concurrency.
- Interface depth: tray open should call one platform/application operation,
  hiding stale-throttle and window focus details from UI.
- Special cases: tray unavailable, window already open, app hidden to tray,
  refresh already running, stale threshold not reached.
- New abstraction needed now: tray panel window controller if Tauri window
  mechanics cannot be cleanly contained in current tray controller.

## Checklist

- [x] Decide compact panel implementation as dedicated Tauri window vs route in
      existing main window.
- [x] Add platform lifecycle for compact tray panel.
- [x] Add stale-data threshold policy for tray-open refresh.
- [x] Trigger refresh on app start if needed.
- [x] Preserve scheduled background refresh.
- [x] Remove/de-emphasize native tray menu refresh as primary action.
- [x] Add `Open details` behavior landing on Summary.
- [x] Add platform/unit tests for tray actions and refresh throttling.

## Test Plan

- Behavior and invariants to prove:
  - tray panel opens from tray action,
  - repeated opens do not spam refresh,
  - active refresh is reused,
  - full window open lands on Summary,
  - close/hide behavior remains correct.
- Lowest stable test layer:
  - Rust platform unit tests,
  - refresh scheduler/coordinator tests,
  - IPC/bootstrap capability tests where needed.
- Failure paths:
  - tray unsupported,
  - compact window creation failure,
  - refresh request failure,
  - no successful refresh timestamp yet.
- Fixtures or fakes:
  - fake clock,
  - fake refresh requester/coordinator,
  - fake window/tray abstractions where possible.
- Runtime or platform evidence:
  - Linux installed-app tray interaction evidence comes in the final runtime
    chunk.
- Relevant commands:
  - `pnpm rust:test`
  - `pnpm architecture:check`
  - `pnpm platform-behavior:check`

## Decisions

- Auto-refresh is primary.
- Manual refresh is secondary recovery/debug behavior.
- The compact panel should not be blocked by full desktop redesign.
- Use a dedicated `tray-panel` Tauri window that loads `index.html#/tray`.
- Keep detailed tray UI for the next chunk; this chunk only adds a minimal
  placeholder shell to validate window routing.
- Use the existing refresh trigger enum for now. The current SQLite schema
  constrains trigger values, so exact `tray_open` telemetry requires a later
  migration.

## Verification

- Command: `pnpm contracts:generate`
  - Outcome: passed.
- Command: `cargo fmt --manifest-path src-tauri/Cargo.toml --check`
  - Outcome: passed.
- Command: `pnpm vitest run src/app/App.test.tsx`
  - Outcome: passed; 6 tests passed.
- Command: `cargo check --manifest-path src-tauri/Cargo.toml`
  - Outcome: passed after fixing event payload clone and coordinator ownership.
- Command: `cargo test --manifest-path src-tauri/Cargo.toml tray_open --lib`
  - Outcome: passed; 1 test passed.
- Command: `pnpm contracts:check`
  - Outcome: passed.
- Command: `pnpm typecheck`
  - Outcome: passed.
- Command: `pnpm security:check`
  - Outcome: passed.
- Command: `pnpm platform-behavior:check`
  - Outcome: passed.
- Command: `pnpm verify:fast`
  - Outcome: passed. ESLint reported warning-only existing/new App size and
    complexity warnings; no errors.
- Command: `pnpm verify`
  - Outcome: not run yet.

## Runtime Evidence

- Not collected yet.

## Follow-Up Debt

- Validate equivalent tray behavior on Windows and macOS later.
