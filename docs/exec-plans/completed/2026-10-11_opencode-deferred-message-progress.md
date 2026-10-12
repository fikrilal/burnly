# 2026-10-11 OpenCode Deferred Message Progress

## Objective

Incremental OpenCode collect must not re-read completed messages that sit
after an incomplete live row. Point-read the stored incomplete message ids
and range-read only rows past the last scanned id.

## Acceptance Criteria

- Migration `0015` adds `opencode_deferred_message`. A fresh database has 25
  tables. The table stores session id and message id only.
- `last_source_message_id` is the high water of rows already scanned,
  including deferred and non-usage rows. A read with no rows stores null.
- The next incremental read point-looks up stored deferred ids and ranges
  `id` greater than that high water. A still-incomplete id stays in the set.
  A completed id is reconciled and dropped. A missing or non-usage id is
  dropped.
- Full scope still reads from the start and replaces the deferred set from
  that scan. It does not point-read the stored ids.
- Header regression does not reconcile and does not replace the cursor or
  the deferred set.
- Cursor and deferred ids commit in one transaction after `reconcile_session`
  succeeds and before projection mapping. A reconcile failure persists
  neither.
- `checkpoint_matches_header` stays false for `DeferredLiveWrite`.
- Message SQL stays on `json_extract`. No prompt, response, or content
  column.

## Risk Class

`medium`.

## Impact Areas

- `src-tauri/migrations/0015_opencode_deferred_messages.sql`
- `src-tauri/src/infrastructure/database/migrations.rs`
- `src-tauri/src/application/ports/opencode_usage_ledger.rs`
- `src-tauri/src/infrastructure/database/opencode_usage_ledger_store.rs`
- `src-tauri/src/infrastructure/collectors/opencode/store.rs`
- `src-tauri/src/infrastructure/collectors/opencode/adapter.rs`

## Design Review

- The new table hides one fact: which source message ids are still
  incomplete. Callers do not learn SQLite, JSON shape, or page layout.
- One cursor cannot both stay before a hole and skip the completed tail.
  The high water is the scan position. The deferred set is the retry list.
- `store_message_progress` owns both writes so a crash cannot advance the
  cursor while leaving the old hole as the only retry key.
- No new collector mode flag. `DeferredLiveWrite` still re-enters the read
  path. The read is smaller.
- The first incremental pass after upgrade still range-reads the tail once,
  because old cursors sit before the hole and the deferred table starts
  empty. The second pass is the cheap gate.

## Checklist

- [x] Add migration `0015` and expect 25 tables.
- [x] Add ledger read and replace methods.
- [x] Add by-id message reads that prefer v2.
- [x] Advance the cursor to the scanned high water and retry stored ids.
- [x] Cover the ledger round-trip and the incremental hole.

## Test Plan

- Behavior and invariants to prove:
  - Cursor and deferred ids survive a later reconcile.
  - Empty ids are rejected. A missing session errors. An empty replacement
    clears the ids.
  - A v2 session with completed `message-1`, incomplete `message-2`, and
    completed `message-3` reads two messages on the first incremental pass,
    zero messages and zero pages on the second, and one by-id row when
    `message-2` completes after the header grows.
- Lowest stable test layer: ledger store and OpenCode collector tests.
- Failure paths: incompatible id, missing checkpoint, header regression
  leaves progress unchanged.
- Fixtures or fakes: existing OpenCode fixture and `CountingLedger`.
- Runtime or platform evidence: not part of this chunk. The installed
  AppImage does not contain this fix. The first post-upgrade refresh of the
  deferred sessions can still read the tail once.
- Relevant commands:
  - `pnpm migrations:check`
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib opencode`
  - `pnpm verify`

## Decisions

- This supersedes the completed scoped-collect rule that the cursor stays at
  the last completed id before the first deferred live row.
- By-id reads use one prepared statement per session. They do not count as
  message pages. A still-incomplete by-id row does not increment
  `messagesRead`. A completed one increments it by one.
- An id already returned by the page is not point-read again.
- `persist_checkpoint` still does not write `last_source_message_id` and
  does not touch the deferred table.
- `store_message_cursor` stays for the cursor-only path. Collect uses
  `store_message_progress`.

## Verification

- Command: `pnpm migrations:check`
- Outcome: passed on 2026-10-11.
- Command: `cargo test --manifest-path src-tauri/Cargo.toml --lib opencode`
- Outcome: 84 passed, 0 failed, 1 ignored
  (`runtime_evidence_collects_default_location_without_sensitive_output`).
- Command: `pnpm verify`
- Outcome: passed on 2026-10-11. Prettier, ESLint (0 errors, 5 pre-existing
  warnings), `tsc`, Vitest 119 passed, sidecar prepare, `cargo fmt --check`,
  Clippy `-D warnings`, Rust tests 813 passed / 0 failed / 3 ignored, and the
  harness checks all passed. Fresh databases now have 25 tables.
- An earlier `pnpm verify` in the same session failed two unrelated refresh
  tests, `baseline_repair_completion_publishes_usage_changed_true` and
  `baseline_repair_failure_emits_diagnostic_and_does_not_fail_refresh`. Both
  passed immediately afterward, and the following full `pnpm verify` passed.

## Runtime Evidence

- Not required for this chunk. Live measurement on 2026-10-11 showed the
  installed scoped-collect build re-reading the deferred tail.

## Follow-Up Debt

- The packaged `read_bytes` gate is still unmeasured on a build that contains
  this fix. The first refresh after migration `0015` can still read the old
  tail once.
