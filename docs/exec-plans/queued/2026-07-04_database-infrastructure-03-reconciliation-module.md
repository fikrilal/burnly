# 2026-07-04 Database Infrastructure 03 Reconciliation Module

## Objective

Split `src-tauri/src/infrastructure/database/reconciliation_store.rs` into a
private `database/reconciliation/` module organized by application port and
transaction flow, while preserving `SqliteReconciliationStore` behavior.

## Acceptance Criteria

- `SqliteReconciliationStore` remains the concrete type exported by
  `database/mod.rs`.
- `RunStore` implementation and run lifecycle SQL move to a run-focused module.
- Daily reconciliation transaction and SQL move to a daily-focused module.
- Session reconciliation transaction and SQL move to a session-focused module.
- Shared identity resolution and value mapping helpers move to narrow private
  modules.
- Daily and session reconciliation each still execute inside exactly one
  transaction per request.
- Existing behavior tests are preserved and may be split only when that improves
  navigation.
- No schema, SQL semantics, or application port changes.

## Risk Class

`medium`

## Impact Areas

- `src-tauri/src/infrastructure/database/reconciliation_store.rs`
- `src-tauri/src/infrastructure/database/reconciliation/`
- `src-tauri/src/infrastructure/database/mod.rs`
- Reconciliation store tests

## Design Review

- What complexity is being introduced?
  - Several private modules replace one large file. This adds module boundaries
    but reduces review cost.
- Which decisions are hidden inside the owning module?
  - Run lifecycle SQL stays behind `RunStore`.
  - Daily/session reconciliation SQL stays behind `UsageStore`.
  - Identity and mapping details stay private to database reconciliation.
- Is each new interface simpler than its implementation?
  - Internal helper interfaces should pass transaction/context values and hide
    SQL details. No new application interface should be added.
- What special cases exist, and can the design eliminate them?
  - Run lifecycle and usage reconciliation are currently coupled by file
    placement. Splitting by port/transaction clarifies ownership.
- Why is each new abstraction needed now?
  - The current file is the largest and highest-risk database hotspot.
- Can an existing module absorb this responsibility cleanly?
  - The existing module cannot stay as one file without continuing the review
    problem.

## Checklist

- [ ] Create `database/reconciliation/mod.rs`.
- [ ] Create a shared store module for `SqliteReconciliationStore` and database
      locking helpers.
- [ ] Move `RunStore` implementation and helpers into a run-focused module.
- [ ] Move daily reconciliation transaction and helpers into a daily-focused
      module.
- [ ] Move session reconciliation transaction and helpers into a session-focused
      module.
- [ ] Move source model/project identity helpers into a private identity module.
- [ ] Move token/cost/status/scope/outcome mapping helpers into a private
      mapping module.
- [ ] Preserve or split tests without weakening coverage.
- [ ] Run focused Rust formatting and tests.
- [ ] Run the full gate before completing this chunk.
- [ ] Record verification outcomes before completing the plan.

## Test Plan

- Behavior and invariants to prove:
  - Source resolution remains idempotent.
  - Refresh/import run lifecycle reaches terminal states.
  - Duplicate job key behavior is unchanged.
  - Latest successful import lookup is unchanged.
  - Daily reconciliation remains idempotent and replaces model breakdowns.
  - Session reconciliation stores non-reversible project identity.
  - Failed writes roll back without partial state.
  - Absence lifecycle behavior is unchanged for full/partial/incremental scopes.
- Lowest stable test layer:
  - Existing `SqliteReconciliationStore` Rust unit tests.
- Failure paths:
  - Duplicate job keys.
  - Missing run completion.
  - Value out of range.
  - Transaction rollback on write failure.
- Fixtures or fakes:
  - Existing `TestDatabase` and store-local seed helpers.
- Runtime or platform evidence:
  - Not required.
- Relevant commands:
  - `pnpm rust:fmt`
  - `pnpm rust:test`
  - `pnpm verify:fast`
  - `pnpm verify`

## Decisions

- Do not split by table.
- Do not introduce one repository per table.
- Keep one `SqliteReconciliationStore` facade unless a later review approves a
  contract change.

## Verification

- Command: not run yet
- Outcome: queued plan only

## Runtime Evidence

- Not required yet.

## Follow-Up Debt

- Consider extracting migration tests only after this chunk if database
  navigation still feels noisy.
