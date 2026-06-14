# 2026-06-14 Phase 1B SQLite Runtime

## Objective

Implement Burnly's SQLite runtime ownership, path resolution, connection policy,
and temporary-database test support without introducing the production schema.

## Dependency

Phase 1A is completed and its module boundaries are verified.

## Acceptance Criteria

- `rusqlite` uses a pinned bundled SQLite build.
- `rusqlite_migration` is available for the next chunk.
- Platform-owned application-data path resolution is explicit and testable.
- Infrastructure owns connection creation and configuration.
- Every connection enables and verifies foreign keys and a bounded busy timeout.
- Database initialization configures and verifies WAL and the approved
  synchronous durability policy where applicable.
- Temporary real SQLite databases use the same connection policy in tests.
- Connection and path failures map to stable internal persistence errors.

## Non-Goals

- Production tables or `0001_initial.sql`
- Repository queries
- Reconciliation or imports
- IPC database status contracts
- Connection pooling without measured need

## Risk Class

`high`

## Design Review

- Complexity introduced: filesystem path policy and SQLite connection lifecycle.
- Decisions hidden: callers request an initialized database runtime; they do not
  issue pragmas or construct paths.
- Interface depth: one constructor hides path creation, SQLite opening, policy
  application, and verification.
- Special cases: in-memory databases can differ from WAL-backed files; prefer
  temporary files when testing production connection behavior.
- Abstractions needed now: a concrete database runtime is needed; generalized
  pools and repository traits are not.
- Existing ownership: platform resolves paths; infrastructure owns SQLite.

## Checklist

- [x] Revalidate this plan against the completed Phase 1A code.
- [ ] Add pinned SQLite dependencies.
- [ ] Implement application-data path resolution.
- [ ] Implement connection initialization and verification.
- [ ] Add temporary database test support.
- [ ] Add failure classification.
- [ ] Run focused persistence tests and `pnpm verify`.
- [ ] Update the Phase 1 overview.

## Test Plan

- Use temporary real database files.
- Verify foreign keys, busy timeout, journal mode, synchronous policy, and repeat
  initialization.
- Verify invalid/unwritable path behavior where deterministic.
- Do not mock SQLite.

## Verification

- Outcome: active; implementation not started.

## Activation Review

- Activated after Phase 1A passed `pnpm verify` and native desktop startup.
- Platform path resolution will live under `platform`.
- SQLite connection ownership and persistence errors will live under
  `infrastructure`.
- No repository traits or connection pool will be introduced without a current
  caller requirement.

## Follow-Up Debt

- None.
