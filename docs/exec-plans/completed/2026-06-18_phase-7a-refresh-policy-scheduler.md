# 2026-06-18 Phase 7A Refresh Policy And Scheduler

## Objective

Implement settings-backed refresh policy and scheduling so Burnly can refresh in
the background without creating competing jobs or requiring the main window.

## Acceptance Criteria

- Background refresh can be enabled or disabled from persisted settings.
- Refresh interval uses the persisted `refresh_interval_minutes` setting.
- Manual refresh remains higher priority than scheduled refresh.
- Concurrent scheduled/manual requests coalesce through the existing refresh
  coordinator instead of creating competing jobs.
- Scheduler starts at app startup and shuts down cleanly with the app.
- Scheduler does not call Tauri APIs from React feature code.

## Risk Class

`high`

Background scheduling can silently create duplicate imports, battery drain,
unexpected process execution, or stale UI state if ownership is unclear.

## Impact Areas

- `src-tauri/src/application/refresh/`
- `src-tauri/src/application/settings` or existing settings port modules
- `src-tauri/src/bootstrap.rs`
- `src-tauri/src/ipc/refresh.rs`
- `src-tauri/src/ipc/settings.rs` if settings update events need refresh policy
  invalidation
- `src/app/App.tsx` only if UI refresh state needs a small wiring change
- Runtime evidence scripts if background refresh requires desktop proof

## Design Review

- What complexity is being introduced? A long-lived scheduler that reacts to
  settings, app startup, and refresh state.
- Which decisions are hidden inside the owning module? Timer cadence, coalescing
  behavior, and scheduled-trigger submission stay in the Rust refresh lifecycle
  owner.
- Is each new interface simpler than its implementation? The app should start
  one refresh lifecycle service and expose only start/stop or policy-update
  entrypoints; timer details stay private.
- What special cases exist, and can the design eliminate them? Disabled
  background refresh, interval changes while running, manual refresh while a
  scheduled refresh is pending, shutdown during an active timer, and failed
  scheduled refresh. Coalescing should be delegated to the existing coordinator.
- Why is this abstraction needed now? Tray and lifecycle behavior need a backend
  refresh service that does not depend on a visible React window.
- Can existing modules absorb this responsibility cleanly? The refresh
  application module can own the policy; bootstrap can compose it.

## Checklist

- [x] Inspect existing settings storage/update flow and refresh coordinator
      trigger semantics.
- [x] Define a small refresh policy model from persisted settings.
- [x] Add a scheduler/service owned by Rust bootstrap, not React.
- [x] Wire app startup to initialize scheduler from settings.
- [x] Wire settings updates to apply policy changes without restarting.
- [x] Ensure scheduled refresh requests use `RefreshTrigger::Scheduled`.
- [x] Ensure manual refresh remains responsive while scheduler is active.
- [x] Add deterministic tests using fake clock/timer boundaries where feasible.
- [x] Add integration coverage for coalescing scheduled and manual refresh.
- [x] Record runtime evidence requirements for background behavior.

## Test Plan

- Behavior and invariants to prove: disabled scheduler does nothing; enabled
  scheduler submits scheduled refresh; interval changes replace the old cadence;
  manual and scheduled refreshes coalesce through the coordinator; failed
  scheduled refresh does not stop future scheduled attempts unless policy is
  disabled.
- Lowest stable test layer: Rust application tests with fake scheduler clock or
  deterministic timer abstraction; bootstrap integration test for composition.
- Failure paths: invalid settings interval, scheduler shutdown during pending
  tick, refresh coordinator failure, settings update failure.
- Fixtures or fakes: fake refresh requester, fake settings store, deterministic
  timer.
- Runtime or platform evidence: `pnpm verify:runtime` after implementation if
  scheduler starts in desktop bootstrap.
- Relevant commands: `cargo test --manifest-path src-tauri/Cargo.toml`,
  `pnpm verify`, `pnpm verify:runtime`.

## Decisions

- Scheduled refresh policy belongs in Rust. React may display or edit settings,
  but must not own background timers.
- Use the existing refresh coordinator for coalescing instead of adding a second
  job queue.
- Do not add file-watch refresh in 7A; defer it unless a real source path change
  signal is available and testable.

## Verification

- Command: `pnpm verify`
- Outcome: passed on 2026-06-18. `eslint` reported existing warning-only
  complexity/size signals; the command completed successfully.
- Additional focused commands:
  - `cargo test --manifest-path src-tauri/Cargo.toml application::refresh::scheduler`
  - `cargo test --manifest-path src-tauri/Cargo.toml concurrent_requests_coalesce_into_one_run`
  - `cargo test --manifest-path src-tauri/Cargo.toml tauri_bridge_updates_settings_when_scheduler_state_is_available`

## Runtime Evidence

- Command: `pnpm verify:runtime`
- Outcome: passed on 2026-06-18.
- Evidence environment: Ubuntu 24.04 x86_64 on X11, Tauri 2.11.2, WebKitGTK
  2.52.3, Rust 1.95.0, Node 22.22.0, pnpm 10.33.1.
- Runtime evidence covered Tauri prerequisites, generated contracts, frontend
  build, Tauri IPC bridge tests, and desktop UI evidence.
- Phase 7D expands `pnpm verify:runtime` with focused scheduler evidence and
  keeps native smoke limitations in `docs/engineering/desktop-runtime-evidence.md`.

## Follow-Up Debt

- File-watch debounce remains deferred until source discovery/path ownership is
  designed.
