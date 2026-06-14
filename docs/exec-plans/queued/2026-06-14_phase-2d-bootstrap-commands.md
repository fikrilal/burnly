# 2026-06-14 Phase 2D Bootstrap Commands

## Objective

Implement and register `app_get_bootstrap` and `app_get_capabilities`, then render
their real data through the typed frontend client.

## Dependency

Phase 2C must provide the verified typed frontend IPC boundary.

## Acceptance Criteria

- Application-owned read models define bootstrap and capability behavior without
  depending on Tauri, SQLite rows, or IPC DTOs.
- Thin IPC handlers map those read models into dedicated wire DTOs.
- `app_get_bootstrap` returns real app version, contract version, database state,
  persisted reporting timezone, enabled feature summary, source summary, refresh
  state, last successful refresh time, and onboarding state.
- Fields without an implemented subsystem use an explicit truthful state rather
  than fabricated data or nullable ambiguity.
- `app_get_capabilities` returns real build/platform capabilities without exposing
  the operating-system name as a frontend decision mechanism.
- Both commands use the common response envelope and stable error mapping.
- The React shell invokes both commands only through `src/ipc/client.ts` and
  renders real version, database, settings, and capability data.
- Basic versioned event names and subscription infrastructure are registered;
  events remain invalidation notifications rather than authoritative data.

## Non-Goals

- Collector discovery or refresh execution
- Usage, sessions, budgets, settings forms, tray, or background jobs
- Returning dashboard history in bootstrap
- Generic repository interfaces unrelated to these two reads

## Risk Class

`high`

## Impact Areas

- Application bootstrap/capability read models
- Persistence read operations required by bootstrap
- Rust IPC DTOs, mappers, commands, and events
- Tauri command registration and managed state
- React application shell

## Design Review

- Complexity introduced: two concrete read paths and their delivery mapping.
- Decisions hidden: application queries assemble truthful state; IPC maps wire
  details; React renders capabilities without platform inference.
- Interface depth: each command exposes one purpose-built response instead of
  leaking database or platform APIs.
- Special cases: unimplemented subsystems use explicit states such as empty or
  unsupported; they do not gain placeholder repositories.
- Abstractions needed now: only read operations required by the two approved
  commands and event registration required for the frontend lifecycle.
- Existing ownership: bootstrap wires database and platform inputs into application
  queries; IPC remains a thin sibling adapter.

## Checklist

- [ ] Revalidate this queued plan against completed Phase 2C behavior.
- [ ] Define application bootstrap and capability read models.
- [ ] Add only the persistence reads required for persisted settings and health.
- [ ] Define dedicated IPC DTOs and mappings.
- [ ] Register both Tauri commands through the Phase 2B registry.
- [ ] Add versioned event names and typed subscription registration.
- [ ] Replace the placeholder React shell data with typed command results.
- [ ] Add Rust command and frontend rendering tests.
- [ ] Run `pnpm verify` and update the Phase 2 overview.

## Test Plan

- Behavior and invariants to prove: real persisted timezone, truthful database and
  subsystem states, real app version, platform capability mapping, envelope
  metadata, stable errors, and bounded bootstrap payload.
- Lowest stable test layer: application query tests, Rust command contract tests,
  and React integration tests.
- Failure paths: poisoned database state, persistence read failure, unsupported
  capability, and malformed frontend payload.
- Fixtures or fakes: isolated migrated databases and narrow platform capability
  inputs.
- Runtime or platform evidence: command registration may receive a smoke test;
  complete desktop invocation evidence is Phase 2E.
- Relevant commands: `cargo test`, `pnpm test`, `pnpm contracts:check`,
  `pnpm verify`.

## Decisions

- Bootstrap remains intentionally small and does not become a general dashboard
  preload endpoint.

## Verification

- Command: `pnpm verify`
- Outcome: queued; not run yet.

## Runtime Evidence

- Full evidence deferred to Phase 2E.

## Follow-Up Debt

- None.
