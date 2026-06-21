# 2026-06-18 Phase 8A Settings Foundation

## Objective

Replace bootstrap-coupled settings mutation with dedicated typed settings use
cases that validate, persist, and apply backend behavior consistently.

## Acceptance Criteria

- `settings_get` returns the durable settings model and revision.
- `settings_update` validates one complete settings replacement and requires the
  expected revision.
- Conflicting updates fail without overwriting newer settings.
- Reporting timezone, refresh policy, close behavior, and supported platform
  settings are applied to runtime owners after persistence.
- Unsupported platform behavior returns an explicit capability-aware result.
- React settings state is fetched and mutated through `src/ipc/` and TanStack
  Query.
- Existing settings survive restart.

## Risk Class

`medium`

Settings already exist, but current bootstrap ownership can create stale runtime
state and lost updates as more behaviors depend on them.

## Impact Areas

- `src-tauri/src/domain/settings/`
- `src-tauri/src/application/settings/`
- `src-tauri/src/application/ports/settings_store.rs`
- `src-tauri/src/infrastructure/database/`
- `src-tauri/src/ipc/`
- `src/features/settings/`
- Generated IPC contracts and contract tests

## Design Review

- What complexity is being introduced? Typed validation, optimistic concurrency,
  and runtime application of durable settings.
- Which decisions are hidden inside the owning module? Settings owns allowed
  values and revision semantics; runtime owners apply only their relevant
  values.
- Is each new interface simpler than its implementation? Callers get and replace
  one settings document without accessing bootstrap storage or SQLite.
- What special cases exist, and can the design eliminate them? Platform
  capability differences remain explicit results; partial patch semantics are
  avoided by replacing the complete typed settings document.
- Why is each new abstraction needed now? Privacy, budgets, notifications, and
  refresh behavior all require a reliable settings boundary.
- Can an existing module absorb this responsibility cleanly? Extract settings
  from bootstrap rather than expanding `BootstrapService`.

## Checklist

- [x] Audit current settings persistence, IPC, UI, and runtime consumers.
- [x] Define settings domain values, validation errors, revision, and read model.
- [x] Add a settings store port and real SQLite implementation.
- [x] Add dedicated get and update use cases with revision conflict handling.
- [x] Apply committed settings to refresh, lifecycle, and supported platform
      owners without exposing those adapters to IPC.
- [x] Add `settings_get` and refine `settings_update` DTOs and generated types.
- [x] Move frontend settings fetching and mutation behind a feature query.
- [x] Cover validation, conflict, restart, and runtime-application behavior.
- [x] Update the Phase 8 overview and move this plan when verified.

## Test Plan

- Behavior and invariants to prove: valid replacement persists; invalid input and
  stale revisions do not mutate state; runtime owners observe committed values.
- Lowest stable test layer: pure settings validation, application use-case tests,
  real SQLite repository tests, IPC bridge tests, and focused React tests.
- Failure paths: invalid timezone or interval, stale revision, database failure,
  and unsupported platform capability.
- Fixtures or fakes: fake clock/runtime appliers; real SQLite for persistence.
- Runtime or platform evidence: restart persistence and close/refresh behavior
  are recorded before completion.
- Relevant commands: focused Rust and frontend tests, `pnpm contracts:check`,
  `pnpm verify`, and `pnpm verify:runtime` where platform behavior changes.

## Decisions

- A complete settings replacement with a revision is preferred to a generic
  patch map.
- Project-path cleanup is delegated to Phase 8B because it is a destructive
  privacy operation, not an ordinary field update.
- Launch-at-login must remain capability-aware; do not report success when the
  platform adapter is unavailable.
- Schema migration `0002_settings_revision.sql` adds optimistic concurrency
  without rebuilding the settings table.
- Reporting timezone is snapshotted once per refresh run so a concurrent settings
  change cannot split one run across timezones.
- Unsupported launch-at-login, notifications, and project-path retention changes
  remain read-only in the UI until their owning platform/privacy phases exist.
- Settings mutation uses an explicit command-request wrapper; flat Tauri
  arguments are not compatible with the Rust `request` command parameter.

## Verification

- Command: `pnpm verify`
- Outcome: passed on 2026-06-18. This included 42 frontend tests, 186 passing
  Rust tests with one opt-in collector smoke test ignored, Clippy with warnings denied,
  architecture and public API checks, contract drift, migration checks, and
  duplicate-code reporting. ESLint reported only existing warning-level signals
  outside the new settings feature.
- Additional commands:
  - `pnpm test:e2e`: passed, 10 tests across desktop and compact projects.
  - `cargo test --manifest-path src-tauri/Cargo.toml updated_settings_survive_database_reopen`:
    passed.

## Runtime Evidence

- `pnpm verify:runtime` passed on 2026-06-18.
- Environment: Ubuntu 24.04, Linux 6.17.0-35-generic, GNOME on X11.
- Evidence covered production frontend build, five Tauri IPC bridge tests,
  lifecycle and refresh scheduler tests, and ten Playwright tests including
  dedicated settings load/save behavior on desktop and compact viewports.
- Native launch-at-login and notification behavior remain unavailable and were
  not claimed as runtime-supported.

## Follow-Up Debt

- None.
