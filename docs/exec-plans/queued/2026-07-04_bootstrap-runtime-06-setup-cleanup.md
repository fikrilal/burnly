# 2026-07-04 Bootstrap Runtime 06 Setup Cleanup

## Objective

Clean up the remaining bootstrap setup facade after responsibility modules are
extracted, preserving behavior while making startup order and managed Tauri
state easier to review.

## Acceptance Criteria

- `setup_runtime` reads as an explicit startup/install sequence.
- Tauri-managed state registration remains concrete and compatible with IPC
  handlers.
- Any remaining test fakes/helpers are moved only if visibility stays clean.
- `StartupError` moves to a sibling module only if it improves readability after
  the other extractions.
- The bootstrap audit is updated with implementation outcomes.
- Full verification passes.

## Risk Class

`medium`

## Impact Areas

- `src-tauri/src/bootstrap.rs`
- `src-tauri/src/bootstrap/`
- bootstrap tests
- `docs/planning/_WIP/bootstrap-runtime-composition-audit.md`

## Design Review

- What complexity is being introduced?
  - Ideally none. This chunk removes leftover composition noise after prior
    extraction chunks.
- Which decisions are hidden inside the owning module?
  - Startup sequence remains visible in `setup_runtime`.
- Is each new interface simpler than its implementation?
  - Yes if any final helper names reflect concrete setup steps.
- What special cases exist, and can the design eliminate them?
  - Tauri-managed state must remain concrete by type. Do not replace it with a
    service bag.
- Why is each new abstraction needed now?
  - Final cleanup is only meaningful after stable responsibility modules exist.
- Can an existing module absorb this responsibility cleanly?
  - This chunk should mostly simplify the remaining composition facade.

## Checklist

- [ ] Review remaining `bootstrap.rs` responsibilities after chunks 01-05.
- [ ] Group or rename setup steps for readability.
- [ ] Move test support only if it reduces production-file noise without
      widening production visibility.
- [ ] Move `StartupError` only if it clearly improves readability.
- [ ] Update audit implementation outcome.
- [ ] Run focused bootstrap tests.
- [ ] Run full local verification.
- [ ] Record verification outcomes before completion.

## Test Plan

- Behavior and invariants to prove:
  - App startup managed state remains compatible with all IPC commands.
  - Bootstrap IPC bridge tests pass.
  - Composed refresh integration test passes.
  - Full repo verification passes.
- Lowest stable test layer:
  - Existing bootstrap Tauri IPC integration tests.
  - Full repo gate.
- Failure paths:
  - setup failures still map to stable `StartupErrorKind`
  - update unavailable runtime remains stable in tests
  - composed refresh reaches terminal state
- Fixtures or fakes:
  - Existing Tauri test mocks.
  - Existing fake ccusage sidecar.
- Runtime or platform evidence:
  - Required only if startup/tray/event behavior changes.
- Relevant commands:
  - `cargo test --manifest-path src-tauri/Cargo.toml bootstrap::`
  - `pnpm verify`

## Decisions

- Do not introduce a `RuntimeServices` bag.
- Keep startup order explicit.
- Treat error-module movement as optional cleanup, not required scope.

## Verification

- Command: not run yet
- Outcome: queued plan only

## Runtime Evidence

- Not required unless behavior changes.

## Follow-Up Debt

- Consider a harness rule only if future changes reintroduce Tauri imports into
  application/domain modules or runtime composition leaks across boundaries.
