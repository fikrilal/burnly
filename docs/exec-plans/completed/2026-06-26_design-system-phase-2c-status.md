# 2026-06-26 Design System Phase 2c: Status & State Components

## Objective

Add Burnly-owned, token-based, monochrome status/state components generalizing
the tray's inline freshness header, empty state, error banner, and open-details
action: `FreshnessStatus`, `EmptyState`, `ErrorState`, `OpenDetailsButton`.

## Acceptance Criteria

- `FreshnessStatus` renders an icon + label for each state
  (`current`/`stale`/`partial`/`refreshing`/`failed`/`empty`); monochrome with
  `destructive` reserved for `failed`; `refreshing` spins (reduced-motion-safe).
- `EmptyState` renders an icon, title, and optional description (token-based).
- `ErrorState` renders a destructive-toned inline banner with title, optional
  description, and optional action.
- `OpenDetailsButton` wraps the `Button` primitive with an external-link icon.
- All token-based and shown in the `#/styleguide` surface.

## Risk Class

`low`

Additive presentational components; tray untouched (Phase 3 consumes these).

## Impact Areas

- `src/components/burnly/status.tsx` (new)
- `src/components/burnly/index.ts` (barrel export)
- `scripts/harness/public-api-budget.json` (burnly barrel 3 -> 4)
- `src/features/styleguide/StyleguideView.tsx` (states section)

## Design Review

- What complexity is being introduced? Four small presentational components with
  a state->icon/label map for freshness.
- Which decisions are hidden inside the owning module? State labels/icons and the
  monochrome-with-destructive-for-failure mapping.
- Is each new interface simpler than its implementation? Yes.
- What special cases exist, and can the design eliminate them? Freshness states
  map uniformly through a single record; only `failed` selects the destructive
  treatment.

## Checklist

- [x] Add `FreshnessStatus` (state -> icon + label, monochrome + destructive-on-fail).
- [x] Add `EmptyState`.
- [x] Add `ErrorState` (destructive inline banner).
- [x] Add `OpenDetailsButton` (wraps `Button`).
- [x] Export from the burnly barrel; bump barrel public-API budget to 4.
- [x] Render all in the styleguide.
- [x] Add component tests.
- [x] Run verification.

## Test Plan

- Behavior and invariants to prove:
  - `FreshnessStatus` renders the right label per state.
  - `EmptyState`/`ErrorState` render title (+ description).
  - `OpenDetailsButton` renders its label and fires `onClick`.
- Lowest stable test layer: RTL component tests.
- Relevant commands: `pnpm test`, `pnpm verify:fast`.

## Decisions

- `FreshnessStatus` is built standalone (token-based, monochrome) rather than
  wrapping the legacy `StatusPill`, which still has pre-design-system hardcoded
  colors. StatusPill's token migration is deferred to the Phase 3 tray refactor
  (tracked as debt below).
- Monochrome states convey meaning via icon + text; `destructive` (red) is used
  only for `failed`/error, per the master plan's status-color decision.
- burnly barrel public-API budget bumped 3 -> 4.

## Verification

- Command: `pnpm test src/components/burnly src/features/styleguide`
- Outcome: passed (14 tests).
- Command: `pnpm test`
- Outcome: passed (124 tests, no regressions).
- Command: `pnpm verify:fast`
- Outcome: passed (exit 0). burnly barrel public-API budget bumped 3 -> 4.

## Runtime Evidence

- Styleguide screenshots are Phase 5b; not captured here.

## Follow-Up Debt

- Migrate `StatusPill` (compact-card.tsx) from hardcoded `zinc`/`emerald`/`amber`/
  `red` to semantic tokens during the Phase 3 tray refactor.
- Phase 3 tray refactor consumes all Phase 2 components.
