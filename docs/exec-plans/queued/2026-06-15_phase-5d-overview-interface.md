# 2026-06-15 Phase 5D Overview Interface

## Objective

Build the populated first overview using the Phase 5C data boundary and a small
domain-appropriate visual foundation.

## Dependency

Phase 5C provides stable overview and refresh interfaces.

## Acceptance Criteria

- The first screen is the usable overview, not a landing page.
- It shows token total, estimated cost, source breakdown, refresh state, and
  manual refresh.
- React only formats values from the read model.
- Layout is compact, scannable, and avoids nested-card marketing composition.
- Controls have familiar icons and accessible labels.
- Components stay feature-local unless reuse is proven.
- Stable responsive dimensions prevent layout shifts.

## Non-Goals

- Calendar, charts, day detail, sessions, budgets, broad design system, or final
  exceptional-state polish

## Risk Class

medium

## Impact Areas

- Overview components
- Application shell
- Minimal visual primitives
- Populated-state tests

## Design Review

- Complexity introduced: one dense screen and a small primitive set.
- Decisions hidden: components own formatting and composition, not aggregation.
- Interface depth: the page consumes one view model and refresh action.
- Special cases: long values, unavailable cost, multiple sources, active refresh.
- Abstraction needed now: only primitives already required by the overview.
- Existing ownership: app shell and overview feature absorb the work.

## Checklist

- [ ] Establish restrained visual tokens and required primitives.
- [ ] Build overview page structure and shell integration.
- [ ] Render token and estimated-cost summaries.
- [ ] Render source breakdown and recent refresh state.
- [ ] Add accessible manual refresh with active behavior.
- [ ] Add populated tests through visible roles and text.
- [ ] Verify text fit at compact and desktop dimensions.
- [ ] Run frontend, architecture, and full verification.
- [ ] Complete this plan and activate Phase 5E.

## Test Plan

- Behavior: formatted values, source rows, unavailable cost, refresh action,
  active control, accessible names.
- Lowest stable layer: React Testing Library with real providers and controlled
  feature data.
- Failure paths: completed in Phase 5E.
- Fixtures: compact typed overview fixtures with meaningful value lengths.
- Runtime evidence: final evidence belongs to Phase 5E.
- Commands: focused vitest, typecheck, lint, and pnpm verify.

## Decisions

- Refine after Phase 5C establishes the component-facing interface.

## Verification

- Command: pnpm verify
- Outcome: queued; not run.

## Runtime Evidence

- Deferred to Phase 5E.

## Follow-Up Debt

- None.
