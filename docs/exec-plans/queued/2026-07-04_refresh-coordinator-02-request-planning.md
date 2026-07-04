# 2026-07-04 Refresh Coordinator 02 Request Planning

## Objective

Extract refresh target scope planning and collection request construction from
`RefreshCoordinator` into an internal request-planning module while preserving
full, catch-up, and freshness behavior.

## Acceptance Criteria

- `coordinator.rs` no longer owns `collection_request` and `planned_scope`
  implementation details.
- Full refresh still uses `CollectionScope::Full`.
- Catch-up refresh still delegates to `RefreshPolicyPlanner` in catch-up mode.
- Freshness refresh still delegates to `RefreshPolicyPlanner` in freshness mode.
- Daily requests still include aggregation timezone.
- Session requests still omit aggregation timezone.
- Previous successful import lookup remains explicit and testable.
- Existing coordinator tests continue to pass.

## Risk Class

`medium`

## Impact Areas

- `src-tauri/src/application/refresh/coordinator.rs`
- `src-tauri/src/application/refresh/request_plan.rs` or equivalent
- `src-tauri/src/application/refresh/target.rs`
- `src-tauri/src/application/refresh/planner.rs`
- Coordinator tests

## Design Review

- What complexity is being introduced?
  - A request-planning helper separates deterministic planning from worker
    lifecycle and persistence.
- Which decisions are hidden inside the owning module?
  - Translation from refresh policy plus previous import state into collection
    scope.
  - Translation from target identity into a `CollectionRequest`.
- Is each new interface simpler than its implementation?
  - The coordinator should pass target, job ID, time, timezone, and policy, then
    receive a planned request or explicit execution failure.
- What special cases exist, and can the design eliminate them?
  - Daily/session timezone differences remain, but become localized in planning
    code.
- Why is each new abstraction needed now?
  - Scope policy is product-critical and should be reviewable without reading
    persistence terminalization code.
- Can an existing module absorb this responsibility cleanly?
  - `planner.rs` only decides scopes. It should not know collection IDs,
    `RunStore`, or request construction.

## Checklist

- [ ] Create a request-planning internal module with a narrow public surface.
- [ ] Move `collection_request` behavior into the new module.
- [ ] Move `planned_scope` behavior into the new module.
- [ ] Keep `RunStore::latest_successful_import` dependency explicit, either by
      passing the store or by passing a narrow lookup closure/helper.
- [ ] Preserve existing error codes and summaries for invalid request,
      timezone, and import-state failures.
- [ ] Update coordinator execution flow to call the planner helper.
- [ ] Run focused coordinator tests for scope policy.
- [ ] Record verification outcomes before completion.

## Test Plan

- Behavior and invariants to prove:
  - Missing baseline uses full scope.
  - Catch-up after baseline uses incremental catch-up policy.
  - Freshness after baseline uses today-only policy.
  - Freshness without baseline uses full scope.
  - Invalid timezone still fails with the same stable failure code.
  - Invalid import lookup still fails with the same stable failure code.
  - Collection IDs remain deterministic and include job/source/projection.
- Lowest stable test layer:
  - Unit tests for the request-planning helper.
  - Existing coordinator tests for end-to-end refresh behavior.
- Failure paths:
  - invalid timezone
  - latest successful import store failure
  - invalid collection ID/request construction
- Fixtures or fakes:
  - Existing fake run store and collection request helpers.
- Runtime or platform evidence:
  - Not required.
- Relevant commands:
  - `cargo test --manifest-path src-tauri/Cargo.toml application::refresh::coordinator::tests::missing_baseline_uses_full_scope_for_collector_import_and_reconciliation`
  - `cargo test --manifest-path src-tauri/Cargo.toml application::refresh::coordinator::tests::manual_refresh_uses_incremental_catch_up_after_baseline`
  - `cargo test --manifest-path src-tauri/Cargo.toml application::refresh::coordinator::tests::freshness_refresh_uses_today_only_after_baseline`
  - `pnpm rust:fmt`
  - `pnpm rust:test`
  - `pnpm verify:fast`

## Decisions

- Keep `RefreshPolicyPlanner` as the scope policy owner.
- Do not change refresh policy semantics.
- Do not change collection ID shape.

## Verification

- Command: not run yet
- Outcome: queued plan only

## Runtime Evidence

- Not required.

## Follow-Up Debt

- None expected.
