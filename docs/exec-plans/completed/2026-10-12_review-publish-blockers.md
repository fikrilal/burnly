# 2026-10-12 Review publish blockers

## Objective

Make the 0.1.37 tree safe to publish by fixing the OpenCode cost-shrink false positive already on `3aff521`, removing the two boundary leaks from the uncommitted work, and locking the rules the current suite can miss.

## Acceptance Criteria

- An incremental OpenCode collect whose tokens grew, and whose source cost is still below the accepted ledger cost but equal to the previously observed source cost, reconciles the new suffix and advances the cursor.
- A real source-cost drop, and a token shrink, still folds the stored rows and leaves the cursor.
- Injecting a baseline repair coordinator clears the per-job opener. Production still opens repair state per job when the opener is set.
- `AntigravityUsageCache` does not mention fingerprint, file length, or mtime. The checkpoint SQL stays in the cache store.
- The missing tests listed in the 2026-10-12 review fail if those rules regress.

## Risk Class

`medium`.

## Impact Areas

- OpenCode incremental collect
- Refresh coordinator repair opener
- Antigravity conversation checkpoint
- Import-run retention failure handling

## Design Review

- No new mode flag. The test coordinator replaces the production opener by clearing it.
- File identity stays inside the cache store. Collectors reach it through an infrastructure trait the store, the no-op cache, and the recording fake already need. Application code does not see the row.
- Cost regression compares the source header with `observed_source_cost_micros`. The priced-row tolerance stays in the ledger, which is the only place that knows that count. A real drop of the source header remains a shrink.
- No new migration. No user-database rewrite.

## Checklist

- [x] Compare header cost with the observed source cost
- [x] Clear the repair opener when a coordinator is injected
- [x] Move the conversation checkpoint methods off the application port
- [x] Lock the review's missing tests
- [x] Run the targeted Rust tests and `pnpm verify`

## Test Plan

- Behavior and invariants to prove:
  - Token growth with source cost 101 and accepted cost 102 reconciles and moves the cursor
  - Token shrink still folds
  - Unchanged `DeferredLiveWrite` header still point-reads
  - Full scope scans redundancy even when a proof exists
  - Failed `reconcile_session` leaves the cursor and deferred set
  - Antigravity full scope, missing cache, identity change, failed reconcile, and open error do not skip as unchanged
  - Retention error keeps the refresh successful and records `refresh.import_retention_failed`
  - A failed collection does not call retention
  - The production repair opener's file descriptors close when the job returns
- Lowest stable test layer: collector and refresh tests against temporary databases. File descriptors use `/proc/self/fd`.
- Failure paths: reconcile error, retention error, conversation open error, cache write error.
- Fixtures or fakes: OpenCode V1 cost-gap session, failing ledger port, Antigravity CLI conversation, fake run store.
- Runtime or platform evidence: not required for this correctness fix. The installed AppImage remains the pre-fix bundle.
- Relevant commands: targeted `cargo test`, then `pnpm verify`.

## Decisions

- Compare cost with `observed_source_cost_micros`, not with the ledger tolerance. The adapter does not know the priced-row count, and a one-micro drop of the source header itself must still fold.
- The production repair opener's connections close when the refresh thread returns. SQLite then parks the main-database descriptors while the long-lived writer still holds a POSIX lock, and later jobs reuse those descriptors. The steady descriptor count must fall below the in-job count and must not grow across jobs. It does not have to return to the post-compose baseline. Dropping the coordinator closes the parked descriptors too.

## Verification

- Command: `pnpm verify`
- Outcome: passed on 2026-10-12. Prettier clean. ESLint 5 pre-existing warnings, 0 errors. Vitest 119 passed. `cargo clippy --all-targets -- -D warnings` clean. Rust lib 826 passed, 0 failed, 3 ignored. Architecture, security, packaging, migration, and contract harnesses passed.

## Runtime Evidence

- Not required. The installed AppImage still contains the cost-shrink bug until a later rebuild.

## Follow-Up Debt

- Rebuild and reinstall the AppImage before calling 0.1.37 the publish candidate. Do not vacuum user databases. Do not push.
