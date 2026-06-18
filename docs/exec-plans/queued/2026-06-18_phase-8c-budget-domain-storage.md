# 2026-06-18 Phase 8C Budget Domain And Storage

## Objective

Define Burnly-owned budget rules and a durable SQLite store with explicit
invariants, revisions, thresholds, and source scope.

## Acceptance Criteria

- Domain types represent token and cost budgets without invalid combinations.
- Daily, weekly, and monthly periods are explicit values.
- Budgets are global or source-specific; model and project scopes are absent.
- Thresholds use integer basis points and have deterministic ordering.
- Create, update, enable, disable, delete, get, and list operations are
  transactionally correct.
- Mutable operations use revision checks.
- Purpose-built application models expose no SQL rows.

## Risk Class

`high`

Budgets are durable user-owned state and later drive visible progress and native
notifications.

## Impact Areas

- `src-tauri/src/domain/budget/`
- `src-tauri/src/application/budgets/`
- Budget store port
- SQLite budget repository and migration compatibility
- Real SQLite repository tests

## Design Review

- What complexity is being introduced? Metric-specific values, period identity,
  source scope, thresholds, revisions, and transactional aggregate writes.
- Which decisions are hidden inside the owning module? Domain constructors own
  valid combinations; the store owns row replacement and concurrency.
- Is each new interface simpler than its implementation? Application callers
  manipulate one budget aggregate rather than three related tables.
- What special cases exist, and can the design eliminate them? Token and cost
  values use separate typed variants; optional source scope is explicit.
- Why is each new abstraction needed now? The aggregate and store hide existing
  multi-table persistence complexity.
- Can an existing module absorb this responsibility cleanly? Budgets require
  their own domain/application module; usage stores should not own them.

## Checklist

- [ ] Finalize first-release budget value, period, source-scope, and threshold
      types from locked product/database decisions.
- [ ] Define validation and revision-conflict errors.
- [ ] Add the budget store port at aggregate granularity.
- [ ] Implement real SQLite create/read/update/delete/list behavior.
- [ ] Add migration changes only if revision support requires them.
- [ ] Prove foreign keys, threshold replacement, delete cascade, and restart.
- [ ] Add application commands independent of IPC.
- [ ] Confirm no generic repository or rules engine is introduced.

## Test Plan

- Behavior and invariants to prove: invalid metric/currency combinations cannot
  exist; threshold identity is unique; stale updates fail; deletes cascade.
- Lowest stable test layer: domain unit tests and real SQLite repository tests.
- Failure paths: unknown source, duplicate/stale mutation, constraint failure,
  and transaction rollback.
- Fixtures or fakes: deterministic sources and budgets; real SQLite.
- Runtime or platform evidence: none.
- Relevant commands: focused Rust tests, migration checks, `pnpm verify`.

## Decisions

- A budget is an aggregate containing its thresholds.
- Store values as integer tokens or cost micros; do not use floating point.

## Verification

- Command: `pnpm verify`
- Outcome: not run yet

## Runtime Evidence

- Not required.

## Follow-Up Debt

- Exact default threshold suggestions are finalized with the budget UI, while
  persisted threshold semantics remain unchanged.
