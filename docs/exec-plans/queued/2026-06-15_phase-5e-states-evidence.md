# 2026-06-15 Phase 5E UI States And Runtime Evidence

## Objective

Complete exceptional overview states and prove the first persisted-data workflow
through the desktop boundary and responsive visual evidence.

## Dependency

Phase 5D provides the populated overview.

## Acceptance Criteria

- Loading, empty, stale, partial, query failure, refresh progress, and refresh
  failure are visibly distinct.
- Refresh failure preserves and labels last successful data.
- Empty state provides the relevant refresh action without explanatory marketing.
- Errors are user-safe and retryable where supported.
- Manual refresh updates the overview after invalidation and re-query.
- Desktop and compact screenshots have no overflow, overlap, or clipped text.
- Desktop evidence proves startup, query, refresh, invalidation, and updated data.

## Non-Goals

- Phase 6 views, tray scheduling, or final brand polish

## Risk Class

high

## Impact Areas

- Overview state presentation
- Retry and stale-data behavior
- Desktop workflow harness
- Responsive visual evidence
- Phase 5 documentation

## Design Review

- Complexity introduced: explicit state rendering and one critical workflow.
- Decisions hidden: a feature-local state derivation avoids scattered conditions.
- Interface depth: state components receive display-ready values and actions.
- Special cases: stale plus refresh failure, partial plus unavailable cost.
- Abstraction needed now: derive presentation state only if it removes repetition.
- Existing ownership: overview feature and desktop evidence harness absorb it.

## Checklist

- [ ] Implement loading and empty states.
- [ ] Implement stale, partial, query-error, and refresh-error states.
- [ ] Preserve and label prior data after refresh failure.
- [ ] Add valid retry and refresh interactions.
- [ ] Test every distinct visible state.
- [ ] Extend desktop evidence through load, refresh, invalidation, and re-query.
- [ ] Capture and inspect desktop and compact screenshots.
- [ ] Run pnpm verify, desktop evidence, and relevant end-to-end tests.
- [ ] Complete and archive the Phase 5 overview.

## Test Plan

- Behavior: state selection, accessible status, prior-data preservation, retry,
  and end-to-end refresh updates.
- Lowest stable layer: React Testing Library plus desktop runtime workflow.
- Failure paths: initial query, refresh with cached data, malformed event, partial
  result, unavailable cost.
- Fixtures: typed responses, fake sidecar modes, temporary real SQLite.
- Runtime evidence: desktop and compact screenshots plus desktop workflow test.
- Commands: frontend tests, pnpm test:e2e where applicable,
  pnpm evidence:desktop, and pnpm verify.

## Decisions

- Refine after the populated interface exists.

## Verification

- Command: pnpm verify
- Outcome: queued; not run.

## Runtime Evidence

- Required before Phase 5 completes.

## Follow-Up Debt

- None.
