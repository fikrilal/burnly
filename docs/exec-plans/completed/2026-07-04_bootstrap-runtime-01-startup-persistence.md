# 2026-07-04 Bootstrap Runtime 01 Startup Persistence

## Objective

Extract startup database initialization, interrupted run recovery, and recovery
diagnostic recording from `bootstrap.rs` into a focused bootstrap startup module
without changing startup behavior.

## Acceptance Criteria

- `src-tauri/src/bootstrap/startup.rs` owns database initialization and recovery
  helpers.
- Startup migration, health check, settings seed, recovery, and recovery
  diagnostic behavior remain unchanged.
- `setup_runtime` still initializes and recovers persistence before constructing
  refresh scheduler or requesting startup refresh.
- Startup-related tests move with the module or retain equal coverage.
- No database schema, migration, persistence port, or startup ordering behavior
  changes.

## Risk Class

`medium`

## Impact Areas

- `src-tauri/src/bootstrap.rs`
- `src-tauri/src/bootstrap/startup.rs`
- Startup database tests
- Recovery diagnostics tests

## Design Review

- What complexity is being introduced?
  - One bootstrap-owned module for startup persistence orchestration.
- Which decisions are hidden inside the owning module?
  - When to backup, migrate, verify, seed, recover interrupted runs, and record
    recovery diagnostics.
- Is each new interface simpler than its implementation?
  - Yes if callers only invoke initialization and recovery functions with the
    database path, timezone, and timestamp.
- What special cases exist, and can the design eliminate them?
  - Recovery diagnostics are only recorded when recovery changed state. Preserve
    this explicitly.
- Why is each new abstraction needed now?
  - Startup persistence is stable, tested behavior currently buried in the
    largest runtime composition file.
- Can an existing module absorb this responsibility cleanly?
  - No. The database adapter should expose operations; bootstrap owns startup
    orchestration order.

## Checklist

- [x] Create `src-tauri/src/bootstrap/startup.rs`.
- [x] Move database initialization helper.
- [x] Move interrupted run recovery helper.
- [x] Move recovery diagnostic helper.
- [x] Preserve `StartupError` mapping and public startup error kinds.
- [x] Move or update focused startup persistence tests.
- [x] Confirm `setup_runtime` ordering is unchanged.
- [x] Run focused bootstrap tests and fast verification.
- [x] Record verification outcomes before completion.

## Test Plan

- Behavior and invariants to prove:
  - Fresh startup creates, migrates, and seeds the database.
  - Repeated startup preserves existing settings.
  - Unsupported newer schema fails with stable category.
  - Foreign-key violation prevents startup.
  - Invalid seed value fails with stable category.
  - Interrupted refresh/import runs are terminalized and diagnostics recorded.
- Lowest stable test layer:
  - Bootstrap startup unit tests using real SQLite temp databases.
- Failure paths:
  - migration failure
  - health check failure
  - settings seed failure
  - run recovery failure
  - diagnostic write failure remains non-fatal
- Fixtures or fakes:
  - Real temp SQLite databases.
- Runtime or platform evidence:
  - Not required if only helpers move.
- Relevant commands:
  - `cargo test --manifest-path src-tauri/Cargo.toml bootstrap::`
  - `pnpm rust:test`
  - `pnpm verify:fast`

## Decisions

- Keep startup policy in bootstrap, not `infrastructure/database`.
- Keep `StartupError` in `bootstrap.rs` unless moving it is required by module
  visibility.

## Verification

- Command: `cargo test --manifest-path src-tauri/Cargo.toml bootstrap::`
- Outcome: passed; 23 passed, 0 failed.
- Command: `pnpm rust:test`
- Outcome: passed; 363 passed, 0 failed, 1 ignored.
- Command: `pnpm verify:fast`
- Outcome: passed; existing ESLint warnings and duplication report remain
  non-fatal.

## Runtime Evidence

- Not required unless startup behavior changes.

## Follow-Up Debt

- None.
