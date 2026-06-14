# 2026-06-14 Phase 1C Initial Migration

## Objective

Implement the approved initial SQLite schema and forward-only migration harness.

## Dependency

Phase 1B provides a verified SQLite runtime and temporary-database support.

## Acceptance Criteria

- `0001_initial.sql` implements the approved initial schema and integrity rules.
- Production tables use SQLite `STRICT` mode.
- The migration runner applies bundled immutable migrations in order.
- Fresh, repeated, failed, and unsupported-newer-schema paths behave safely.
- Foreign-key and integrity checks pass after migration.
- Required constraints reject invalid canonical values.
- Migration tests use temporary real SQLite databases.

## Non-Goals

- Repository implementations beyond what migration tests require
- Collector import or reconciliation
- UI read models
- Destructive migration machinery before a destructive migration exists, except
  for preserving the approved policy in documentation

## Risk Class

`high`

## Design Review

- Complexity introduced: long-lived schema compatibility and migration ordering.
- Decisions hidden: migration execution and schema-version checks stay inside the
  database runtime.
- Interface depth: startup requests migration to latest without knowing SQL files
  or version bookkeeping.
- Special cases: unsupported newer schemas and interrupted migration behavior are
  explicit failures, not normal branches throughout the application.
- Abstractions needed now: migration runner and schema tests only; repository APIs
  wait for application use cases.
- Existing ownership: database constraints protect canonical invariants while
  Rust retains complete semantic validation.

## Checklist

- [x] Revalidate this plan against completed Phase 1B behavior.
- [ ] Translate the approved schema into `0001_initial.sql`.
- [ ] Wire the migration registry and version checks.
- [ ] Add migration, constraint, integrity, and idempotency tests.
- [ ] Test unsupported newer schema handling.
- [ ] Run `pnpm migrations:check` and `pnpm verify`.
- [ ] Update the Phase 1 overview.

## Test Plan

- Fresh database migrates to latest.
- Re-running migration is a no-op.
- Failed migration preserves the prior committed state.
- Newer unsupported schema is rejected.
- Foreign-key and integrity checks pass.
- Approved invalid values are rejected by constraints.

## Verification

- Outcome: active; implementation not started.

## Activation Review

- Activated after the file-backed SQLite runtime passed six tests and the full
  verification gate.
- Migration code will remain inside `infrastructure/database`.
- Migration tests will reuse the Phase 1B `TestDatabase` fixture.
- The initial migration must follow the approved database design; no repository
  or IPC behavior is added in this chunk.

## Follow-Up Debt

- None.
