# 2026-10-12 Session lifetime totals

## Objective

Incremental OpenCode and Antigravity session collects keep lifetime session totals. Daily collects stay inside the request window.

## Acceptance Criteria

- An OpenCode session with activity before and inside a 2-day catch-up window keeps the full token total, model input, and earliest activity on the first incremental collect and on the second unchanged collect.
- A session whose activity is entirely outside that window is emitted from the first reconcile and skipped on the second collect, without a ledger fold.
- An OpenCode daily collect over the same window excludes the earlier day.
- An unchanged Antigravity conversation keeps cached usage from before the session window. The same daily window excludes it, and the cache read count stays windowed. The conversation file stays closed on both incremental collects.
- Daily OpenCode and Antigravity folds still apply the request window.

## Risk Class

`high`.

## Impact Areas

- OpenCode session fold
- Antigravity cache supplement
- Session usage candidates

## Design Review

- `ScopedSessionFold` no longer stores a scope. A scope on that fold was a mode that dropped lifetime rows. Skip versus fold stays in `cached_read`.
- Session ledger reads pass no activity window. Daily reads keep the window.
- Antigravity session supplement uses `CollectionScope::Full`. Daily supplement uses the request scope. No new flag.
- Job-scoped collector opens stay as shipped. The per-call close criterion already holds.

## Checklist

- [x] Stop date-filtering OpenCode session rows
- [x] Read the full OpenCode ledger when folding a session
- [x] Supplement Antigravity session usage without the catch-up window
- [x] Lock both with narrow-window tests
- [x] Run targeted tests and `pnpm verify`

## Test Plan

- Behavior and invariants to prove: lifetime session totals survive a real catch-up window; daily does not; an unchanged Antigravity file is not opened.
- Lowest stable test layer: collector adapter tests.
- Failure paths: an out-of-window OpenCode session is skipped on the second collect; the Antigravity daily cache read count stays windowed.
- Fixtures or fakes: V1 OpenCode messages on 2026-10-02 and 2026-10-12; an Antigravity CLI cache plus one older cache row.
- Runtime or platform evidence: not required. This is a collect invariant.
- Relevant commands: targeted `cargo test`, then `pnpm verify`.

## Decisions

- Do not change `JobScopedCollectorGraph`. The review's open-count finding does not match the checked acceptance criterion.
- Do not commit. Untracked migration 0016 and the existing untracked docs stay untracked until asked.
- The first incremental reconcile of a never-seen session emits its lifetime total, including activity outside the window. The second collect skips a session that is entirely outside the window.

## Verification

- Command: `pnpm verify`
- Outcome: exit 0 on 2026-10-12. Prettier clean. ESLint 5 existing warnings, 0 errors. Vitest 119 passed. Rust lib 828 passed, 0 failed, 3 ignored. Clippy `-D warnings` clean. Harness check passed.
- Targeted filter `session_keeps` passed `incremental_session_keeps_lifetime_totals_across_the_catch_up_window` and `unchanged_session_keeps_usage_from_before_the_incremental_window` before the full gate.

## Runtime Evidence

- Not required.

## Follow-Up Debt

- The installed AppImage still has the previous collect behavior until a rebuild. Do not rebuild in this chunk.
