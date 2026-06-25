# 2026-06-26 Design System Phase 1c-1: Inline Primitives (Card, Tabs, Switch)

## Objective

Add the token-based inline/layout interactive primitives needed for screens, the
full-window navigation (Phase 4a), and Settings: `Card` (composable), `Tabs`
(pill style), and `Switch`. Overlay primitives (Tooltip, Popover, Dialog,
DropdownMenu) follow in Phase 1c-2.

## Acceptance Criteria

- `Card` (+ `CardHeader`, `CardTitle`, `CardDescription`, `CardContent`,
  `CardFooter`) renders on `bg-card`/token styling.
- `Tabs` wraps Radix `Tabs` with a monochrome pill list; the active trigger is
  visually distinct and switches content.
- `Switch` wraps Radix `Switch`, token-based, accessible, toggles state.
- All three are monochrome (no hardcoded `zinc`/`cyan`) and keyboard-accessible
  via Radix.
- Each is rendered in the `#/styleguide` surface.

## Risk Class

`low`

Thin wrappers over already-installed `radix-ui` primitives; additive only.

## Impact Areas

- `src/components/ui/card.tsx`, `tabs.tsx`, `switch.tsx` (new)
- `src/features/styleguide/StyleguideView.tsx` (add sections)

## Design Review

- What complexity is being introduced? Standard composable wrappers; Radix owns
  behavior/accessibility, the wrappers own monochrome token styling.
- Which decisions are hidden inside the owning module? Token classes and the pill
  tab treatment.
- Is each new interface simpler than its implementation? Yes — callers use
  semantic component names and get accessibility for free.
- What special cases exist, and can the design eliminate them? None;
  orientation/state handled uniformly by Radix data attributes.

## Checklist

- [x] Add `Card` composable set (token-based).
- [x] Add `Tabs` (Radix, pill list, token-based).
- [x] Add `Switch` (Radix, token-based).
- [x] Render all three in the styleguide.
- [x] Add behavior tests (tab switching, switch toggle, card composition).
- [x] Run verification.

## Test Plan

- Behavior and invariants to prove:
  - Selecting a tab shows its panel and marks the trigger active.
  - Toggling the switch flips its checked state.
  - Card composition renders its regions.
- Lowest stable test layer: RTL component tests.
- Relevant commands: `pnpm test`, `pnpm verify:fast`.

## Decisions

- Imported by file path (matching `ui` convention); no public-API budget change.
- Pill-style tabs now; beUI animated Tabs may enhance the active indicator in
  Phase 1d without changing the wrapper API.

## Verification

- Command: `pnpm test src/components/ui src/features/styleguide`
- Outcome: passed (13 tests).
- Command: `pnpm test`
- Outcome: passed (106 tests, no regressions).
- Command: `pnpm verify:fast`
- Outcome: passed (exit 0).

## Runtime Evidence

- Styleguide screenshots are Phase 5b; not captured here.

## Follow-Up Debt

- Phase 1c-2 overlay primitives (Tooltip, Popover, Dialog, DropdownMenu).
