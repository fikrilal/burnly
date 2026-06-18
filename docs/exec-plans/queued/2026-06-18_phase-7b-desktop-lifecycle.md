# 2026-06-18 Phase 7B Desktop Lifecycle

## Objective

Implement desktop lifecycle behavior so Burnly handles close, quit, focus,
resume, and second-launch activation predictably across supported platforms.

## Acceptance Criteria

- Close behavior follows persisted setting: `quit` exits, `hide` keeps the app
  running.
- The app can reopen/focus the existing main window after it was hidden.
- A second app launch focuses the existing instance instead of creating a second
  active app.
- Wake/resume can trigger a refresh request without bypassing coordinator
  coalescing.
- Lifecycle behavior is owned by Rust/platform code, not React feature code.
- User-facing behavior is covered by tests where possible and by runtime
  evidence where OS behavior cannot be fully mocked.

## Risk Class

`high`

Desktop lifecycle behavior is OS-sensitive. Incorrect handling can leave hidden
processes running, lose user intent to quit, or create duplicate refresh loops.

## Impact Areas

- `src-tauri/src/platform/`
- `src-tauri/src/bootstrap.rs`
- `src-tauri/src/ipc/app.rs` or app command modules
- Tauri app setup and window event handlers
- Settings store for `close_behavior`
- Runtime evidence scripts and smoke checklist

## Design Review

- What complexity is being introduced? Window lifecycle and app-instance
  coordination across platform-specific Tauri events.
- Which decisions are hidden inside the owning module? How Tauri close events,
  focus requests, and second-instance events map to Burnly lifecycle actions.
- Is each new interface simpler than its implementation? Commands should expose
  simple actions such as open/focus/quit; event plumbing stays inside platform
  composition.
- What special cases exist, and can the design eliminate them? Explicit quit vs
  window close, hidden main window, app already refreshing, second launch before
  bootstrap completes, resume events firing repeatedly. Central lifecycle
  ownership should prevent scattered event handling.
- Why is this abstraction needed now? Tray behavior depends on a reliable hidden
  window and explicit quit model.
- Can existing modules absorb this responsibility cleanly? Platform/bootstrap
  modules should absorb it; application refresh only receives triggers.

## Checklist

- [ ] Inspect current Tauri setup, window labels, close behavior, and settings
      access.
- [ ] Define a small lifecycle service or handler boundary in Rust.
- [ ] Implement close-to-hide vs quit behavior from persisted settings.
- [ ] Implement open/focus main window action for hidden/minimized state.
- [ ] Implement single-instance activation behavior.
- [ ] Add wake/resume refresh trigger if Tauri/plugin support is available and
      testable.
- [ ] Add bootstrap or platform tests for lifecycle decision mapping.
- [ ] Add runtime smoke evidence for close, reopen, quit, and second launch.

## Test Plan

- Behavior and invariants to prove: close with `hide` does not quit; close with
  `quit` exits; focus action is idempotent; second-instance event maps to focus;
  resume trigger submits through coordinator.
- Lowest stable test layer: Rust unit tests for lifecycle decision mapping;
  Tauri bridge tests where event simulation is stable.
- Failure paths: missing main window, settings read failure, focus failure,
  duplicate activation during startup.
- Fixtures or fakes: fake window/app handle where possible; explicit runtime
  evidence where Tauri cannot be cleanly faked.
- Runtime or platform evidence: required. Record platform, window manager, and
  command/checklist result.
- Relevant commands: `cargo test --manifest-path src-tauri/Cargo.toml`,
  `pnpm verify`, `pnpm verify:runtime`.

## Decisions

- Treat explicit quit as different from close-to-hide. Do not infer user intent
  from a close event without consulting settings.
- Keep lifecycle state in Rust. React should not decide whether the process
  keeps running.
- Wake/resume refresh may be deferred if the platform event source is not stable
  without another dependency.

## Verification

- Command: `pnpm verify`
- Outcome: not run yet

## Runtime Evidence

- Required after implementation because this changes OS/window behavior.

## Follow-Up Debt

- Cross-platform lifecycle differences should be revisited during Phase 10
  release hardening.
