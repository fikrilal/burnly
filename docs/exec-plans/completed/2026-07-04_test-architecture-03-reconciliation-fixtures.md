# 2026-07-04 Test Architecture 03 Reconciliation Fixtures

## Objective

Reduce repeated SQLite fixture setup in database reconciliation tests while
continuing to test real SQLite behavior.

Status: Completed on July 5, 2026.

## Acceptance Criteria

- `src-tauri/src/infrastructure/database/reconciliation/test_support.rs` owns
  repeated reconciliation fixture construction.
- Reconciliation tests still use temporary real SQLite databases.
- Repository calls and transaction behavior are not mocked.
- Conflict, duplicate, recovery, and transaction assertions remain explicit.
- No schema, migration, repository, or reconciliation behavior changes.

## Risk Class

`medium`

## Impact Areas

- `src-tauri/src/infrastructure/database/reconciliation/tests.rs`
- `src-tauri/src/infrastructure/database/reconciliation/test_support.rs`
- SQLite reconciliation fixtures
- Persistence tests

## Design Review

- What complexity is being introduced?
  - Small fixture builders for repeated reconciliation setup.
- Which decisions are hidden inside the owning module?
  - Boring candidate row construction and database seed mechanics.
- Is each new interface simpler than its implementation?
  - Yes if tests describe daily/session/interrupted-run scenarios directly.
- What special cases exist, and can the design eliminate them?
  - Duplicate rows, conflicts, absence lifecycle, and interrupted runs are
    meaningful cases and should stay visible.
- Why is each new abstraction needed now?
  - Reconciliation tests are one of the largest files and duplicate setup
    blocks.
- Can an existing module absorb this responsibility cleanly?
  - Yes, inside the reconciliation module.

## Checklist

- [x] Inspect repeated SQLite setup in reconciliation tests.
- [x] Add `reconciliation/test_support.rs`.
- [x] Add a database fixture for creating temporary real SQLite stores.
- [x] Add daily candidate fixture builders.
- [x] Add session candidate fixture builders.
- [x] Add interrupted-run fixture builders where repeated.
- [x] Keep repository calls explicit in tests.
- [x] Keep assertions for conflict and recovery behavior explicit.
- [x] Run focused reconciliation tests.
- [x] Run duplication report and architecture checks.
- [x] Record verification outcomes before completion.

## Test Plan

- Behavior and invariants to prove:
  - Daily candidate reconciliation behavior is unchanged.
  - Session candidate reconciliation behavior is unchanged.
  - Duplicate/conflict behavior is unchanged.
  - Interrupted run recovery behavior is unchanged.
  - Transaction boundaries are still exercised through SQLite.
- Lowest stable test layer:
  - Database reconciliation persistence tests with real SQLite.
- Failure paths:
  - duplicate candidates
  - conflicting canonical rows
  - interrupted import runs
  - transaction rollback paths currently covered
- Fixtures or fakes:
  - Temporary SQLite databases.
  - Reconciliation-owned candidate builders.
- Runtime or platform evidence:
  - Not required.
- Relevant commands:
  - `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::database::reconciliation --lib`
  - `pnpm rust:test`
  - `pnpm duplication:report`
  - `pnpm architecture:check`

## Decisions

- Keep real SQLite in the tests.
- Do not add production database abstractions.
- Do not mock repositories or transaction behavior.

## Verification

- Command: `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::database::reconciliation --lib`
- Outcome: passed, 25 passed
- Command: `pnpm rust:test`
- Outcome: passed, 365 passed, 1 ignored
- Command: `pnpm duplication:report`
- Outcome: passed as report-only, existing clones remain
- Command: `pnpm architecture:check`
- Outcome: passed
- Command: `pnpm rust:fmt`
- Outcome: failed before formatting, passed after `pnpm rust:fmt:write`

## Runtime Evidence

- Not required yet.

## Follow-Up Debt

- None.
