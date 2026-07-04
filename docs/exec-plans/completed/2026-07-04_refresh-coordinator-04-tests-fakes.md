# 2026-07-04 Refresh Coordinator 04 Tests And Fakes

## Objective

Move the large coordinator test module and fakes out of
`coordinator.rs` after production refresh internals have been split, preserving
test coverage and names where practical.

## Acceptance Criteria

- Coordinator production file no longer contains the full fake/test harness.
- Existing coordinator behavior tests remain present and pass.
- Test helpers stay close to refresh tests and do not leak into production
  modules.
- Test movement does not require broad visibility widening in production code.
- No production behavior changes.

## Risk Class

`low`

## Impact Areas

- `src-tauri/src/application/refresh/coordinator.rs`
- `src-tauri/src/application/refresh/tests.rs`
- `src-tauri/src/application/refresh/mod.rs`
- Possibly extracted helper module tests

## Design Review

- What complexity is being introduced?
  - A test-only module boundary for fakes and integration-style behavior tests.
- Which decisions are hidden inside the owning module?
  - Test fixture setup and fake storage behavior stay in test code.
- Is each new interface simpler than its implementation?
  - Production modules should not gain public APIs only for moved tests.
- What special cases exist, and can the design eliminate them?
  - Some tests may need access to internal helpers. Prefer colocated module tests
    over broad `pub(crate)` visibility.
- Why is each new abstraction needed now?
  - Once production internals are extracted, test code should stop dominating
    coordinator navigation.
- Can an existing module absorb this responsibility cleanly?
  - A private `tests.rs` module under `refresh/` is the narrowest owner.

## Checklist

- [x] Create `src-tauri/src/application/refresh/tests.rs` behind `#[cfg(test)]`.
- [x] Move coordinator tests and fakes from `coordinator.rs`.
- [x] Keep test names stable unless module paths naturally change.
- [x] Avoid production visibility widening solely for tests.
- [x] Move helper tests into their owning modules when they are pure helper
      behavior.
- [x] Run all refresh tests and full Rust tests.
- [x] Record verification outcomes before completion.

## Test Plan

- Behavior and invariants to prove:
  - All existing coordinator tests still run.
  - Test module move does not reduce coverage of failure paths.
  - Production module visibility remains narrow.
- Lowest stable test layer:
  - Rust unit tests.
- Failure paths:
  - accidental test omission
  - visibility widening that weakens architecture boundaries
- Fixtures or fakes:
  - Existing fake run store, usage store, collector, clock, event sink, and
    budget evaluator.
- Runtime or platform evidence:
  - Not required.
- Relevant commands:
  - `cargo test --manifest-path src-tauri/Cargo.toml application::refresh::`
  - `pnpm rust:fmt`
  - `pnpm rust:test`
  - `pnpm verify:fast`

## Decisions

- Do not refactor test fixture design aggressively in this chunk.
- Preserve behavior tests over reducing test line count.

## Verification

- Command: `cargo fmt --manifest-path src-tauri/Cargo.toml`
- Outcome: passed.
- Command: `cargo test --manifest-path src-tauri/Cargo.toml application::refresh:: --quiet`
- Outcome: passed; 43 refresh tests passed.
- Command: `pnpm rust:test`
- Outcome: passed; 338 Rust tests passed, 1 ignored.
- Command: `pnpm verify:fast`
- Outcome: passed; ESLint warnings and duplication report remain non-fatal.

## Runtime Evidence

- Not required.

## Follow-Up Debt

- Consider splitting test fakes only if they become shared by multiple refresh
  modules.
