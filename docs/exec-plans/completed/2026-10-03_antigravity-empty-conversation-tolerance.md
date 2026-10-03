# 2026-10-03 Antigravity Empty Conversation Tolerance

## Status

Completed on October 3, 2026.

## Objective

Prevent false-positive `antigravity.runtime_not_found` warnings and `source.not_found` refresh failures when local Antigravity CLI conversation databases exist with zero token generation records and no active runtime is running.

## Acceptance Criteria

- When an Antigravity CLI conversation database is successfully opened and parsed from SQLite but contains zero generation records, and no live runtime endpoints are active, the collector treats this as normal zero-usage rather than requiring a runtime endpoint.
- Antigravity collector returns a successful `CollectionResult` with zero usage candidates when all conversations in scope have zero usage and no runtime failures occurred.
- Real runtime failures (when runtime is actually required and fails, such as unhandled App/IDE conversations or corrupt SQLite databases) continue to report `antigravity.runtime_not_found` and `CollectorFailureCode::SourceNotFound`.
- Collector unit tests verify:
  1. A successfully parsed empty CLI conversation database with no runtime process returns an empty `CollectionResult` with `outcome == Complete` and no diagnostic warnings.
  2. Missing runtime endpoints for unhandled conversations still produce `SourceNotFound` errors and warning diagnostics.
- Full verification gates pass (`pnpm verify:fast`, `pnpm architecture:check`, `pnpm test`).

## Risk Class

`medium`

The change is scoped strictly to Antigravity collector runtime target selection and empty-usage handling in `src-tauri/src/infrastructure/collectors/antigravity/`.

## Impact Areas

- `src-tauri/src/infrastructure/collectors/antigravity/cli_sqlite_reader.rs`
- `src-tauri/src/infrastructure/collectors/antigravity/app_ide_sqlite_reader.rs`
- `src-tauri/src/infrastructure/collectors/antigravity/adapter.rs`
- Collector adapter tests in `adapter.rs`

## Design Review

- **What complexity is being introduced?**
  Minimal. Tracking parsed conversation IDs in `CliSqliteCollectionReport` and `AppIdeSqliteCollectionReport` so `conversations_needing_runtime` can distinguish between "conversation was parsed from SQLite and yielded zero records" vs "conversation could not be parsed from SQLite".
- **Which decisions are hidden inside the owning module?**
  The Antigravity collector adapter encapsulates the decision of whether a conversation artifact requires the live language-server runtime or can be satisfied entirely from its local SQLite database.
- **Is each new interface simpler than its implementation?**
  No new public interfaces; only internal collector report fields.
- **What special cases exist, and can the design eliminate them?**
  Eliminates the special case where an empty session file caused an erroneous runtime-missing error. An empty session is now treated consistently with empty sessions in other collectors.
- **Why is each new abstraction needed now?**
  No new abstraction is introduced.
- **Can an existing module absorb this responsibility cleanly?**
  Yes, `adapter.rs`, `cli_sqlite_reader.rs`, and `app_ide_sqlite_reader.rs` own this logic directly.

## Checklist

- [x] Add `parsed_conversation_ids` to `CliSqliteCollectionReport` in `cli_sqlite_reader.rs`.
- [x] Add `accepted_conversation_ids` to `AppIdeSqliteCollectionReport` in `app_ide_sqlite_reader.rs`.
- [x] Update `conversations_needing_runtime` in `adapter.rs` to take the parsed/accepted conversation sets and active endpoints.
- [x] Update `finish_collection` in `adapter.rs` so that empty usage without a runtime failure returns a successful empty result rather than forcing `SourceNotFound`.
- [x] Add a unit test in `adapter.rs` verifying that an empty CLI SQLite database with no runtime returns `Ok(empty_result)` without warning diagnostics.
- [x] Ensure existing tests (e.g. `returns_source_not_found_when_runtime_endpoint_is_missing`) continue to pass.
- [x] Run fast local gate and architecture checks (`pnpm verify:fast`).

## Test Plan

- **Behavior and invariants to prove:**
  An offline empty CLI session database (0 rows in `gen_metadata`) yields a successful collection result with 0 candidates, `CollectionOutcome::Complete`, and no warning events.
- **Lowest stable test layer:**
  `infrastructure::collectors::antigravity::adapter::tests`.
- **Failure paths:**
  Corrupt database files, unhandled App/IDE conversations, and true runtime metadata errors still emit their respective failure codes and diagnostic events.
- **Fixtures or fakes:**
  `create_cli_db` test helper in `adapter.rs`.
- **Runtime evidence:**
  Verified on live desktop environment.
- **Relevant commands:**
  `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors::antigravity`
  `pnpm verify:fast`

## Decisions

- Track parsed conversation IDs in collection reports so the adapter knows which conversations were successfully inspected offline.
- Do not consider an offline, successfully parsed CLI conversation as needing a runtime endpoint.
- Only return `Err(failure)` in `finish_collection` when a real runtime failure occurred (`runtime_failure.is_some()`).

## Verification

- Command: `pnpm rust:fmt`
  - Outcome: passed
- Command: `pnpm format:check`
  - Outcome: passed
- Command: `pnpm lint`
  - Outcome: passed (0 errors, 5 pre-existing warnings)
- Command: `pnpm typecheck`
  - Outcome: passed
- Command: `pnpm test`
  - Outcome: passed (20 test files, 119 tests passed)
- Command: `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors::antigravity`
  - Outcome: passed (90 passed, 0 failed, 1 ignored)
- Command: `cargo test --manifest-path src-tauri/Cargo.toml`
  - Outcome: passed (716 passed, 0 failed, 3 ignored)
- Command: `pnpm verify`
  - Outcome: passed (Rust fmt, Clippy, test suite, and full harness check)
- Command: `pnpm harness:check`
  - Outcome: passed (all 18 harness scripts and self-tests passed)

## Runtime Evidence

- Live desktop build packaged to AppImage and installed to `~/.local/share/burnly/Burnly.AppImage`.
- Live manual refresh runs (`6119` and `6120`) executed on desktop with empty CLI session database `eab03060-6466-4b92-a93b-35157e794c11.db` in scope.
- Both manual runs succeeded completely (`status: "succeeded"`, `error: null`).
- Antigravity collector emitted `antigravity.collection_completed` with `severity: info`, `recordsExtracted: 0`, and no failure diagnostics.

## Follow-Up Debt

- None.
