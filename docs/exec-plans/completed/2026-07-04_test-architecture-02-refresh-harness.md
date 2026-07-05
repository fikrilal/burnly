# 2026-07-04 Test Architecture 02 Refresh Harness

## Status

Completed on July 5, 2026.

## Objective

Normalize refresh coordinator test setup into a small refresh-owned scenario
harness without hiding refresh-state assertions.

## Acceptance Criteria

- `src-tauri/src/application/refresh/test_support.rs` owns repeated refresh test
  setup.
- Refresh tests still clearly show the behavior being proved.
- Fake ports remain aligned with application-layer contracts.
- Helper methods do not perform hidden assertions as side effects.
- Refresh coordinator behavior coverage remains equivalent.

## Risk Class

`medium`

## Impact Areas

- `src-tauri/src/application/refresh/tests.rs`
- `src-tauri/src/application/refresh/test_support.rs`
- Refresh coordinator tests
- Application-layer fake ports

## Design Review

- What complexity is being introduced?
  - A refresh-owned scenario harness and small fixture builders.
- Which decisions are hidden inside the owning module?
  - Collector outcome setup, import outcome setup, and boring fake wiring.
- Is each new interface simpler than its implementation?
  - Yes if test bodies become shorter while assertions stay visible.
- What special cases exist, and can the design eliminate them?
  - Cancellation, partial failure, import failure, and diagnostics are real
    scenario variants and should stay explicit.
- Why is each new abstraction needed now?
  - `application/refresh/tests.rs` is the largest test file and has repeated
    setup mechanics.
- Can an existing module absorb this responsibility cleanly?
  - Yes, inside the refresh module.

## Checklist

- [x] Inspect repeated setup in `application/refresh/tests.rs`.
- [x] Add `src-tauri/src/application/refresh/test_support.rs`.
- [x] Introduce focused refresh scenario support helpers.
- [x] Introduce collector scenario fixtures for success, empty, failure, and
      cancellation where repeated today.
- [x] Introduce import outcome fixtures only for repeated mechanics.
- [x] Keep final run-state, import, and diagnostic assertions in test bodies.
- [x] Prefer table-driven cases only where one rule has multiple inputs.
- [x] Run focused refresh tests.
- [x] Run duplication report and architecture checks.
- [x] Record verification outcomes before completion.

## Test Plan

- Behavior and invariants to prove:
  - Refresh planning and collection requests are unchanged.
  - Run state transitions are unchanged.
  - Import outcomes and diagnostics are unchanged.
  - Partial failure and cancellation behavior remain covered.
- Lowest stable test layer:
  - Refresh application tests.
- Failure paths:
  - collector failure
  - all records rejected
  - import failure
  - cancellation
  - diagnostic write failure if currently covered
- Fixtures or fakes:
  - Refresh-owned fake collectors, importers, diagnostics, clock, and policy
    inputs.
- Runtime or platform evidence:
  - Not required for test support refactors.
- Relevant commands:
  - `cargo test --manifest-path src-tauri/Cargo.toml application::refresh --lib`
  - `pnpm rust:test`
  - `pnpm duplication:report`
  - `pnpm architecture:check`

## Decisions

- Keep refresh tests in the application layer.
- Do not change refresh policy or scheduler behavior.
- Do not hide assertions in fluent builder methods.

## Verification

- Command: `cargo test --manifest-path src-tauri/Cargo.toml application::refresh --lib`
- Outcome: passed; 43 passed, 0 failed.
- Command: `pnpm rust:test`
- Outcome: passed; 365 passed, 0 failed, 1 ignored.
- Command: `pnpm duplication:report`
- Outcome: passed as report-only; existing clone report remains non-fatal.
- Command: `pnpm architecture:check`
- Outcome: passed.
- Command: `pnpm rust:fmt`
- Outcome: passed after applying `pnpm rust:fmt:write`.

## Runtime Evidence

- Not required yet.

## Follow-Up Debt

- None.
