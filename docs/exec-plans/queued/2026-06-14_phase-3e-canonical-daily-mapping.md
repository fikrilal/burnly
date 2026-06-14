# 2026-06-14 Phase 3E Canonical Daily Mapping

## Objective

Map decoded Claude daily rows into validated Burnly canonical daily candidates
without process execution or persistence.

## Dependency

Phase 3D provides typed, profile-validated Claude daily rows.

## Acceptance Criteria

- Mapping produces application-owned canonical daily candidates.
- Collector-reported total tokens remain authoritative.
- Input/output/cache token breakdowns are optional and never fabricated.
- Unclassified tokens are calculated only when supported values permit it.
- Cost currency, status, and provenance follow the approved Claude daily profile.
- Positive usage with an unexplained zero cost becomes unavailable unless the
  profile explicitly permits genuine zero cost.
- Dates and stable source keys are deterministic.
- Invalid totals, negative values, impossible breakdowns, and invalid cost values
  fail with structured validation errors.
- Mapper code remains inside the adapter and does not write SQLite.

## Non-Goals

- Import-run records or reconciliation
- Missing/removed lifecycle behavior
- Model/session persistence
- UI formatting

## Risk Class

`high`

## Impact Areas

- Claude daily mapper
- Canonical candidate invariants
- Cost and token provenance
- Mapping fixture expectations

## Design Review

- Complexity introduced: translating collector semantics into Burnly invariants.
- Decisions hidden: the mapper owns collector-specific zero/null interpretation
  and provenance assignment.
- Interface depth: decoded rows become canonical candidates or validation failures.
- Special cases: nullable unsupported values eliminate fake zero-value branches.
- Abstraction needed now: mapping separates external representation from canonical
  product meaning.
- Existing ownership: the `ccusage` Claude adapter can own this source-specific
  translation while application/domain types enforce shared invariants.

## Checklist

- [ ] Define deterministic daily source-key construction.
- [ ] Map date, authoritative totals, optional components, and model breakdowns.
- [ ] Implement unclassified-token rules.
- [ ] Implement cost status, currency, and provenance rules.
- [ ] Validate non-negative values and aggregate/breakdown consistency.
- [ ] Add mapping tests over sanitized decoded fixtures.
- [ ] Prove no collector envelope types cross the application boundary.
- [ ] Run `pnpm verify` and activate Phase 3F.

## Test Plan

- Behavior and invariants to prove: deterministic identity, authoritative totals,
  null preservation, unclassified calculations, cost safeguards, and provenance.
- Lowest stable test layer: mapper and canonical value unit tests.
- Failure paths: negative tokens/cost, component sum above total, invalid date,
  unsupported currency, and ambiguous zero cost.
- Fixtures or fakes: decoded Claude rows derived from sanitized fixtures.
- Runtime or platform evidence: not required.
- Relevant commands: `cargo test`, `pnpm architecture:check`, `pnpm verify`.

## Decisions

- Canonical totals are never reconstructed from optional model breakdowns.
- Mapping does not silently repair incompatible collector output.

## Verification

- Command: `pnpm verify`
- Outcome: queued; not run yet.

## Runtime Evidence

- Not required.

## Follow-Up Debt

- None.
