# 2026-06-26 Design System Phase 2a: Compact Metric Components

## Objective

Add Burnly-owned, token-based metric components — `CompactMetric` (primary) and
`MetricRow` (secondary pair) — generalizing the tray's inline `PrimaryMetric` /
`SecondaryMetricRow` patterns. These are prop-driven and do not call IPC.

## Acceptance Criteria

- `CompactMetric` renders a label, a prominent value (any `ReactNode`, so callers
  may pass a plain string or an `AnimatedNumber`), and an optional caption.
- `MetricRow` renders a responsive two-up grid of compact secondary metrics from
  an `items` list.
- Both use semantic tokens only (no hardcoded `zinc`/`cyan`) and live in
  `src/components/burnly/`, exported via the burnly barrel.
- Both are shown in the `#/styleguide` surface.
- No IPC or Tauri usage; pure presentational components.

## Risk Class

`low`

Additive presentational components; existing tray code is untouched in this chunk
(the tray refactor onto these is Phase 3).

## Impact Areas

- `src/components/burnly/metric.tsx` (new)
- `src/components/burnly/index.ts` (barrel export)
- `src/features/styleguide/StyleguideView.tsx` (metrics section)

## Design Review

- What complexity is being introduced? Two small presentational components.
- Which decisions are hidden inside the owning module? Type scale and the
  secondary metric's bordered-card treatment.
- Is each new interface simpler than its implementation? Yes — callers pass a
  label/value; no behavioral-mode flags (value accepts a `ReactNode`).
- What special cases exist, and can the design eliminate them? The secondary
  metric is owned entirely inside `MetricRow`, avoiding a primary/secondary mode
  flag on `CompactMetric`.

## Checklist

- [x] Add `CompactMetric` (primary metric, value is `ReactNode`).
- [x] Add `MetricRow` (secondary pair grid from `items`).
- [x] Export both from the burnly barrel.
- [x] Render both in the styleguide (primary uses `AnimatedNumber`).
- [x] Add component tests.
- [x] Run verification.

## Test Plan

- Behavior and invariants to prove:
  - `CompactMetric` renders label, value, and optional caption.
  - `MetricRow` renders each item's label and value.
- Lowest stable test layer: RTL component tests.
- Relevant commands: `pnpm test`, `pnpm verify:fast`.

## Decisions

- `CompactMetric` value is a `ReactNode` so animation is a caller choice
  (`AnimatedNumber`) rather than a boolean mode flag on the component.
- Secondary metric styling lives inside `MetricRow`; no primary/secondary flag.
- burnly barrel stays within its public-API budget (2 export lines).

## Verification

- Command: `pnpm test src/components/burnly src/features/styleguide`
- Outcome: passed (5 tests).
- Command: `pnpm test`
- Outcome: passed (115 tests, no regressions).
- Command: `pnpm verify:fast`
- Outcome: passed (exit 0). burnly barrel stayed within its 2-line public-API
  budget.

## Runtime Evidence

- Styleguide screenshots are Phase 5b; not captured here.

## Follow-Up Debt

- Phase 2b (allocation/trend) and 2c (status/empty/error) components.
- Phase 3 tray refactor will consume these.
