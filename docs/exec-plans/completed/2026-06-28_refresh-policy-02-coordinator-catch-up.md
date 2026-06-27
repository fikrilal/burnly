# 2026-06-28 Refresh Policy 02 Coordinator Catch-Up

## Objective

Wire scheduled, startup-after-gap, resume, and normal manual refresh through the
refresh policy planner so automatic catch-up uses bounded incremental scopes
after a baseline exists.

## Acceptance Criteria

- Automatic catch-up triggers use source/projection-specific incremental scopes
  with a two-day lookback when prior successful import state exists.
- Missing-baseline targets still run full refresh.
- Normal manual refresh uses catch-up incremental policy after baseline.
- Collector requests receive the planned scope for each target.
- Reconciliation remains idempotent and does not treat out-of-scope historical
  rows as absent.

## Risk Class

`high`

This changes refresh behavior for core data ingestion paths and could affect
historical correctness if scope handling is wrong.

## Impact Areas

- `src-tauri/src/application/refresh*`
- `src-tauri/src/bootstrap.rs`
- `src-tauri/src/domain/usage*`
- `src-tauri/src/infrastructure/*collector*`
- Rust refresh/coordinator/reconciliation tests

## Design Review

- Keep trigger policy in the application layer; collectors should only execute
  requested scopes.
- Do not expose collector command details to domain or UI code.
- Preserve full-refresh support for baseline and future explicit resync.
- Avoid broad scheduler interval changes in this chunk.

## Checklist

- [x] Activate and complete planner/import-state chunk first.
- [x] Thread planned scopes into coordinator collection requests.
- [x] Route scheduled, startup-after-gap, resume, and normal manual refresh
      through catch-up policy.
- [x] Preserve full refresh for missing source/projection baseline.
- [x] Add tests proving collector request bounds and scoped reconciliation
      behavior.
- [x] Record verification outcomes when this plan becomes active.

## Test Plan

- Behavior and invariants to prove: catch-up triggers are bounded; missing
  baseline remains full; manual refresh is catch-up; scoped absence does not
  delete older out-of-scope canonical data.
- Lowest stable test layer: coordinator tests, collector fake assertions,
  reconciliation tests.
- Failure paths: partial source failures, failed incremental import, no baseline
  for one projection but baseline for another.
- Fixtures or fakes: existing collector fakes and SQLite-backed stores.
- Runtime or platform evidence: not required unless desktop integration changes.
- Relevant commands: `pnpm lint`, `pnpm verify:fast`, `pnpm verify`.

## Decisions

- Scheduled fallback interval remains unchanged until incremental behavior is
  proven reliable.
- Normal manual refresh is catch-up incremental, not full.

## Verification

- Command: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
- Outcome: passed; 204 passed, 1 ignored.
- Command: `pnpm lint`
- Outcome: passed with 15 existing warnings.
- Command: `pnpm verify:fast`
- Outcome: passed.
- Command: `pnpm verify`
- Outcome: passed.

## Runtime Evidence

- Not required unless this chunk changes tray-visible runtime behavior.

## Follow-Up Debt

- Add tray-open today-only freshness in the next chunk.
