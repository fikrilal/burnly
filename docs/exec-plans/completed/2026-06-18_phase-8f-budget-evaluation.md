# 2026-06-18 Phase 8F Budget Evaluation

## Objective

Compute authoritative budget progress and threshold transitions from committed
daily usage using deterministic reporting-timezone period boundaries.

## Acceptance Criteria

- Daily, weekly, and monthly period boundaries are deterministic in the
  configured reporting timezone.
- Global and source-specific budgets query only active daily facts.
- Token budgets sum tokens; cost budgets preserve currency and unavailable-cost
  semantics.
- Progress and crossed thresholds are computed in Rust with integer arithmetic.
- Evaluation runs after committed daily changes and never inside collector or
  reconciliation transactions.
- Multiple thresholds crossed by one refresh produce ordered decisions.

## Risk Class

`high`

Incorrect period or aggregation logic can produce misleading alerts and budget
progress.

## Impact Areas

- Budget domain period/progress rules
- Budget evaluation application service
- Usage/budget read-store ports and SQLite queries
- Refresh post-commit orchestration
- Deterministic clock/timezone tests

## Design Review

- What complexity is being introduced? Timezone-aware period identity,
  authoritative aggregation, and threshold transition decisions.
- Which decisions are hidden inside the owning module? Budgets own period
  arithmetic and eligibility; SQLite owns efficient daily-fact aggregation.
- Is each new interface simpler than its implementation? Callers request
  evaluation after a committed change and receive progress plus decisions.
- What special cases exist, and can the design eliminate them? Unavailable cost
  is explicit, DST is handled through date boundaries, and multiple crossed
  thresholds are a collection rather than branches.
- Why is each new abstraction needed now? Evaluation hides rules used by
  overview, tray, and notifications.
- Can an existing module absorb this responsibility cleanly? The budget module
  owns it; refresh only invokes it after commit.

## Checklist

- [x] Define period identity and boundary calculations.
- [x] Define progress and threshold-decision models.
- [x] Add authoritative daily-fact aggregation for budget scope.
- [x] Implement evaluation for all metric, period, and source variants.
- [x] Invoke evaluation after committed daily reconciliation changes.
- [x] Prove timezone, DST, period rollover, unavailable cost, and multi-threshold
      behavior.
- [x] Confirm evaluation failure does not invalidate committed usage.

## Test Plan

- Behavior and invariants to prove: correct boundaries and sums; no double
  counting; stable threshold order; post-commit isolation.
- Lowest stable test layer: pure period/evaluation tests and real SQLite query
  tests.
- Failure paths: invalid timezone, unavailable cost, deleted/disabled budget,
  database failure after usage commit.
- Fixtures or fakes: fixed clock, boundary dates, multiple sources/currencies.
- Runtime or platform evidence: none.
- Relevant commands: focused Rust tests, `pnpm verify`.

## Decisions

- Weekly periods require an explicit product convention during implementation;
  default to ISO Monday-start weeks unless an approved document says otherwise.
- Progress may exceed 100 percent and must not be clamped in authoritative data.
- Budget evaluation is read-only in Phase 8F. Threshold decisions are returned
  in memory; persistence/deduplication for notification delivery remains Phase
  8G.
- Cost threshold decisions are emitted only when cost is computable in the
  budget currency. Unavailable cost is represented explicitly and does not emit
  threshold decisions.
- Refresh invokes evaluation after daily reconciliation commits and ignores
  evaluation failure so committed usage and refresh lifecycle are not rolled
  back.

## Verification

- Command: `cargo test --manifest-path src-tauri/Cargo.toml budget_evaluation --no-fail-fast`
- Outcome: passed; period, DST, token, cost, and refresh hook tests.
- Command: `cargo test --manifest-path src-tauri/Cargo.toml budget_usage_store --no-fail-fast`
- Outcome: passed; real SQLite aggregation tests.
- Command: `cargo test --manifest-path src-tauri/Cargo.toml coordinator --no-fail-fast`
- Outcome: passed; 13 refresh coordinator tests.
- Command: `pnpm verify`
- Outcome: passed; 55 frontend tests, 217 Rust tests, 1 ignored sidecar smoke
  test, clippy/rustfmt/harness checks passed. ESLint and duplication reports
  remain warning-style configured output.

## Runtime Evidence

- Not required.

## Follow-Up Debt

- Rolling periods and anomaly detection are outside Phase 8.
- Expose the progress read model to overview/tray in Phase 8H.
