# 2026-06-25 Tray Compact UI

## Objective

Implement the compact tray panel UI and the minimal design-system primitives it
needs.

This chunk should make the tray panel feel like the product's primary surface.
It should not redesign the full desktop app.

## Acceptance Criteria

- Tray panel renders:
  - freshness header,
  - large today token metric,
  - this week and this month metric row,
  - model usage allocation list,
  - coding-agent labels,
  - trend versus yesterday,
  - `Open details` action.
- Tray panel supports loading, empty, current, refreshing, partial, and failed
  states.
- Cost, source split, budgets, export, diagnostics details, and filters are not
  rendered in tray v1.
- The compact UI uses reusable components instead of repeating raw Tailwind
  styling across the feature.
- React feature code continues to use `src/ipc/` only.

## Risk Class

`medium`

This is product-critical UI. The main risk is building a small dashboard instead
of a compact tracker.

## Impact Areas

- React components
- Design-system primitives
- Tray panel feature code
- IPC hooks/query state
- Accessibility and reduced-motion behavior

## Design Review

- Complexity introduced: compact metric/allocation components.
- Owning module: generic primitives belong in `src/components/ui`; Burnly
  concept components belong in `src/components/burnly` or equivalent.
- Interface depth: tray feature should consume one compact summary hook and
  compose small display components.
- Special cases: long model names, missing trend baseline, no data, partial
  refresh, reduced motion.
- New abstractions needed now: compact metric, secondary metric row, allocation
  row, freshness/status indicator.

## Checklist

- [ ] Add minimal compact UI primitives.
- [ ] Add Burnly compact metric/allocation components.
- [ ] Add tray panel feature route/component.
- [ ] Add query hook for compact tray summary.
- [ ] Implement empty/loading/current/partial/failed states.
- [ ] Implement `Open details` action.
- [ ] Add React tests for key states.
- [ ] Confirm no direct Tauri API usage in feature code.

## Test Plan

- Behavior and invariants to prove:
  - primary metric dominates visual hierarchy,
  - week/month are secondary,
  - model list uses coding-agent labels, not percentage text,
  - missing trend baseline renders safely,
  - no refresh button appears as primary action.
- Lowest stable test layer:
  - React component tests,
  - hook tests with fake IPC client responses,
  - existing architecture checks.
- Failure paths:
  - empty data,
  - failed refresh state,
  - partial refresh state,
  - long model and source labels.
- Fixtures or fakes:
  - frontend fixture responses for compact tray summary states.
- Runtime or platform evidence:
  - final runtime chunk.
- Relevant commands:
  - `pnpm typecheck`
  - `pnpm test`
  - `pnpm architecture:check`

## Decisions

- Build only the primitives needed for tray v1.
- Do not introduce Storybook.
- Do not introduce broad beUI components before a concrete need.

## Verification

- Command: `pnpm verify`
- Outcome: not run yet.

## Runtime Evidence

- Not collected yet.

## Follow-Up Debt

- Full desktop app shell redesign remains separate and later.
