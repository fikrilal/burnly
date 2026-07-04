# 2026-07-04 Refresh Coordinator 05 Harness Guardrails

## Objective

Add architecture harness guardrails for the refactored refresh coordinator only
if the implementation reveals a repeatable boundary mistake worth encoding.

## Acceptance Criteria

- No harness rule is added unless it protects an observed architectural
  boundary.
- Any new harness rule has self-tests.
- Existing codebase passes the new harness rule.
- Harness error messages are actionable.
- If no useful guardrail is identified, record that decision and complete this
  plan without code changes.

## Risk Class

`low`

## Impact Areas

- `scripts/harness/check-architecture.mjs`
- `docs/engineering/harness-engineering-design.md`
- Possibly `src-tauri/src/application/refresh/` if a rule requires small
  compliant adjustments

## Design Review

- What complexity is being introduced?
  - Potentially one architecture rule and its self-tests.
- Which decisions are hidden inside the owning module?
  - Harness rules encode architecture decisions, not implementation preferences.
- Is each new interface simpler than its implementation?
  - A rule should make violations clear without requiring maintainers to know
    the entire refresh refactor history.
- What special cases exist, and can the design eliminate them?
  - Refresh targets may be duplicated accidentally as new sources are added.
    Only guard this if duplication is observed during implementation.
- Why is each new abstraction needed now?
  - Harness should be updated when the same mistake repeats, per project rules.
- Can an existing module absorb this responsibility cleanly?
  - Existing architecture harness is the right owner for boundary checks.

## Checklist

- [x] Review chunks 01-04 implementation notes for repeated boundary mistakes.
- [x] Decide whether a harness guardrail is justified.
- [x] If justified, add the smallest architecture harness rule. Not needed.
- [x] Add self-test coverage for pass and fail cases. Not needed.
- [x] Update harness engineering docs if a new rule is added. Not needed.
- [x] Run architecture self-test and architecture check.
- [x] Run fast verification.
- [x] Record verification outcomes before completion.

## Test Plan

- Behavior and invariants to prove if a rule is added:
  - Valid refactored refresh modules pass.
  - Simulated violation fails.
  - Error message points to the correct ownership boundary.
- Lowest stable test layer:
  - Harness self-test.
  - Real architecture check.
- Failure paths:
  - false positives against valid refresh code
  - false negatives for the intended boundary violation
- Fixtures or fakes:
  - Existing harness self-test cases.
- Runtime or platform evidence:
  - Not required.
- Relevant commands:
  - `pnpm architecture:test`
  - `pnpm architecture:check`
  - `pnpm verify:fast`

## Decisions

- Do not add speculative harness rules.
- It is acceptable for this chunk to complete as "no code change" if the
  refactor does not reveal a repeatable mistake.
- No new harness rule is justified for this refactor. Chunks 1-4 did not reveal
  a repeated architecture-boundary mistake. The only correction needed during
  test relocation was explicit test imports after removing `use super::*`.
- `RefreshEventSink`, `BudgetEvaluationRunner`, and `RefreshCoordinatorHooks`
  remain in `coordinator.rs` because they are part of the coordinator facade
  surface and do not currently justify a separate `hooks.rs` module.

## Verification

- Command: `pnpm architecture:test`
- Outcome: passed.
- Command: `pnpm architecture:check`
- Outcome: passed.
- Command: `pnpm verify:fast`
- Outcome: passed; ESLint warnings and duplication report remain non-fatal.
- Command: `pnpm verify`
- Outcome: passed; ESLint warnings and duplication report remain non-fatal.

## Runtime Evidence

- Not required.

## Follow-Up Debt

- None expected.
