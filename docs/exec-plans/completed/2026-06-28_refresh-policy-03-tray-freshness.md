# 2026-06-28 Refresh Policy 03 Tray Freshness

## Objective

Make tray-open stale refresh use today-only freshness scope after a baseline
exists, so the primary tray job stays fast: showing today's burned tokens.

## Acceptance Criteria

- Tray-open stale refresh plans today-only scope when prior baseline exists.
- Tray-open stale refresh falls back to full baseline behavior when no prior
  successful import exists.
- Week and month summaries are recalculated from SQLite after today's row is
  updated.
- Existing stale and throttle behavior remains unless evidence shows it should
  change.

## Risk Class

`medium`

This changes a visible refresh path but intentionally narrows the collector
scope only for a high-intent freshness trigger.

## Impact Areas

- `src-tauri/src/bootstrap.rs`
- `src-tauri/src/application/refresh*`
- `src-tauri/src/ipc/tray*`
- `src/features/tray/*`
- Rust tray/bootstrap/refresh tests

## Design Review

- Tray-open behavior should request freshness, not know collector mechanics.
- Today-only scope must not be used before a baseline exists.
- The summary read path should continue deriving week/month from stored canonical
  data, not from collector responses directly.
- Do not reintroduce user-configurable refresh settings.

## Checklist

- [x] Activate and complete coordinator catch-up chunk first.
- [x] Route tray-open stale refresh through today-only freshness policy after
      baseline.
- [x] Preserve baseline/full fallback for first install.
- [x] Add tests for today-only tray-open scope and fallback behavior.
- [x] Run runtime evidence if tray-visible behavior changes.
- [x] Record verification outcomes when this plan becomes active.

## Test Plan

- Behavior and invariants to prove: tray-open stale refresh is today-only after
  baseline; first-install tray-open refresh can still full-refresh; summaries
  include updated today plus stored historical rows.
- Lowest stable test layer: bootstrap/refresh tests with collector fakes and
  summary read tests.
- Failure paths: today-only refresh failure keeps existing data and does not
  advance successful import state.
- Fixtures or fakes: collector fake, SQLite stores, tray IPC harness if needed.
- Runtime or platform evidence: `pnpm evidence:desktop` if tray-visible behavior
  changes.
- Relevant commands: `pnpm lint`, `pnpm verify:fast`, `pnpm verify:runtime`.

## Decisions

- The two-day lookback is for catch-up paths, not tray-open freshness.
- Stale/throttle constants should be tuned only with evidence.

## Verification

- Command: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
- Outcome: passed; 206 passed, 1 ignored.
- Command: `pnpm lint`
- Outcome: passed with 15 existing warnings.
- Command: `pnpm verify:fast`
- Outcome: passed.
- Command: `pnpm verify:runtime`
- Outcome: passed; internally ran `pnpm evidence:desktop`.

## Runtime Evidence

- Command: `pnpm verify:runtime`
- Outcome: passed; desktop runtime evidence passed.

## Follow-Up Debt

- Consider a later explicit "resync all" action and compatibility repair policy.
