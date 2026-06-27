# 2026-06-28 Refresh Policy 01 Planner And Import State

## Objective

Create the application-layer refresh policy decision point and source/projection
import-state query without changing visible refresh behavior.

## Acceptance Criteria

- A refresh policy planner can choose full, catch-up incremental, or today-only
  freshness scopes from explicit inputs.
- Successful import state can be derived per source/projection from existing run
  data.
- First-install or missing-baseline targets plan a full refresh.
- Existing automatic refresh behavior remains unchanged until the coordinator is
  wired in a later chunk.

## Risk Class

`medium`

This introduces refresh policy logic and import-state read behavior, but should
not yet change production refresh triggers.

## Impact Areas

- `src-tauri/src/application/refresh*`
- `src-tauri/src/domain/import*`
- `src-tauri/src/infrastructure/*run*`
- Rust application and storage tests

## Design Review

- The planner should hide policy branching from the coordinator.
- Import-state queries should expose product-relevant state, not raw storage
  details.
- Avoid a dedicated cursor table unless deriving from import runs creates
  unclear or slow queries.
- The planner interface should use typed scope decisions rather than boolean
  mode flags.

## Checklist

- [ ] Inspect existing import-run storage and refresh coordinator dependencies.
- [ ] Add planner input/output types for refresh target, trigger kind, today,
      timezone, and last successful import state.
- [ ] Add import-state query support for successful source/projection runs.
- [ ] Add unit tests for baseline full, catch-up after a gap, two-day lookback,
      and today-only freshness.
- [ ] Record verification outcomes when this plan becomes active.

## Test Plan

- Behavior and invariants to prove: missing state plans full; existing state
  plans bounded incremental scopes; lookback never starts after today; freshness
  scope is today-only.
- Lowest stable test layer: planner unit tests and run-store query tests.
- Failure paths: empty import state, failed import runs, mixed source/projection
  histories.
- Fixtures or fakes: in-memory planner inputs and existing SQLite test stores.
- Runtime or platform evidence: not required.
- Relevant commands: `pnpm lint`, `pnpm verify:fast`.

## Decisions

- Product policy source: `docs/product/refresh-policy.md`.
- Implementation overview: `docs/planning/_WIP/refresh-policy-implementation-plan.md`.
- Two-day lookback is fixed policy for catch-up refresh.
- Today-only freshness is only valid after a baseline exists.

## Verification

- Command: `pnpm lint`
- Outcome: not run yet
- Command: `pnpm verify:fast`
- Outcome: not run yet

## Runtime Evidence

- Not required for this chunk.

## Follow-Up Debt

- Wire the planner into coordinator refresh paths in the next chunk.
