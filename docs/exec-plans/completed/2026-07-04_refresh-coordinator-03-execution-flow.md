# 2026-07-04 Refresh Coordinator 03 Execution Flow

## Objective

Extract side-effect-heavy refresh execution from `RefreshCoordinator` into an
internal execution module while preserving run/import terminalization,
reconciliation, and partial failure behavior.

## Acceptance Criteria

- `RefreshCoordinator` remains responsible for public refresh submission,
  coalescing, state snapshots, worker spawning, and terminal event publication.
- Execution flow lives outside the main coordinator facade.
- Refresh run creation/completion behavior is unchanged.
- Import run creation/completion behavior is unchanged.
- Reconciliation behavior is unchanged.
- Failure terminalization remains explicit in code and tests.
- Existing coordinator tests continue to pass.

## Risk Class

`high`

## Impact Areas

- `src-tauri/src/application/refresh/coordinator.rs`
- `src-tauri/src/application/refresh/execution.rs`
- `src-tauri/src/application/refresh/outcome.rs`
- `src-tauri/src/application/refresh/request_plan.rs` or equivalent
- Coordinator tests

## Design Review

- What complexity is being introduced?
  - A dedicated execution type/function that owns refresh side effects.
- Which decisions are hidden inside the owning module?
  - Lifecycle persistence order, target iteration, failure classification, and
    terminal cleanup.
- Is each new interface simpler than its implementation?
  - The coordinator should delegate execution and receive an execution result
    with terminal status, finish time, and usage-change flag.
- What special cases exist, and can the design eliminate them?
  - Import completion failure after committed reconciliation must report usage
    as changed. This special case must stay explicit.
- Why is each new abstraction needed now?
  - Execution is the largest risk area in the coordinator and blocks safe future
    changes to diagnostics, refresh policy, and sources.
- Can an existing module absorb this responsibility cleanly?
  - No. `planner.rs`, `scheduler.rs`, and `state.rs` are not side-effect
    execution owners.

## Checklist

- [x] Create `src-tauri/src/application/refresh/execution.rs`.
- [x] Move `execute` and `execute_open_refresh` behavior into execution code.
- [x] Move `persist` and `reconcile_collection` behavior into execution code.
- [x] Preserve `RefreshCoordinator` public methods and worker-spawn flow.
- [x] Keep terminal cleanup code explicit for open import and refresh runs.
- [x] Preserve budget evaluation after daily reconciliation only.
- [x] Preserve first-error behavior for partial/failed aggregate refreshes.
- [x] Keep error codes and summaries stable.
- [x] Run all coordinator tests and full Rust tests.
- [x] Record verification outcomes before completion.

## Test Plan

- Behavior and invariants to prove:
  - Complete collection reconciles and succeeds.
  - Empty collection succeeds with no records.
  - Partial collection reports partial without failing.
  - Failed collection changes no facts.
  - Collector failure for one target keeps later targets and marks partial.
  - Source resolution failure terminalizes refresh.
  - Import creation failure terminalizes refresh.
  - Reconciliation failure terminalizes import and refresh.
  - Import completion failure after reconciliation keeps usage-changed true.
  - Refresh completion failure reports failed execution.
  - Budget evaluation failure does not fail refresh.
- Lowest stable test layer:
  - Existing coordinator tests.
  - Add focused execution tests only if moved code loses direct coverage.
- Failure paths:
  - collector failure
  - run store failures at each lifecycle point
  - usage store failure
  - budget evaluator failure
- Fixtures or fakes:
  - Existing fake stores, collector, clock, event sink, and budget evaluator.
- Runtime or platform evidence:
  - Not required unless public IPC or scheduler wiring changes.
- Relevant commands:
  - `cargo test --manifest-path src-tauri/Cargo.toml application::refresh::coordinator::tests::`
  - `pnpm rust:fmt`
  - `pnpm rust:test`
  - `pnpm architecture:check`
  - `pnpm verify:fast`

## Decisions

- Do not introduce a generic pipeline abstraction.
- Do not make terminal cleanup implicit.
- Do not alter cancellation behavior.
- Keep execution synchronous inside the worker thread.

## Verification

- Command: `cargo test --manifest-path src-tauri/Cargo.toml application::refresh:: --quiet`
- Outcome: passed; 43 refresh tests passed.
- Command: `cargo fmt --manifest-path src-tauri/Cargo.toml`
- Outcome: passed.
- Command: `pnpm rust:test`
- Outcome: passed; 338 Rust tests passed, 1 ignored.
- Command: `pnpm verify:fast`
- Outcome: passed; ESLint warnings and duplication report remain non-fatal.

## Runtime Evidence

- Not required.

## Follow-Up Debt

- Revisit cooperative cancellation separately if product scope requires it.
