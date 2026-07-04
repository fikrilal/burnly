# 2026-07-04 Refresh Coordinator 01 Outcome And Target Helpers

## Objective

Extract pure outcome bookkeeping and refresh target helper code from
`src-tauri/src/application/refresh/coordinator.rs` into narrow internal modules,
without changing refresh behavior.

## Acceptance Criteria

- Outcome mapping and aggregation no longer live in `coordinator.rs`.
- Refresh target catalog and target-level helper functions no longer live in
  `coordinator.rs`.
- `RefreshCoordinator` behavior and public API remain unchanged.
- Supported source/projection pairs remain exactly the same.
- Existing coordinator tests continue to pass.
- Add focused tests for any extracted pure helper that is not already directly
  covered.

## Risk Class

`low`

## Impact Areas

- `src-tauri/src/application/refresh/coordinator.rs`
- `src-tauri/src/application/refresh/outcome.rs`
- `src-tauri/src/application/refresh/target.rs`
- `src-tauri/src/application/refresh/mod.rs`

## Design Review

- What complexity is being introduced?
  - Two small internal modules for behavior that is already cohesive and mostly
    pure.
- Which decisions are hidden inside the owning module?
  - Outcome conversion and target catalog details are hidden behind refresh
    module internals.
- Is each new interface simpler than its implementation?
  - Callers should ask target/outcome helpers for values instead of manually
    matching source/projection/outcome details.
- What special cases exist, and can the design eliminate them?
  - Daily targets need timezones and sessions do not. `target.rs` should own
    that distinction.
- Why is each new abstraction needed now?
  - It shrinks the coordinator before touching side-effect-heavy execution flow.
- Can an existing module absorb this responsibility cleanly?
  - `planner.rs` owns policy selection, not supported target catalog or
    collection ID/import timezone helpers.

## Checklist

- [x] Create `src-tauri/src/application/refresh/outcome.rs`.
- [x] Move `RunOutcome`, `TargetRunAccumulator`, `ExecutionResult`,
      `ExecutionFailure`, `run_error`, and `clamp_count` if visibility remains
      clean.
- [x] Create `src-tauri/src/application/refresh/target.rs`.
- [x] Move `RefreshTarget`, `refresh_targets`, `projection_label`,
      `import_timezone`, `local_date`, and `records_seen`.
- [x] Keep extracted items `pub(super)` or narrower where possible.
- [x] Update imports in `coordinator.rs`.
- [x] Add focused unit tests for outcome aggregation and target timezone rules
      if existing tests do not cover them at the lowest stable layer.
- [x] Run focused verification and record outcomes before completion.

## Test Plan

- Behavior and invariants to prove:
  - Complete/empty collection outcomes still map to successful run outcomes.
  - Partial collection outcomes still map to partial run outcomes.
  - Aggregate target outcomes still produce succeeded, partial, or failed
    refresh outcomes correctly.
  - Target list still includes all current daily/session pairs.
  - Daily imports still carry aggregation timezone.
  - Session imports still omit aggregation timezone.
  - Record counts still clamp to `u32::MAX`.
- Lowest stable test layer:
  - New pure unit tests in `outcome.rs` and `target.rs`.
  - Existing coordinator tests for integration coverage.
- Failure paths:
  - invalid timezone in `local_date`
  - oversized record count clamp
- Fixtures or fakes:
  - Existing collection result test helpers may be reused if needed.
- Runtime or platform evidence:
  - Not required.
- Relevant commands:
  - `cargo test --manifest-path src-tauri/Cargo.toml application::refresh::`
  - `pnpm rust:fmt`
  - `pnpm rust:test`
  - `pnpm verify:fast`

## Decisions

- Do not change the source list.
- Do not generate refresh targets from collector descriptors in this chunk.
- Prefer `pub(super)` visibility for extracted helpers.

## Verification

- `cargo fmt --manifest-path src-tauri/Cargo.toml` — applied Rust formatting
- `cargo test --manifest-path src-tauri/Cargo.toml application::refresh::` —
  passed: 37 passed, 0 failed
- `pnpm rust:test` — passed: 332 passed, 0 failed, 1 ignored
- `pnpm verify:fast` — passed; ESLint reported existing warnings only

## Runtime Evidence

- Not required.

## Follow-Up Debt

- None expected.
