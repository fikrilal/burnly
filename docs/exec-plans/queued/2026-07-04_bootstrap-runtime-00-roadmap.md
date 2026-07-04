# 2026-07-04 Bootstrap Runtime Composition Roadmap

## Objective

Coordinate the bootstrap/runtime composition cleanup described in
`docs/planning/_WIP/bootstrap-runtime-composition-audit.md` without changing
Tauri behavior, app startup order, collector behavior, refresh policy,
persistence semantics, IPC contracts, or user-visible behavior.

## Acceptance Criteria

- `bootstrap.rs` remains the public Tauri composition entry point.
- Application and domain modules remain independent from Tauri.
- Startup order is preserved.
- Startup recovery still terminalizes interrupted runs before any new refresh
  starts.
- Project-path privacy policy enforcement remains part of startup before the app
  becomes interactive.
- Launch-at-login behavior remains unchanged, including unavailable debug-build
  behavior.
- Tray-open refresh throttling and active-refresh skipping remain unchanged.
- AppImage packaged-resource fallback remains tested.
- Existing bootstrap IPC integration tests remain in place or move with equal
  coverage.
- No dependency-injection container, service locator, plugin registry, or
  product behavior change is introduced.
- Each chunk records verification before completion.

## Risk Class

`medium`

## Impact Areas

- `src-tauri/src/bootstrap.rs`
- `src-tauri/src/bootstrap/`
- `src-tauri/src/platform/lifecycle.rs`
- `src-tauri/src/platform/tray.rs`
- `src-tauri/src/platform/updater.rs`
- `src-tauri/src/infrastructure/database/`
- `src-tauri/src/infrastructure/collectors/`
- Bootstrap/runtime tests

## Design Review

- What complexity is being introduced?
  - A shallow `bootstrap/` module tree organized by runtime composition
    responsibility.
- Which decisions are hidden inside the owning module?
  - Startup persistence, resource resolution, runtime service construction,
    settings runtime behavior, tray runtime behavior, and Tauri run-event
    handling.
- Is each new interface simpler than its implementation?
  - Yes if each module exposes narrow install/build functions instead of a
    runtime service bag.
- What special cases exist, and can the design eliminate them?
  - AppImage resource casing fallback, debug-build launch-at-login unavailable
    policy, tray-open freshness refresh, and startup recovery diagnostics must
    remain explicit.
- Why is each new abstraction needed now?
  - `bootstrap.rs` is Burnly's largest production Rust file and is now a merge
    and review hotspot for every runtime feature.
- Can an existing module absorb this responsibility cleanly?
  - No. These are composition-root concerns. Moving them into application,
    infrastructure, or platform would blur ownership.

## Checklist

- [x] Complete chunk 01: startup persistence module.
- [x] Complete chunk 02: resource and collector composition.
- [ ] Complete chunk 03: settings runtime module.
- [ ] Complete chunk 04: tray runtime module.
- [ ] Complete chunk 05: runtime event module.
- [ ] Complete chunk 06: setup composition cleanup.
- [ ] Re-run the full local gate after all chunks are complete.
- [ ] Update `docs/planning/_WIP/bootstrap-runtime-composition-audit.md` with
      important implementation decisions or deviations.

## Test Plan

- Behavior and invariants to prove:
  - Startup database initialization and recovery behavior is unchanged.
  - Startup privacy enforcement still runs before app interactivity.
  - Tauri commands still receive the same managed state types.
  - Tray-open refresh decisions are unchanged.
  - Launch-at-login startup reconciliation behavior is unchanged.
  - Packaged resource fallback is unchanged.
  - IPC bridge integration tests still pass.
- Lowest stable test layer:
  - Focused Rust unit tests for moved helpers.
  - Bootstrap Tauri IPC integration tests.
- Failure paths:
  - database path resolution failure
  - migration/health/seed failure
  - interrupted run recovery
  - resource directory lookup failure
  - collector construction failure
  - launch-at-login apply failure
  - tray install/panel failure
- Fixtures or fakes:
  - Existing SQLite startup fixtures.
  - Existing fake ccusage sidecar.
  - Existing Tauri test harness.
- Runtime or platform evidence:
  - Required if a chunk changes app builder/plugin installation, run-event
    handling, tray open/close behavior, launch-at-login behavior, update runtime
    wiring, or packaged resource lookup.
- Relevant commands:
  - `cargo test --manifest-path src-tauri/Cargo.toml bootstrap::`
  - `pnpm rust:test`
  - `pnpm architecture:check`
  - `pnpm verify:fast`
  - `pnpm verify`

## Decisions

- Keep `bootstrap.rs` as the public composition facade.
- Split by runtime responsibility, not by dependency type.
- Do not introduce a dependency injection container or runtime service registry.
- Keep Tauri-specific behavior out of application/domain modules.

## Verification

- Command: not run yet
- Outcome: queued plan only

## Runtime Evidence

- Not required yet.

## Follow-Up Debt

- Revisit stale active/queued execution-plan files separately; do not mix plan
  hygiene into this refactor.
