# 2026-06-19 Phase 9G Diagnostics Export Recovery Evidence

## Objective

Close Phase 9 by proving diagnostics, logs, history, export, deletion,
maintenance, and recovery work together safely.

## Acceptance Criteria

- Phase 9 overview exit criteria are all satisfied.
- Diagnostics, logs, history, export, delete-history, maintenance, and recovery
  UI states have automated coverage.
- Runtime evidence covers the stable desktop workflows introduced in Phase 9.
- `pnpm verify` and `pnpm verify:runtime` pass or limitations are documented
  with concrete environment details.
- Phase 9 plans are moved to completed and the overview records final decisions,
  evidence, and follow-up debt.

## Risk Class

`medium`

This chunk should not introduce core behavior, but it verifies multiple risky
side effects and documentation completeness.

## Impact Areas

- Playwright evidence
- Desktop runtime harness
- Execution plans
- Final Phase 9 verification

## Design Review

- What complexity is being introduced? Cross-feature evidence and final phase
  reconciliation.
- Which decisions are hidden inside the owning module? No new product decisions
  should be introduced here.
- Is each new interface simpler than its implementation? Evidence should use
  existing commands and UI flows.
- What special cases exist, and can the design eliminate them? Platform-specific
  reveal/export/recovery limitations must be documented explicitly.
- Why is each new abstraction needed now? Avoid adding abstractions in this
  chunk unless repeated evidence gaps require harness support.
- Can an existing module absorb this responsibility cleanly? Runtime harness and
  exec-plan docs should absorb this work.

## Checklist

- [ ] Expand stable Playwright evidence for Phase 9 states.
- [ ] Run focused checks for any final harness changes.
- [ ] Run `pnpm verify`.
- [ ] Run `pnpm verify:runtime`.
- [ ] Record environment, limitations, and outcomes.
- [ ] Mark Phase 9 overview complete.
- [ ] Move completed plans and identify Phase 10 follow-up debt.

## Test Plan

- Behavior and invariants to prove: end-to-end workflows preserve privacy and
  side-effect safety.
- Lowest stable test layer: existing tests from 9A-9F plus Playwright runtime
  evidence.
- Failure paths: documented from previous chunks; no new behavior expected.
- Fixtures or fakes: desktop evidence fixtures for success/empty/error states.
- Runtime or platform evidence: required on current desktop environment.
- Relevant commands: `pnpm verify`, `pnpm verify:runtime`.

## Decisions

- Phase 9G is evidence-only unless a blocking bug is found.

## Verification

- Command: `pnpm verify`
- Outcome: not run yet

## Runtime Evidence

- Required before completion.

## Follow-Up Debt

- Phase 10 owns cross-platform release-matrix validation.
