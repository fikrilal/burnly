# 2026-07-04 Test Architecture 01 Bootstrap Support

## Status

Completed on July 5, 2026.

## Objective

Move bootstrap test setup and fakes out of the production composition file while
preserving the same bootstrap behavior coverage.

## Acceptance Criteria

- `src-tauri/src/bootstrap/test_support.rs` owns test-only bootstrap fixtures and
  fakes behind `#[cfg(test)]`.
- `src-tauri/src/bootstrap.rs` keeps the production bootstrap API unchanged.
- Startup, recovery, IPC bridge, and composed refresh tests retain equivalent
  behavior coverage.
- No production visibility is widened for test convenience.
- No startup order, Tauri plugin wiring, IPC behavior, refresh behavior, or
  persistence behavior changes.

## Risk Class

`medium`

## Impact Areas

- `src-tauri/src/bootstrap.rs`
- `src-tauri/src/bootstrap/test_support.rs`
- Bootstrap runtime tests
- Startup/recovery diagnostics tests
- Tauri IPC bridge tests

## Design Review

- What complexity is being introduced?
  - One bootstrap-owned test support module.
- Which decisions are hidden inside the owning module?
  - Only fixture setup and fake boundary behavior.
- Is each new interface simpler than its implementation?
  - Yes if tests call focused fixtures instead of repeating setup blocks.
- What special cases exist, and can the design eliminate them?
  - Tauri app setup and fake sidecar behavior are bootstrap-specific and should
    not become global test helpers.
- Why is each new abstraction needed now?
  - `bootstrap.rs` remains a composition hotspot and test setup adds review
    noise.
- Can an existing module absorb this responsibility cleanly?
  - Yes, `bootstrap/test_support.rs` can absorb it under `#[cfg(test)]`.

## Checklist

- [x] Inspect bootstrap tests and list repeated setup/fakes.
- [x] Add `src-tauri/src/bootstrap/test_support.rs`.
- [x] Move startup database fixtures that are only used by bootstrap tests.
- [x] Move fake sidecar/process setup used by composed refresh tests.
- [x] Move runtime app or command bridge setup helpers where appropriate.
- [x] Keep behavioral assertions in the tests unless a helper name makes the
      assertion clearer.
- [x] Confirm production builds do not expose test support.
- [x] Run focused bootstrap tests.
- [x] Run architecture checks.
- [x] Record verification outcomes before completion.

## Test Plan

- Behavior and invariants to prove:
  - Startup database migration, seed, health, and recovery behavior still works.
  - Recovery diagnostics still follow the same success/failure semantics.
  - Tauri command bridge tests still execute composed refresh behavior.
  - Packaged resource and sidecar path tests remain equivalent.
- Lowest stable test layer:
  - Bootstrap Rust module tests.
- Failure paths:
  - startup database failure
  - recovery failure
  - diagnostic write failure
  - fake sidecar/process failure
- Fixtures or fakes:
  - Bootstrap-owned test fixtures and fake sidecars.
- Runtime or platform evidence:
  - Not required if only test support moves.
- Relevant commands:
  - `cargo test --manifest-path src-tauri/Cargo.toml bootstrap --lib`
  - `pnpm rust:test`
  - `pnpm architecture:check`

## Decisions

- Keep bootstrap tests at the bootstrap boundary.
- Do not move these tests to `src-tauri/tests/` unless a crate-boundary behavior
  requires it.
- Do not introduce a service bag or dependency-injection abstraction.

## Verification

- Command: `cargo test --manifest-path src-tauri/Cargo.toml bootstrap --lib`
- Outcome: passed; 24 passed, 0 failed.
- Command: `pnpm rust:test`
- Outcome: passed; 365 passed, 0 failed, 1 ignored.
- Command: `pnpm architecture:check`
- Outcome: passed.
- Command: `pnpm rust:fmt`
- Outcome: passed after applying `pnpm rust:fmt:write`.

## Runtime Evidence

- Not required yet.

## Follow-Up Debt

- None.
