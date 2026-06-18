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

- [x] Implement loading and empty states.
- [x] Implement stale, partial, query-error, and refresh-error states.
- [x] Preserve and label prior data after refresh failure.
- [x] Add valid retry and refresh interactions.
- [x] Test every distinct visible state.
- [x] Extend desktop evidence through load, refresh, invalidation, and re-query.
- [x] Capture and inspect desktop and compact screenshots.
- [x] Run verification and relevant end-to-end tests.
- [x] Complete and archive the Phase 5 overview.

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

- Command: `pnpm test src/features/overview src/app/App.test.tsx src/ipc/client.test.ts src/lib/format/index.test.ts`
- Outcome: passed on 2026-06-17; 34 tests passed.
- Command: `pnpm test:e2e`
- Outcome: passed on 2026-06-17; 8 tests passed across Desktop and Compact.
- Command: `pnpm lint`
- Outcome: passed on 2026-06-17 with existing complexity warnings.
- Command: `pnpm typecheck`
- Outcome: passed on 2026-06-17.

## Runtime Evidence

- Desktop and compact screenshots were captured by Playwright for populated,
  empty, and error states. The evidence suite also proves refresh progress,
  `data-invalidated`, and authoritative re-query.

## Follow-Up Debt

- Phase 5E required remediation on 2026-06-17 because the first evidence suite
  used stale IPC fixtures and did not prove invalidation-driven re-query.
