# 2026-06-26 Design System Phase 1b + 5a: Static Primitives & Styleguide Scaffold

## Objective

Add the first token-based static primitives (`Badge`, `Skeleton`, `Separator`)
and a `#/styleguide` surface that renders tokens, primitives, and the
`ThemeToggle` in both themes. This establishes the visual verification surface
the rest of the design-system work depends on.

## Acceptance Criteria

- `Badge`, `Skeleton`, `Separator` exist in `src/components/ui/`, are
  token-based (no hardcoded `zinc`/`cyan`), and respect reduced motion where
  animated.
- A `#/styleguide` app surface renders: theme toggle, color/surface token
  swatches, typography samples, and all current primitives (`Button`, `Badge`,
  `Skeleton`, `Separator`, `StatusPill`, `CompactCard`).
- The styleguide is itself theme-driven (uses semantic tokens), so flipping the
  theme visibly restyles it.
- No regressions to existing surfaces (desktop/tray routing unchanged otherwise).

## Risk Class

`low`

Additive primitives plus one new dev/reference surface; the only edit to
existing code is extending the surface switch in `App.tsx`.

## Impact Areas

- `src/components/ui/badge.tsx`, `skeleton.tsx`, `separator.tsx` (new)
- `src/features/styleguide/StyleguideView.tsx` (new)
- `src/app/App.tsx` (surface routing adds `#/styleguide`)

## Design Review

- What complexity is being introduced? Three small presentational primitives and
  a reference surface; no new state or data flow.
- Which decisions are hidden inside the owning module? Variant classes and token
  mappings live inside each primitive.
- Is each new interface simpler than its implementation? Yes — primitives take
  standard element props plus a `variant`.
- What special cases exist, and can the design eliminate them? Surface routing
  adds one branch (`styleguide`) alongside the existing `tray` branch; uniform.

## Checklist

- [x] Add `Badge` (cva variants, token-based).
- [x] Add `Skeleton` (token-based, reduced-motion-safe).
- [x] Add `Separator` (Radix `Separator`, already-installed `radix-ui`).
- [x] Add `StyleguideView` with token/typography/primitive sections + theme toggle.
- [x] Extend `appSurface()` in `App.tsx` to route `#/styleguide`.
- [x] Add tests for primitives and the styleguide surface.
- [x] Run verification.

## Test Plan

- Behavior and invariants to prove:
  - `Badge` renders children and applies the requested variant class.
  - `Separator` exposes a separator role/orientation.
  - `StyleguideView` renders its key sections and an interactive theme toggle.
- Lowest stable test layer: RTL component/surface tests.
- Relevant commands: `pnpm test`, `pnpm verify:fast`.

## Decisions

- Primitives imported by file path (matching the `ui` convention); no public-API
  budget change and no `styleguide/index.ts` barrel.
- beUI/Motion adoption (Phase 1a/1d) deferred to when a motion component is
  actually wired; interactive Radix primitives (Phase 1c) use already-installed
  `radix-ui` packages and come in the next chunk.

## Verification

- Command: `pnpm test src/components/ui src/features/styleguide`
- Outcome: passed (10 tests).
- Command: `pnpm test`
- Outcome: passed (103 tests, no regressions).
- Command: `pnpm verify:fast`
- Outcome: passed (exit 0). Pre-existing `App.tsx` size/complexity warnings and
  the `badge.tsx` component+variants `react-refresh` warning (matching the
  existing `button.tsx` pattern) are warnings only, not errors.

## Runtime Evidence

- Styleguide screenshots become part of Phase 5b evidence; not captured in this
  chunk.

## Follow-Up Debt

- Phase 1c interactive primitives (Tooltip, Dialog, Popover, DropdownMenu, Switch,
  Tabs) and Phase 1a/1d beUI + Motion.
