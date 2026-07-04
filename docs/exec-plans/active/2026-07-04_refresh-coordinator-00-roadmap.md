# 2026-07-04 Refresh Coordinator Refactor Roadmap

## Objective

Coordinate the refresh coordinator cleanup described in
`docs/planning/_WIP/refresh-coordinator-audit.md` without changing refresh
policy, collector behavior, persistence semantics, or application-visible
contracts.

## Acceptance Criteria

- `RefreshCoordinator` remains the public application facade.
- Refresh request submission, coalescing, worker spawning, state snapshots, and
  event publication remain behaviorally unchanged.
- Refresh target support is reviewable outside the main coordinator facade.
- Scope planning and collection request construction are isolated from worker
  lifecycle code.
- Execution terminalization behavior remains explicit and tested.
- Existing application ports and collector contracts do not change.
- Existing coordinator behavior tests continue to pass.
- Each implementation chunk records verification before completion.

## Risk Class

`medium`

## Impact Areas

- `src-tauri/src/application/refresh/coordinator.rs`
- `src-tauri/src/application/refresh/mod.rs`
- New internal modules under `src-tauri/src/application/refresh/`
- Coordinator unit tests and fakes
- Architecture harness only if a repeated boundary issue appears

## Design Review

- What complexity is being introduced?
  - Internal module boundaries around existing coordinator responsibilities.
    No new product behavior or application-facing port is planned.
- Which decisions are hidden inside the owning module?
  - Process-wide refresh concurrency stays inside `RefreshCoordinator`.
  - Target planning stays inside refresh application code.
  - Import/refresh terminalization stays inside execution code.
- Is each new interface simpler than its implementation?
  - Internal helpers should expose deterministic request/target/outcome behavior
    while hiding low-level mapping details.
- What special cases exist, and can the design eliminate them?
  - `coordinator.rs` currently mixes public facade, execution flow, target
    catalog, outcome aggregation, hooks, and tests. The plan separates those
    ownership concerns.
- Why is each new abstraction needed now?
  - Refresh is Burnly's highest-risk application workflow. The file now changes
    whenever new sources, policies, diagnostics, or terminalization behavior are
    added.
- Can an existing module absorb this responsibility cleanly?
  - Existing `planner.rs`, `scheduler.rs`, and `state.rs` already have narrow
    responsibilities. New modules should complement them rather than expanding
    them into catch-all files.

## Checklist

- [x] Complete chunk 01: outcome and target helpers.
- [ ] Complete chunk 02: request planning extraction.
- [ ] Complete chunk 03: execution flow extraction.
- [ ] Complete chunk 04: tests and fakes relocation.
- [ ] Complete chunk 05: optional harness guardrails.
- [ ] Re-run the full local gate after all chunks are complete.
- [ ] Update `docs/planning/_WIP/refresh-coordinator-audit.md` with important
      implementation decisions or deviations.

## Test Plan

- Behavior and invariants to prove:
  - Concurrent refresh requests coalesce into the active run.
  - Failed worker spawn publishes a failed snapshot.
  - Successful refresh updates `last_successful_refresh_at_ms`.
  - Submission and terminal events are published with correct usage-change
    flags.
  - Full, catch-up, and freshness scope policies remain unchanged.
  - Daily targets carry aggregation timezone; session targets do not.
  - Collector failure for one target does not stop later targets.
  - Partial target failure yields partial refresh when any target succeeds.
  - Reconciliation/import/refresh completion failures remain terminalized.
  - Budget evaluation runs after daily commit and cannot fail refresh.
- Lowest stable test layer:
  - Existing Rust coordinator tests.
  - Focused tests for extracted pure target/outcome modules.
- Failure paths:
  - source resolution failure
  - collector failure
  - import creation failure
  - reconciliation failure
  - import completion failure after committed usage
  - refresh completion failure
  - worker thread spawn failure if feasible to keep covered
- Fixtures or fakes:
  - Existing fake run store, usage store, collector, clock, event sink, and
    budget evaluator.
- Runtime or platform evidence:
  - Not required unless public IPC/scheduler wiring changes, which this roadmap
    should avoid.
- Relevant commands:
  - `pnpm rust:fmt`
  - `pnpm rust:test`
  - `pnpm architecture:check`
  - `pnpm verify:fast`
  - `pnpm verify`

## Decisions

- Split by refresh lifecycle responsibility, not by source collector.
- Keep terminalization explicit. Do not hide open-run cleanup behind a generic
  error handler.
- Keep source/projection targets static for now; do not derive them dynamically
  from collector descriptors in this refactor.
- Do not implement deeper cancellation behavior in this refactor.

## Verification

- Command: not run yet
- Outcome: queued plan only

## Runtime Evidence

- Not required yet.

## Follow-Up Debt

- Revisit cancellation semantics separately after the refactor, if product scope
  requires cooperative collector cancellation.
