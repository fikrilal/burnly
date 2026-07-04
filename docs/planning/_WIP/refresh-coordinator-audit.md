# Refresh Coordinator Audit

## Status

Drafted on July 4, 2026.

This audit focuses on `src-tauri/src/application/refresh/coordinator.rs`.

The goal is to reduce review risk in the refresh pipeline without changing the
application ports, refresh policy, collector contracts, persistence semantics, or
user-visible behavior.

This document is not an execution plan. It is an inspection and refactor
proposal that should be turned into small execution chunks before
implementation.

## Executive Summary

`RefreshCoordinator` is currently the highest-leverage refactor target in the
backend.

The file is large, but the line count is not the main problem. The risk is that
one module owns several independently fragile responsibilities:

- process-wide refresh concurrency
- refresh state snapshots and event publication
- worker thread spawning
- refresh run lifecycle persistence
- target iteration across all supported sources and projections
- refresh scope planning
- collection request construction
- collector execution and failure handling
- import run lifecycle persistence
- daily/session reconciliation request construction
- post-commit budget evaluation
- aggregate refresh outcome derivation
- terminal cleanup for partial failures
- a broad fake-based test suite

Most of this behavior belongs near the refresh application service, but not all
of it needs to live in one file. The safest direction is to preserve the
`RefreshCoordinator` public API while extracting internal modules around stable
concepts: target planning, execution bookkeeping, persistence/reconciliation
flow, and tests.

Recommended direction: split by refresh lifecycle responsibility, not by source
collector or by database table.

## Current File Map

Current refresh application files:

```text
2141 src-tauri/src/application/refresh/coordinator.rs
 260 src-tauri/src/application/refresh/planner.rs
 145 src-tauri/src/application/refresh/scheduler.rs
  63 src-tauri/src/application/refresh/state.rs
  12 src-tauri/src/application/refresh/mod.rs
2621 total
```

`coordinator.rs` is the clear hotspot. It contains both production orchestration
and the test harness/fakes that prove its behavior.

## Current Responsibility Map

### `RefreshCoordinator`

Responsibilities:

- expose refresh submission methods:
  - `request_refresh`
  - `request_full_refresh`
  - `request_freshness_refresh`
- coalesce concurrent refresh requests by returning the active snapshot
- spawn the background refresh worker thread
- update in-memory refresh state
- publish refresh snapshots through `RefreshEventSink`
- keep the current aggregation timezone
- own the injected collector, stores, clock, event sink, and budget evaluator

Assessment:

This should remain the public application-facing facade. It is the right place
to own process-wide concurrency and state snapshots. It should not also contain
all target planning, import persistence, reconciliation mapping, and test fakes.

### Refresh Execution Flow

Current functions:

- `finish_refresh`
- `execute`
- `execute_open_refresh`
- `persist`
- `reconcile_collection`
- `failed_result`
- `failure`
- `import_failure`

Responsibilities:

- create a refresh run
- iterate refresh targets
- resolve source IDs
- plan scope per target
- call the collector
- persist import runs
- reconcile usage
- complete import runs
- complete refresh runs
- preserve partial progress on target failures
- terminalize open import/refresh runs on failures

Assessment:

This is cohesive as "refresh execution", but too much for a single impl block.
The critical invariant is terminalization: once a refresh run or import run is
opened, failure paths must persist a terminal state where possible. Any refactor
must keep that behavior explicit and heavily tested.

### Refresh Target List And Request Construction

Current items:

- `RefreshTarget`
- `refresh_targets`
- `projection_label`
- `import_timezone`
- `local_date`
- `records_seen`
- `collection_request`
- `planned_scope`

Responsibilities:

- define supported source/projection pairs
- create collection IDs
- decide whether daily imports need a timezone
- translate target identity into planner input
- look up previous successful imports
- construct daily/session `CollectionRequest` values

Assessment:

This is a strong extraction candidate. It is stable, deterministic, and easy to
test without threading or persistence side effects.

The target list is becoming more important as Burnly adds more collectors. It
should be easy to review "what sources are refreshed" without reading the whole
coordinator.

### Outcome And Failure Bookkeeping

Current items:

- `RunOutcome`
- `TargetRunAccumulator`
- `ExecutionResult`
- `ExecutionFailure`
- `run_error`
- `clamp_count`

Responsibilities:

- map collection outcomes into import/refresh outcomes
- aggregate target outcomes into one refresh outcome
- carry enough failure context to terminalize an import run
- clamp record counts for storage

Assessment:

This is another good extraction candidate. It is small but central to partial
failure semantics. Keeping it isolated would make it easier to test aggregate
outcome rules directly.

### Hooks

Current items:

- `RefreshEventSink`
- `BudgetEvaluationRunner`
- `RefreshCoordinatorHooks`
- noop implementations

Responsibilities:

- decouple UI event publishing from coordinator internals
- run post-commit budget evaluation after daily reconciliation

Assessment:

These are useful extension points. They can stay close to the coordinator facade
or move into a small `hooks.rs` module. Avoid making them a general plugin
system.

### Tests

The test module currently owns:

- fake run store
- fake usage store
- fake collector
- fake clocks
- event sink and budget evaluator fakes
- candidate builders
- request/metadata fixtures
- many end-to-end coordinator behavior tests

Assessment:

The tests are valuable but inflate the production file and make navigation
harder. Moving tests to `coordinator/tests.rs` is a low-risk readability win, but
it should be done after production internals are split enough that test imports
stay simple.

## Non-Negotiable Invariants

Any refactor must preserve these behaviors:

- Concurrent refresh requests coalesce into the active run.
- A failed worker spawn publishes a failed snapshot.
- Refresh state moves to the terminal status produced by execution.
- Successful refresh updates `last_successful_refresh_at_ms`.
- Event sink sees submission and terminal events with correct `usage_changed`.
- Full refresh always uses `CollectionScope::Full`.
- Catch-up refresh uses planner catch-up policy.
- Freshness refresh uses today-only after a baseline exists.
- Daily targets include aggregation timezone; session targets do not.
- Collector failure for one target does not stop later targets.
- Partial target failure yields a partial refresh when any target succeeds.
- Complete failure yields a failed refresh.
- Reconciliation failure terminalizes the import and refresh runs as failed.
- Import completion failure reports usage as changed when reconciliation already
  committed.
- Budget evaluation runs after daily reconciliation and does not fail refresh.
- Record counts are clamped to storage-safe `u32`.

## Recommended Target Structure

Proposed module shape:

```text
src-tauri/src/application/refresh/
  coordinator.rs          # public facade, state, request submission, worker spawn
  execution.rs            # refresh execution flow and terminal cleanup
  target.rs               # supported targets, target planning, request construction
  outcome.rs              # RunOutcome, TargetRunAccumulator, ExecutionResult/Failure
  hooks.rs                # event sink, budget evaluator, hook bundle, noops
  planner.rs              # existing policy planner
  scheduler.rs            # existing scheduler
  state.rs                # existing snapshot/status
  tests.rs                # coordinator integration-style tests and fakes
```

This is a suggested end state, not a mandate. If implementation shows that
`hooks.rs` is too small to justify a file, it can stay in `coordinator.rs`.

## Recommended Execution Chunks

### Chunk 1: Move Outcome And Target Helpers

Scope:

- Extract `RunOutcome`, `TargetRunAccumulator`, `ExecutionResult`,
  `ExecutionFailure`, `run_error`, `clamp_count`, `RefreshTarget`,
  `refresh_targets`, `projection_label`, `import_timezone`, `local_date`, and
  `records_seen`.
- Keep behavior unchanged.
- Add or preserve focused tests for outcome aggregation and target/request
  mapping.

Why first:

- It is mostly pure code.
- It reduces the coordinator file before touching side-effect flow.
- It creates stable names for later chunks.

### Chunk 2: Extract Request Planning

Scope:

- Move `collection_request` and `planned_scope` behavior into a planner/request
  helper owned by the refresh module.
- Keep `RunStore::latest_successful_import` access explicit.
- Preserve all catch-up/full/freshness tests.

Why second:

- Scope planning is easy to regress and deserves a narrower test surface.
- It separates policy preparation from collector execution.

### Chunk 3: Extract Execution Flow

Scope:

- Move `execute`, `execute_open_refresh`, `persist`, and
  `reconcile_collection` into `execution.rs`.
- Keep `RefreshCoordinator` as the public facade that submits work and delegates
  execution.
- Keep terminal cleanup paths explicit; do not hide them behind a generic error
  handler.

Why third:

- This is the highest-risk chunk. Earlier extractions should make it smaller and
  easier to review.

### Chunk 4: Move Tests And Fakes

Scope:

- Move the large `#[cfg(test)]` module to `refresh/tests.rs`.
- Keep test names and behavior coverage intact.
- Split only if it improves navigation; do not over-abstract test builders.

Why fourth:

- Moving tests before production internals settle can cause churn.
- After chunks 1-3, tests can import stable internal helpers directly.

### Chunk 5: Harness Guardrails If Needed

Scope:

- Add an architecture harness check only if the refactor reveals a repeatable
  boundary mistake.
- Possible guardrail: prevent collector-specific source lists from being
  duplicated outside `refresh/target.rs`.

Why last:

- Harness rules should encode proven patterns, not speculative structure.

## Risks

- Terminalization regressions are the main risk. Failure paths are more
  important than happy path readability.
- Over-extracting can make the refresh flow harder to read. Prefer a few modules
  with cohesive responsibilities over many tiny files.
- Generic "pipeline" abstractions would be harmful here. The refresh flow has
  concrete product semantics and should remain explicit.
- Moving tests without improving production structure only hides the problem.

## Verification Strategy

Minimum checks per implementation chunk:

- `pnpm rust:fmt`
- `pnpm rust:test`
- `pnpm architecture:check`
- `pnpm verify:fast`

For the execution-flow chunk, also run the specific Rust coordinator tests during
iteration:

```sh
cargo test --manifest-path src-tauri/Cargo.toml application::refresh::coordinator::tests::
```

Full final gate before merge:

- `pnpm verify`

Runtime evidence is not required unless the public coordinator API, IPC wiring,
or scheduler behavior changes.

## Open Questions

- Should `RefreshEventSink` and `BudgetEvaluationRunner` stay in
  `coordinator.rs` or move to `hooks.rs`?
  - Recommendation: move them only if chunk 1-3 still leaves coordinator
    navigation noisy.
- Should refresh targets be static or generated from collector descriptors?
  - Recommendation: keep static for now. It is explicit and matches the current
    reviewed source support matrix.
- Should cancellation be handled in this refactor?
  - Recommendation: no. Cancellation is product behavior, not cleanup. Keep this
    refactor behavior-preserving.

## Success Criteria

- `RefreshCoordinator` remains the public facade.
- The coordinator file becomes small enough to review request submission and
  state handling without scrolling through execution internals and test fakes.
- Refresh target support is reviewable in one small module.
- Failure terminalization behavior remains explicit and tested.
- No application port contracts change.
- No collector contracts change.
- All existing coordinator tests continue to pass.
