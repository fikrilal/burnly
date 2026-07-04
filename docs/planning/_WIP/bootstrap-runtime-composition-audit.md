# Bootstrap Runtime Composition Audit

## Status

Drafted on July 4, 2026.

This audit focuses on `src-tauri/src/bootstrap.rs`.

The goal is to reduce startup and runtime-composition risk without changing
Tauri behavior, app startup order, collector behavior, refresh policy,
persistence semantics, IPC contracts, or user-visible behavior.

This document is not an execution plan. It is an inspection and refactor
proposal that should be converted into small execution chunks before
implementation.

## Executive Summary

`bootstrap.rs` is now Burnly's largest production Rust file and the main
composition hotspot.

That is partly normal: a composition root is allowed to know about concrete
infrastructure and platform integrations. The risk is that the file now owns
several different kinds of work:

- Tauri builder and plugin setup
- process run-event handling
- startup database path, timezone, clock, migration, seed, and recovery
- SQLite store construction
- collector construction and routing
- diagnostic recorder/service wiring
- refresh coordinator and scheduler wiring
- tray controller, tray snapshot, and tray invalidation behavior
- tray-open freshness refresh behavior
- runtime settings and launch-at-login behavior
- updater and bootstrap service wiring
- packaged resource resolution for sidecars
- a broad Tauri IPC integration test harness

Recommended direction: keep `bootstrap.rs` as the composition root, but extract
runtime-owned modules around stable responsibilities. Do not move startup
behavior into the application layer and do not introduce a service locator or
dependency-injection framework.

The best refactor shape is a shallow `bootstrap/` module tree that keeps Tauri
and concrete adapter wiring at the application edge:

```text
src-tauri/src/bootstrap.rs              # public run/setup facade and StartupError
src-tauri/src/bootstrap/
  startup.rs                            # database initialization and recovery
  services.rs                           # store/service/collector composition
  runtime_events.rs                     # RunEvent, tray/menu/reopen/exit handling
  tray_runtime.rs                       # tray snapshot, invalidation, open-refresh
  settings_runtime.rs                   # DesktopSettingsRuntime and launch-at-login
  resources.rs                          # packaged resource and sidecar path resolution
  test_support.rs                       # optional test-only helpers/fakes
```

This is intentionally not a new architecture layer. It is only a split of the
composition root by runtime responsibility.

## Current File Map

Current hotspot:

```text
1643 src-tauri/src/bootstrap.rs
```

Important nearby modules:

```text
src-tauri/src/application/bootstrap.rs
src-tauri/src/application/refresh/
src-tauri/src/application/settings.rs
src-tauri/src/application/update.rs
src-tauri/src/application/usage/tray_summary.rs
src-tauri/src/infrastructure/collectors/
src-tauri/src/infrastructure/database/
src-tauri/src/ipc/
src-tauri/src/platform/lifecycle.rs
src-tauri/src/platform/tray.rs
src-tauri/src/platform/updater.rs
```

Current top-level production functions and types in `bootstrap.rs`:

```text
ExitGuard
StartupErrorKind
StartupError
run
handle_run_event
handle_tray_icon_event
setup_runtime
DesktopSettingsRuntime
should_reconcile_launch_at_login_on_startup
launch_at_login_supported
launch_at_login_capability
handle_menu_event
open_tray_panel
automatic_refresh_policy
build_tray_summary_query
build_refresh_coordinator
default_cline_data_dir
default_zcode_data_dir
default_home_data_dir
resolve_packaged_resource_directory
compose_refresh_coordinator
RuntimeRefreshEventSink
install_tray_invalidation_listener
tray_snapshot
TrayOpenRefreshController
initialize
recover_interrupted_runs
record_recovery_diagnostic
```

The module also contains broad tests for:

- startup database migration/seed/health failure behavior
- startup run recovery and recovery diagnostics
- packaged sidecar resource path resolution
- launch-at-login reconciliation policy
- tray-open refresh decision policy
- Tauri command bridge behavior
- composed refresh execution against a fake sidecar and real SQLite

## Current Responsibility Map

### Tauri App Builder

Owned today by:

- `run`

Responsibilities:

- install Tauri plugins
- install IPC invoke handler
- install window blur handler
- install single-instance plugin in release builds
- call setup hook
- run event loop

Assessment:

This should remain close to `bootstrap::run`. It is the public entry point and
the clearest place to see what Tauri capabilities the process loads. Do not
hide plugin installation behind a generic plugin registry.

Good extraction boundary:

- none initially, except moving event handling out of the file.

### Runtime Event Handling

Owned today by:

- `handle_run_event`
- `handle_tray_icon_event`
- `handle_menu_event`
- `open_tray_panel`
- `ExitGuard`

Responsibilities:

- resume-trigger refresh
- menu actions
- tray icon click behavior on Windows/macOS
- prevent process exit unless explicit quit was requested
- macOS Dock reopen behavior
- tray panel open behavior

Assessment:

This is cohesive platform runtime behavior. It is mixed with service
composition today, which makes startup review harder. It can move to a
`bootstrap/runtime_events.rs` module.

Important invariant:

- Exit prevention must keep using the explicit exit guard so closing windows or
  OS quit requests do not accidentally terminate the tray app.

### Startup Persistence And Recovery

Owned today by:

- `initialize`
- `recover_interrupted_runs`
- `record_recovery_diagnostic`
- parts of `setup_runtime`

Responsibilities:

- open the Burnly database
- create migration backup when needed
- migrate to latest schema
- verify database health
- seed app settings
- recover interrupted refresh/import runs
- record local diagnostic event when recovery changed state

Assessment:

This is one of the best extraction targets. It has stable behavior, strong
tests, and a clear output: initialized database plus recovered run state.

Good extraction boundary:

```rust
bootstrap::startup::{
    initialize_database,
    recover_interrupted_runs,
}
```

Do not move this into `infrastructure/database`. Startup policy decides when to
backup, migrate, seed, and record recovery diagnostics. The database adapter
should provide operations; bootstrap should orchestrate startup order.

Important invariant:

- Recovery must run after migration/health/seed and before refresh scheduler
  startup.

### Runtime Service Composition

Owned today by:

- most of `setup_runtime`
- `build_tray_summary_query`
- `build_refresh_coordinator`
- `compose_refresh_coordinator`
- collector construction inside `build_refresh_coordinator`

Responsibilities:

- construct stores from SQLite connections
- construct tray summary query
- construct collector graph
- construct refresh coordinator and scheduler
- construct bootstrap, settings, update, and diagnostics services
- manage services into Tauri state

Assessment:

This is legitimate composition-root work, but the current function is too dense.
It should be split by service family, not by implementation detail.

Good extraction boundary:

```rust
bootstrap::services::{
    build_tray_summary_query,
    build_refresh_coordinator,
    build_diagnostics_service,
    build_settings_service,
    build_bootstrap_service,
}
```

Do not introduce a container. Explicit construction is still the correct style.

Important invariant:

- All database-backed services should continue opening their own `Database`
  connection unless a store explicitly owns a shared transaction boundary.

### Collector Runtime Composition

Owned today by:

- `build_refresh_coordinator`
- `default_cline_data_dir`
- `default_zcode_data_dir`
- `default_home_data_dir`
- `resolve_packaged_resource_directory`

Responsibilities:

- resolve packaged ccusage sidecar resources
- allow development sidecar override through `BURNLY_CCUSAGE_DEV_BINARY`
- resolve native collector data roots
- wire diagnostic recorder into native collectors
- create `RoutedCollector`

Assessment:

Collector construction is now a distinct responsibility and should not stay
buried inside refresh coordinator construction. It can move to `services.rs` or
its own `collectors.rs` under bootstrap.

Good extraction boundary:

```rust
bootstrap::services::build_collector
bootstrap::resources::resolve_packaged_resource_directory
bootstrap::resources::default_source_data_dir
```

Important invariant:

- The AppImage `Burnly`/`burnly` resource fallback must remain covered by tests.
- Native collector diagnostic recorder wiring must remain local-only.

### Tray Runtime

Owned today by:

- tray install/update calls inside `setup_runtime`
- `RuntimeRefreshEventSink`
- `install_tray_invalidation_listener`
- `tray_snapshot`
- `tray_refresh_status`
- `TrayOpenRefreshController`
- `tray_open_refresh_decision`

Responsibilities:

- build native tray snapshot from refresh and usage state
- update tray on refresh events
- update tray on frontend data invalidation event
- request freshness refresh on tray open when stale and not throttled
- request startup refresh when stale

Assessment:

This is cohesive runtime behavior and should move together. It is not pure
platform code because it knows `RefreshCoordinator`, `TraySummaryQuery`, and
Burnly refresh policy. It belongs under `bootstrap/tray_runtime.rs`, not
`platform/tray.rs`.

Good extraction boundary:

```rust
bootstrap::tray_runtime::{
    install_tray_runtime,
    runtime_refresh_event_sink,
    tray_snapshot,
    TrayOpenRefreshController,
}
```

Important invariant:

- Tray-open refresh should continue using freshness refresh for manual tray
  opens and normal refresh for launch.

### Settings Runtime And Launch At Login

Owned today by:

- `DesktopSettingsRuntime`
- `should_reconcile_launch_at_login_on_startup`
- `launch_at_login_supported`
- `launch_at_login_capability`
- launch-at-login reconciliation call inside `setup_runtime`

Responsibilities:

- validate launch-at-login availability
- apply autostart enable/disable
- rollback autostart on settings persistence failure
- update in-memory runtime settings after commit
- reconcile persisted launch-at-login state on startup

Assessment:

This is a clear extraction target. It is platform runtime behavior implementing
an application settings port. It should move as a unit to
`bootstrap/settings_runtime.rs`.

Important invariant:

- Debug builds should continue reporting launch-at-login as unavailable.
- Startup reconciliation failures should remain non-fatal and diagnostic/log
  only unless product policy changes.

### IPC Bridge Tests

Owned today by:

- tests inside `bootstrap.rs`

Responsibilities:

- prove Tauri command wiring returns real response envelopes
- prove tray panel window can call bootstrap IPC
- prove update unavailable behavior
- prove settings update wiring
- prove refresh IPC state and composed refresh execution

Assessment:

These tests are valuable, but they inflate `bootstrap.rs` and make production
responsibilities harder to scan. After production extraction, test fakes can
move to `bootstrap/test_support.rs` under `#[cfg(test)]`, or tests can remain in
module-specific files if Rust visibility permits.

Do not delete these tests. They catch integration issues that unit tests in
application modules cannot see.

## Pressure Points

### Startup Order Is Implicit

`setup_runtime` encodes a critical sequence:

1. manage `ExitGuard`
2. set macOS activation policy
3. resolve database path, timezone, and clock
4. initialize database
5. recover interrupted runs
6. read settings
7. enforce project-path privacy policy
8. build tray summary, refresh event sink, coordinator, scheduler
9. install tray and prepare tray panel
10. manage services into Tauri state
11. request startup refresh if stale
12. reconcile launch-at-login

This ordering is correct but not named. Extraction should preserve the sequence
and make it more obvious, not hide it behind a generic setup abstraction.

### Tauri State Is Spread Across Setup

`setup_runtime` calls `app.manage` many times across unrelated concerns. That is
normal for Tauri, but review becomes hard because it is not obvious which
commands require which managed states.

Potential improvement:

- group `app.manage` calls by service family,
- or add tiny install functions that manage exactly the states they own.

Avoid:

- a global `RuntimeServices` bag managed into state, because IPC handlers expect
  concrete Tauri-managed state types.

### Startup Diagnostics Are Built By Hand

`record_recovery_diagnostic` manually constructs diagnostic codes, summaries,
contexts, and events. Collector diagnostics now have a safe support helper, but
startup diagnostics are still local.

This is acceptable for now. A future diagnostic helper should be cross-area only
if a second startup/runtime diagnostic path repeats the pattern.

### Resource Resolution Is Runtime-Packaging Specific

`resolve_packaged_resource_directory` exists because AppImage resource
directories can differ by product-name casing. This is small but easy to break
because it is packaging-specific.

It should move to a named runtime resource module with its existing tests.

### Tests Are High Value But Buried

The test suite in `bootstrap.rs` is not accidental noise. It proves startup and
IPC integration behavior across Tauri, SQLite, and refresh. The issue is
placement, not value.

Extraction should keep these tests close enough to private helpers without
weakening visibility.

## Recommended Refactor Plan

### Chunk 1: Startup Persistence Module

Scope:

- Create `src-tauri/src/bootstrap/startup.rs`.
- Move database initialization, interrupted run recovery, and recovery
  diagnostic recording.
- Keep `StartupError` in `bootstrap.rs` or a sibling `error.rs` until more
  chunks need it.
- Move startup persistence tests with the module if visibility stays clean.

Why first:

- Behavior is stable and well tested.
- It reduces startup risk without touching Tauri event handling.

Verification:

- `cargo test --manifest-path src-tauri/Cargo.toml bootstrap::`
- `pnpm rust:test`
- `pnpm verify:fast`

### Chunk 2: Resource And Collector Composition

Scope:

- Create `bootstrap/resources.rs` for packaged resource and source data-dir
  resolution.
- Move AppImage resource fallback tests with the module.
- Create a narrow collector builder function that returns `Arc<dyn Collector>`.
- Keep `RoutedCollector` in `infrastructure/collectors`.

Why second:

- Collector wiring is growing and will keep changing as sources are added.
- Resource resolution has packaging-specific tests that should be easy to find.

Verification:

- packaged resource tests
- routed collector tests
- `pnpm verify:fast`

### Chunk 3: Settings Runtime Module

Scope:

- Create `bootstrap/settings_runtime.rs`.
- Move `DesktopSettingsRuntime`, launch-at-login capability helpers, and startup
  reconciliation policy.
- Keep application settings service unchanged.

Why third:

- Launch-at-login has already produced cross-platform bugs and needs a clear
  owner.

Verification:

- launch-at-login reconciliation policy test
- settings IPC update test
- `pnpm verify:fast`

### Chunk 4: Tray Runtime Module

Scope:

- Create `bootstrap/tray_runtime.rs`.
- Move tray snapshot mapping, runtime refresh event sink, invalidation listener,
  tray-open refresh controller, and tray-open decision tests.
- Keep `platform/tray.rs` as native tray UI/control owner.

Why fourth:

- Tray runtime touches refresh, usage query, Tauri events, and platform tray.
  Moving it after simpler chunks lowers risk.

Verification:

- tray-open refresh decision test
- tray snapshot tests if added
- refresh event/invalidation smoke through existing tests
- `pnpm verify:fast`

### Chunk 5: Runtime Event Module

Scope:

- Create `bootstrap/runtime_events.rs`.
- Move run-event, menu-event, tray-icon-event, reopen, quit, and panel-open
  behavior.
- Keep `ExitGuard` either in this module or in `bootstrap.rs` if setup and
  events both need it.

Why fifth:

- It is Tauri-specific and behavior-sensitive, but mostly independent after tray
  runtime extraction.

Verification:

- existing tray/lifecycle tests
- manual runtime evidence only if event behavior changes
- `pnpm verify:fast`

### Chunk 6: Setup Composition Cleanup

Scope:

- Rename the remaining `setup_runtime` steps into a readable sequence of
  install/build functions.
- Consider moving `StartupError` to `bootstrap/error.rs` only if it improves
  readability after other chunks.
- Move reusable test fakes/helpers under `bootstrap/test_support.rs` if they
  still dominate the file.

Why last:

- Cleanup should happen after the stable responsibilities have owners.

Verification:

- full `pnpm verify`
- desktop runtime evidence only if startup/tray behavior changes.

## Non-Goals

- Do not change startup order.
- Do not change refresh policy or tray-open refresh thresholds.
- Do not move Tauri-specific behavior into `application`.
- Do not add a dependency injection container or runtime service registry.
- Do not change IPC command contracts.
- Do not change collector routing or source support.
- Do not change database schema, migration policy, or startup seed behavior.
- Do not remove existing bootstrap IPC integration tests.

## Non-Negotiable Invariants

- `bootstrap.rs` remains the only public Tauri composition entry point.
- Application and domain modules stay independent from Tauri.
- React continues to reach Rust only through IPC.
- Startup recovery terminalizes interrupted runs before any new refresh starts.
- Project-path privacy policy enforcement remains part of startup before the app
  becomes interactive.
- Launch-at-login remains unavailable in debug builds unless that product policy
  explicitly changes.
- AppImage resource fallback remains tested.
- Tray-open refresh remains throttled and does not submit when refresh is
  already active.
- Tauri-managed state remains concrete enough for IPC handlers to request by
  type.

## Verification Strategy

Minimum checks per chunk:

- focused Rust tests for moved behavior,
- `cargo test --manifest-path src-tauri/Cargo.toml bootstrap::`,
- `pnpm rust:test`,
- `pnpm verify:fast`.

Run full `pnpm verify` after all chunks complete.

Runtime evidence is required if a chunk changes any of:

- Tauri app builder/plugin installation,
- run-event handling,
- tray open/close behavior,
- launch-at-login behavior,
- update runtime wiring,
- packaged resource lookup.

## Open Questions

- Should `StartupError` move first?
  - Recommendation: no. Keep error movement opportunistic. It is not the
    readability bottleneck by itself.
- Should tray runtime move to `platform/`?
  - Recommendation: no. `platform/tray.rs` should own native tray mechanics.
    Bootstrap tray runtime owns Burnly-specific refresh and summary behavior.
- Should collector construction move into `infrastructure/collectors`?
  - Recommendation: no. Source adapters live there, but selecting concrete
    collectors, env overrides, diagnostics, and packaged resources is
    composition-root work.
- Should the Tauri IPC integration tests move out of bootstrap?
  - Recommendation: only after production extraction. Keeping tests near private
    helpers may be simpler until chunk 6.

## Success Criteria

- `bootstrap.rs` becomes a readable composition facade instead of a 1.6k-line
  runtime catch-all.
- Startup order is easier to review.
- Platform/runtime behavior has named owning modules.
- Adding a new collector no longer requires editing a dense refresh-coordinator
  builder buried in startup.
- Launch-at-login, tray-open refresh, and packaged-resource behavior remain
  covered by focused tests.
- No product behavior changes.
