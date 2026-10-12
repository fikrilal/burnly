# 2026-10-11 import_runs retention

## Objective

After a successful refresh commit, delete expired `import_runs` in bounded batches while every lookup key keeps its newest succeeded row.

## Acceptance Criteria

- Rows older than 14 days are deleted unless they are the newest succeeded row for source, projection, timezone, collector key, and profile version.
- Failure rows inside the 14-day window stay.
- Canonical usage tables are not deleted. An import row still referenced by usage stays.
- One refresh deletes at most 1000 rows. Later refreshes continue the backlog.
- A partial refresh does not run retention.

## Risk Class

`medium`.

## Impact Areas

- Run store
- Refresh execution

## Design Review

- The delete lives on `RunStore` and runs after `complete_refresh_run`, outside any collector transaction.
- The batch limit is inside the `SELECT`, so SQLite does not need `ENABLE_UPDATE_DELETE_LIMIT`.
- Foreign keys use `ON DELETE RESTRICT` for usage provenance, so referenced import ids are excluded instead of deleting usage rows.
- A retention failure records a diagnostic and leaves the already committed refresh successful.

## Checklist

- [x] Keep the newest succeeded row per lookup key.
- [x] Keep recent failures and referenced import ids.
- [x] Bound each delete to 1000 rows.
- [x] Run only after a succeeded refresh.

## Test Plan

- Behavior and invariants to prove: two old successes, a recent failure, a referenced older success, and a second profile's old success. The unreferenced older success is removed. The newest success remains. Usage row counts stay put.
- Lowest stable test layer: `SqliteReconciliationStore::delete_expired_import_runs` on a migrated temp database, plus the refresh coordinator hook on the fake run store.
- Failure paths: a third call after the backlog is drained deletes nothing.
- Fixtures or fakes: store API seeds. One daily reconcile attaches usage to the older success.
- Runtime or platform evidence: not required.
- Relevant commands: `cargo test --manifest-path src-tauri/Cargo.toml --lib delete_expired_import_runs_keeps_newest_success_and_recent_failures`

## Decisions

- Lookup key includes collector key and profile version because refresh planning filters on both.
- Null timezone is its own partition so session rows do not collapse onto a daily timezone.
- Batch size is 1000.

## Verification

- Command: `cargo test --manifest-path src-tauri/Cargo.toml --lib delete_expired_import_runs_keeps_newest_success_and_recent_failures -- --test-threads=8`
- Outcome: pass, 2026-10-11. 1 passed.
- Command: `cargo test --manifest-path src-tauri/Cargo.toml --lib complete_collection_reconciles_and_succeeds -- --test-threads=8`
- Outcome: pass, 2026-10-11. Retention ran once at finish time 1000.
- Command: `cargo test --manifest-path src-tauri/Cargo.toml --lib partial_collection_reports_partial_without_failing -- --test-threads=8`
- Outcome: pass, 2026-10-11. Retention did not run.
- Command: `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
- Outcome: pass, 2026-10-11.

## Runtime Evidence

- Not required.

## Follow-Up Debt

- The first launch against the existing 132,207-row history drains across later successful refreshes. That is intended.
