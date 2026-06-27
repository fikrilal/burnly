# 2026-06-26 Design System Phase 3: Tray Panel Refactor

## Objective

Rebuild the tray panel on the new design system as the first end-to-end proof:
replace inline `zinc`/`cyan` utilities and tray-local components with tokens and
Burnly compact components, and resolve the legacy `StatusPill`/`CompactCard`
color debt.

## Acceptance Criteria

- `TrayPanel` uses `CompactMetric` (+ `AnimatedNumber`), `MetricRow`,
  `AllocationList`, `FreshnessStatus`, `EmptyState`, `ErrorState`, and
  `OpenDetailsButton`; no hardcoded `zinc`/`cyan` remain in the tray.
- The panel uses `bg-background`/`text-foreground`/token surfaces and renders
  correctly in light and dark.
- Tray v1 scope preserved: no cost, no source split, no budgets/filters, no
  primary refresh button.
- `CompactCard` and `StatusPill` migrated from hardcoded colors to tokens.
- Tray-local `TrayMetric.tsx` and `ModelUsageAllocation.tsx` are removed (folded
  into the design-system components).
- Existing tray tests pass (updated for the new monochrome trend and copy).

## Risk Class

`medium`

Refactors a shipped product surface; behavior and tests must be preserved.

## Impact Areas

- `src/features/tray/TrayPanel.tsx` (rewrite)
- `src/features/tray/components/TrayMetric.tsx`, `ModelUsageAllocation.tsx` (remove)
- `src/components/burnly/compact-card.tsx` (token migration)
- `src/features/tray/TrayPanel.test.tsx` (update assertions)
- `src/test/setup.ts` (default `matchMedia` for components using `useReducedMotion`)

## Design Review

- What complexity is being introduced? None net — the tray composes existing
  design-system components instead of bespoke inline markup; net code reduction.
- Which decisions are hidden inside the owning module? The mapping from the IPC
  summary (dataStatus/isRefreshing/isError) to a `FreshnessState`, and from
  contract models to `ModelUsage`.
- Is each new interface simpler than its implementation? Yes — the tray reads as
  a composition of named components.
- What special cases exist, and can the design eliminate them? Freshness mapping
  is a single function; empty/error states are dedicated components.

## Checklist

- [x] Add a default `matchMedia` to `src/test/setup.ts`.
- [x] Migrate `CompactCard` and `StatusPill` to semantic tokens.
- [x] Rewrite `TrayPanel` on the design-system components.
- [x] Remove `TrayMetric.tsx` and `ModelUsageAllocation.tsx`.
- [x] Update tray tests for the monochrome trend and new copy.
- [x] Run verification (and desktop runtime evidence if available).

## Test Plan

- Behavior and invariants to prove:
  - Today/week/month token totals render formatted.
  - Model allocation renders names, agents, tokens, and trend ("8.5%", "new today").
  - Empty state shows the no-usage copy; failed state shows the error.
  - `Open details` calls the IPC boundary; no refresh button is present.
  - No cost text leaks into the tray.
- Lowest stable test layer: RTL tray tests with mocked IPC.
- Runtime evidence: `pnpm evidence:desktop` if the desktop runtime is available.
- Relevant commands: `pnpm test`, `pnpm verify:fast`.

## Decisions

- Resolve the `StatusPill`/`CompactCard` token debt here (planned during the tray
  refactor). `StatusPill` success/warning share a monochrome emphasis treatment;
  `danger` uses `destructive` — consistent with the status-color decision.
- `FreshnessStatus` replaces the tray's inline status pill; the contract
  `dataStatus`/refresh/error state maps to a single `FreshnessState`.
- Today's metric uses `AnimatedNumber` (value parsed from the token string).

## Verification

- Command: `pnpm test src/features/tray`
- Outcome: passed (4 tray tests).
- Command: `pnpm test`
- Outcome: passed (124 tests, no regressions).
- Command: `pnpm verify:fast`
- Outcome: passed (exit 0).
- Command: `pnpm test:e2e`
- Outcome: passed (30 Playwright tests across Desktop + Compact projects),
  confirming no desktop regression from the `CompactCard`/`StatusPill` token
  migration. Desktop evidence screenshots regenerated.

## Runtime Evidence

- Playwright e2e ran in this environment and passed (30/30), regenerating the
  desktop/compact evidence screenshots. The full `pnpm evidence:desktop` pipeline
  (which also runs `tauri info` and a Tauri-oriented build) was not run end to
  end; the e2e visual portion — the part affected by this frontend refactor —
  passed. The tray surface itself is covered by the 4 RTL tray tests; the shared
  design-system components it now uses are exercised by the desktop e2e.

## Follow-Up Debt

- Phase 4 full desktop window reskin; Phase 5 styleguide evidence + docs.
