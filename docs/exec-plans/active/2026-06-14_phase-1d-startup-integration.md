# 2026-06-14 Phase 1D Startup Integration

## Objective

Integrate path resolution, database initialization, migrations, health checks,
and required seed state into Burnly's explicit application startup sequence.

## Dependency

Phase 1C provides the verified initial schema and migration runner.

## Acceptance Criteria

- Bootstrap initializes the database before the Tauri application serves product
  behavior.
- Startup creates required directories, opens SQLite, runs migrations, verifies
  health, and ensures required seed state.
- Repeated startup is idempotent.
- Startup and persistence failures map to stable internal categories without
  leaking SQLite details across boundaries.
- The single-instance integration point remains explicit and does not compromise
  database ownership.
- Integration tests use isolated application-data directories.
- Real desktop startup succeeds with a migrated local database.

## Non-Goals

- Public IPC bootstrap DTOs, which belong to Phase 2
- Collector refresh on startup
- Background jobs, tray behavior, or settings UI
- Repository interfaces without a concrete application use case

## Risk Class

`high`

## Design Review

- Complexity introduced: startup sequencing and failure ownership.
- Decisions hidden: bootstrap owns ordering; Tauri and future IPC callers receive
  only ready state or a stable startup failure.
- Interface depth: one startup operation hides directory preparation, database
  opening, migration, health checks, and seeding.
- Special cases: first and repeated launch share one idempotent path.
- Abstractions needed now: concrete application state only for resources that
  startup actually creates.
- Existing ownership: bootstrap coordinates; platform resolves locations;
  infrastructure performs persistence work.

## Checklist

- [x] Revalidate this plan against completed Phase 1C behavior.
- [ ] Compose the startup sequence.
- [ ] Remove the temporary Phase 1B `dead_code` expectations when startup begins
      consuming the database path and runtime.
- [ ] Add health and integrity checks.
- [ ] Add idempotent seed initialization.
- [ ] Add startup and failure integration tests.
- [ ] Run isolated desktop startup evidence.
- [ ] Run `pnpm verify` and Phase 1 exit checks.
- [ ] Complete the Phase 1 overview.

## Test Plan

- Fresh isolated startup creates and migrates the database.
- Repeated startup is idempotent.
- Newer schema and unhealthy database states fail safely.
- Required seed state exists exactly once.
- Desktop runtime evidence confirms the actual application starts.

## Verification

- Outcome: active; implementation not started.

## Activation Review

- Activated after the initial migration passed the full verification gate.
- Startup can depend on one infrastructure operation to migrate to the latest
  supported schema.
- The schema contains 13 verified `STRICT` tables and rejects unsupported newer
  versions without changing the database.
- Phase 1D will own startup ordering, health checks, and seed initialization; it
  will not add product IPC or collector behavior.

## Follow-Up Debt

- None.
