# 2026-06-20 Phase 10H Release-Candidate Evidence

## Objective

Close Phase 10 by building and smoke-testing release candidates across the
supported matrix and reconciling every release, security, and platform claim.

## Acceptance Criteria

- All Phase 10 overview criteria are satisfied with linked evidence.
- Release candidates build for every declared target and installer format.
- Installed first launch, migration, sidecar import, refresh, tray,
  close/reopen, export, recovery, and quit workflows pass.
- Security, signing/update policy, performance budgets, and known limitations
  are documented.
- Release artifacts and checksums are traceable to the tested commit.
- Phase 10 plans move to completed with no unsupported platform claims.

## Risk Class

`high`

## Impact Areas

- Release-candidate build matrix
- Installed smoke tests
- Runtime evidence and artifact inventory
- Release checklist and documentation
- Phase 10 closure

## Design Review

- Complexity introduced: evidence aggregation only; no new product behavior
  should be introduced.
- Owning layers: CI produces artifacts; platform smoke harness records outcomes.
- Interface depth: evidence invokes existing workflows and public commands.
- Special cases: unavailable hardware, signing credentials, desktop-specific
  tray behavior, and platform installer limitations.
- Blocking defects return to the owning earlier chunk.
- Avoid one-off release abstractions unless evidence is otherwise irreproducible.

## Checklist

- [ ] Build final candidate artifacts from a clean tagged commit.
- [ ] Verify artifact identity, checksums, signatures, and provenance.
- [ ] Run installed smoke checklist on every supported environment.
- [ ] Run packaged migrations and sidecar execution.
- [ ] Run critical Playwright and performance evidence.
- [ ] Record limitations without extrapolating unsupported claims.
- [ ] Complete release checklist and Phase 10 overview.

## Test Plan

- Behavior and invariants to prove: distributed artifacts match source,
  preserve user data, and support declared workflows.
- Lowest stable test layer: installed release-candidate smoke tests.
- Failure paths: install/upgrade failure, sidecar mismatch, migration failure,
  missing tray, invalid signature, recovery failure, and performance regression.
- Fixtures or fakes: disposable profiles and old-schema databases.
- Runtime or platform evidence: mandatory for every supported target and
  declared desktop environment.
- Relevant commands: `pnpm verify`, `pnpm verify:runtime`, release workflows,
  platform smoke scripts.

## Decisions

- A platform is supported only when the final candidate was installed and
  tested there.

## Verification

- Command: `pnpm verify`
- Outcome: not run yet

## Runtime Evidence

- Required before completion.

## Follow-Up Debt

- None.
