# 2026-07-04 Test Architecture 06 Harness Calibration

## Objective

Re-run test architecture checks after the support refactors and document any
intentional remaining duplication or deferred test-architecture debt.

## Acceptance Criteria

- Remaining duplication has an owner and rationale.
- Mechanical setup duplication is either removed or explicitly deferred.
- Intentional contract repetition is documented near the owning tests, harness,
  or audit.
- No new hard duplication gate is added until the baseline is stable.
- Full verification passes before the series is considered complete.

## Risk Class

`low`

## Impact Areas

- `docs/planning/_WIP/test-architecture-audit.md`
- `docs/exec-plans/`
- Duplication report output
- Harness checks if a repeated mistake needs enforcement

## Design Review

- What complexity is being introduced?
  - Documentation and possible harness notes only.
- Which decisions are hidden inside the owning module?
  - None.
- Is each new interface simpler than its implementation?
  - No new production interface.
- What special cases exist, and can the design eliminate them?
  - Some duplication may be intentional contract repetition. Do not DRY it away
    if it makes tests less clear.
- Why is each new abstraction needed now?
  - The series needs a final calibration pass after structural changes.
- Can an existing module absorb this responsibility cleanly?
  - Yes, docs and harness files can record the baseline.

## Checklist

- [ ] Run `pnpm duplication:report`.
- [ ] Inspect remaining repeated blocks.
- [ ] Classify each meaningful remaining duplication as intentional, deferred,
      or still worth extracting.
- [ ] Update the test architecture audit with implementation decisions or
      remaining debt.
- [ ] Update harness checks only if the same mistake is likely to repeat.
- [ ] Run the full local gate.
- [ ] Record verification outcomes before completion.

## Test Plan

- Behavior and invariants to prove:
  - All test-support refactors preserve the full suite.
  - Architecture checks still pass.
  - Duplication report is understood and documented.
- Lowest stable test layer:
  - Full local gate.
- Failure paths:
  - Not applicable.
- Fixtures or fakes:
  - Not applicable.
- Runtime or platform evidence:
  - Not required unless a prior chunk unexpectedly touched runtime behavior.
- Relevant commands:
  - `pnpm duplication:report`
  - `pnpm verify`

## Decisions

- Do not make duplication reporting fatal in this chunk.
- Do not chase repeated lines when repetition protects contract clarity.
- Do not use this chunk for unrelated refactors.

## Verification

- Command: `pnpm duplication:report`
- Outcome: not run yet
- Command: `pnpm verify`
- Outcome: not run yet

## Runtime Evidence

- Not required yet.

## Follow-Up Debt

- None.
