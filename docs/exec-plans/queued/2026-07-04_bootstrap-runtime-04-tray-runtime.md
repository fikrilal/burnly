# 2026-07-04 Bootstrap Runtime 04 Tray Runtime

## Objective

Move tray summary snapshot mapping, tray invalidation wiring, refresh event tray
updates, and tray-open freshness refresh behavior from `bootstrap.rs` into a
focused bootstrap tray runtime module.

## Acceptance Criteria

- `src-tauri/src/bootstrap/tray_runtime.rs` owns tray snapshot mapping and
  tray-open refresh behavior.
- `platform/tray.rs` remains the native tray UI/control owner.
- Refresh event sink still updates both frontend events and the native tray.
- Data invalidation listener still refreshes tray snapshot.
- Tray-open refresh decisions remain unchanged.
- Startup refresh-if-stale behavior remains unchanged.

## Risk Class

`medium`

## Impact Areas

- `src-tauri/src/bootstrap.rs`
- `src-tauri/src/bootstrap/tray_runtime.rs`
- `src-tauri/src/platform/tray.rs`
- `src-tauri/src/platform/lifecycle.rs`
- refresh event sink wiring
- tray-open refresh tests

## Design Review

- What complexity is being introduced?
  - A bootstrap-owned module for Burnly-specific tray runtime behavior.
- Which decisions are hidden inside the owning module?
  - How refresh state and tray summary are mapped to native tray state, and when
    tray opens request freshness refresh.
- Is each new interface simpler than its implementation?
  - Yes if setup calls a small install/build function and manages the returned
    controller.
- What special cases exist, and can the design eliminate them?
  - Tray-open manual refresh uses freshness refresh while launch uses normal
    refresh. Preserve this explicitly.
- Why is each new abstraction needed now?
  - Tray runtime currently mixes application refresh state, usage read models,
    Tauri events, and platform tray mechanics inside bootstrap.
- Can an existing module absorb this responsibility cleanly?
  - No. `platform/tray.rs` should not know Burnly refresh or usage query rules.

## Checklist

- [ ] Create `src-tauri/src/bootstrap/tray_runtime.rs`.
- [ ] Move `RuntimeRefreshEventSink`.
- [ ] Move runtime refresh event sink constructor.
- [ ] Move tray invalidation listener.
- [ ] Move tray snapshot mapping helpers.
- [ ] Move `TrayOpenRefreshController` and decision helper.
- [ ] Move tray-open decision tests.
- [ ] Update `setup_runtime` and `open_tray_panel` call sites.
- [ ] Run focused tray/bootstrap tests and fast verification.
- [ ] Record verification outcomes before completion.

## Test Plan

- Behavior and invariants to prove:
  - Tray snapshot maps refresh status and period totals unchanged.
  - Tray-open refresh requests only when stale, inactive, and not throttled.
  - Startup refresh uses launch trigger.
  - Manual tray-open refresh uses freshness refresh.
  - Data invalidation listener still updates tray state.
- Lowest stable test layer:
  - Tray runtime unit tests.
  - Existing refresh/tray integration tests where available.
- Failure paths:
  - clock read failure skips tray-open refresh
  - tray summary read failure skips tray-open refresh
  - active refresh skips tray-open refresh
- Fixtures or fakes:
  - Existing fake clock/query/coordinator where available or small fakes at the
    boundary.
- Runtime or platform evidence:
  - Required only if tray event behavior changes.
- Relevant commands:
  - `cargo test --manifest-path src-tauri/Cargo.toml bootstrap::`
  - `cargo test --manifest-path src-tauri/Cargo.toml platform::tray::`
  - `pnpm verify:fast`

## Decisions

- Keep native tray mechanics in `platform/tray.rs`.
- Keep Burnly refresh/summary tray behavior in bootstrap runtime.

## Verification

- Command: not run yet
- Outcome: queued plan only

## Runtime Evidence

- Not required unless tray behavior changes.

## Follow-Up Debt

- None.
