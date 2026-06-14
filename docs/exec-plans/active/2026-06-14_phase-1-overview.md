# 2026-06-14 Phase 1 Rust And SQLite Foundation

## Objective

Establish Burnly's durable Rust backend and SQLite foundation without exposing
persistence details to IPC, collectors, or the frontend.

## Phase Acceptance Criteria

- Rust ownership boundaries exist for domain, application, infrastructure, IPC,
  platform, and bootstrap code.
- Startup composition is explicit and testable.
- Burnly opens a bundled SQLite database with required connection policies.
- Fresh and supported prior databases migrate deterministically to the latest
  schema.
- Unsupported newer schemas are rejected safely.
- Database health, integrity, foreign-key enforcement, and required seed state
  are verified.
- Persistence and startup failures map into stable internal error categories.
- The complete Phase 1 migration and startup test suite passes.

## Risk Class

`high`

The phase establishes long-lived data compatibility and application startup
behavior.

## Chunk Plan

| Chunk                          | Status    | Dependency | Plan                                                             |
| ------------------------------ | --------- | ---------- | ---------------------------------------------------------------- |
| Phase 1A: Rust module skeleton | Completed | Phase 0    | [Plan](../completed/2026-06-14_phase-1a-rust-module-skeleton.md) |
| Phase 1B: SQLite runtime       | Completed | Phase 1A   | [Plan](../completed/2026-06-14_phase-1b-sqlite-runtime.md)       |
| Phase 1C: Initial migration    | Active    | Phase 1B   | [Plan](./2026-06-14_phase-1c-initial-migration.md)               |
| Phase 1D: Startup integration  | Queued    | Phase 1C   | [Plan](../queued/2026-06-14_phase-1d-startup-integration.md)     |

## Dependency Rules

- Phase 1A must lock ownership boundaries before persistence code is added.
- Phase 1B must prove connection policy and temporary-database support before the
  production schema is introduced.
- Phase 1C must prove migration correctness before startup depends on migrations.
- Phase 1D must not introduce IPC product contracts or collector behavior.
- A chunk moves from queued to active only after its dependency is completed and
  its assumptions are reviewed against the implemented code.

## Phase-Wide Design Review

- Complexity introduced: durable storage, migration compatibility, startup
  composition, and error classification.
- Decisions hidden: database construction and migration mechanics remain inside
  infrastructure; bootstrap exposes only application startup outcomes.
- Interface depth: callers receive a ready application context rather than
  coordinating paths, connections, pragmas, migrations, health checks, and seeds.
- Special cases: production and test paths may differ in location, but they must
  share the same connection and migration behavior.
- Abstractions needed now: only boundaries required to isolate persistence and
  compose startup. Repository interfaces wait until a real use case needs them.
- Existing ownership: Tauri remains the runtime edge; SQLite belongs to
  infrastructure; application and domain modules remain technology-independent.

## Phase-Wide Test Strategy

- Use pure module tests for ownership-neutral rules and error classification.
- Use temporary real SQLite databases for connection, migration, constraint,
  integrity, and seed behavior.
- Do not mock SQLite.
- Use startup integration tests with isolated application-data directories.
- Record desktop runtime evidence after startup uses the real database path.

## Progress

- [x] Phase 1A completed and verified.
- [x] Phase 1B completed and verified.
- [ ] Phase 1C completed and verified.
- [ ] Phase 1D completed and verified.
- [ ] Phase-level exit criteria verified.

## Decisions

- Split Phase 1 into four dependency-ordered implementation chunks.
- Keep this overview active until all Phase 1 chunks are completed.
- Use one active implementation chunk at a time.

## Verification

- Phase verification: not run yet.

## Follow-Up Debt

- None.
