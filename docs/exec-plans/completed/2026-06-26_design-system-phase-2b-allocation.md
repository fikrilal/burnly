# 2026-06-26 Design System Phase 2b: Allocation & Trend Components

## Objective

Add Burnly-owned, token-based, monochrome `AllocationList` and `TrendIndicator`
components generalizing the tray's inline `ModelUsageAllocation` pattern, with
prop types local to the design system (no IPC contract types).

## Acceptance Criteria

- `TrendIndicator` shows direction via an icon (not color) plus a percentage, and
  renders "new today" when there is no trend. Monochrome (no green/red).
- `AllocationList` renders ranked model rows (monochrome rank accent, model name,
  agent label, token value, trend) and an inline empty state.
- Components define their own prop types (`Trend`, `ModelUsage`) and do not import
  generated IPC contract types.
- Token-based only; shown in the `#/styleguide` surface.

## Risk Class

`low`

Additive presentational components; tray code untouched (Phase 3 consumes these).

## Impact Areas

- `src/components/burnly/allocation.tsx` (new)
- `src/components/burnly/index.ts` (barrel export)
- `scripts/harness/public-api-budget.json` (burnly barrel 2 -> 3)
- `src/features/styleguide/StyleguideView.tsx` (allocation section)

## Design Review

- What complexity is being introduced? One list component and one small trend
  indicator; row rendering is internal to the list.
- Which decisions are hidden inside the owning module? Monochrome rank accents,
  trend formatting, and direction-by-icon.
- Is each new interface simpler than its implementation? Yes — callers pass a
  `models` list; the list owns ranking accents and rows.
- What special cases exist, and can the design eliminate them? "Other"/missing
  trend handled uniformly: a null trend renders "new today" with no branching at
  the call site.

## Checklist

- [x] Add `TrendIndicator` (icon + percentage, monochrome, "new today" fallback).
- [x] Add `AllocationList` (ranked rows + inline empty state) with local prop types.
- [x] Export both from the burnly barrel; bump barrel public-API budget to 3.
- [x] Render both in the styleguide.
- [x] Add component tests.
- [x] Run verification.

## Test Plan

- Behavior and invariants to prove:
  - `TrendIndicator` renders "new today" with no trend and a percentage otherwise.
  - `AllocationList` renders each model's name/agent/tokens and an empty state.
- Lowest stable test layer: RTL component tests.
- Relevant commands: `pnpm test`, `pnpm verify:fast`.

## Decisions

- Trend uses icon + monochrome text, honoring the monochrome direction (no
  green/red), consistent with the master plan's status-color decision.
- Local prop types (`Trend`, `ModelUsage`) keep the component independent of the
  IPC contract; the tray maps contract data to these props in Phase 3.
- burnly barrel public-API budget bumped 2 -> 3 deliberately for the new
  allocation export line.

## Verification

- Command: `pnpm test src/components/burnly src/features/styleguide`
- Outcome: passed (10 tests).
- Command: `pnpm test`
- Outcome: passed (120 tests, no regressions).
- Command: `pnpm verify:fast`
- Outcome: passed (exit 0). Initial typecheck flagged an
  `exactOptionalPropertyTypes` mismatch passing an optional `trend` into
  `TrendIndicator`; fixed by coercing `model.trend ?? null` at the call site.
  burnly barrel public-API budget bumped 2 -> 3.

## Runtime Evidence

- Styleguide screenshots are Phase 5b; not captured here.

## Follow-Up Debt

- Phase 2c (status/empty/error) components; Phase 3 tray refactor.
