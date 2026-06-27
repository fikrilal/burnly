# 2026-06-26 Design System Phase 1c-2: Overlay Primitives

## Objective

Add the token-based overlay primitives over already-installed `radix-ui`:
`Tooltip`, `Popover`, `Dialog`, and `DropdownMenu`. These complete the Phase 1
primitive layer.

## Acceptance Criteria

- `Tooltip` (+ `TooltipProvider`), `Popover`, `Dialog`, and `DropdownMenu` exist
  in `src/components/ui/`, render through portals, and use `popover`/`card`
  surface tokens (no hardcoded `zinc`/`cyan`).
- Each opens from its trigger and is keyboard/escape dismissible via Radix.
- `Dialog` exposes a title for accessibility.
- Each is shown in the `#/styleguide` surface.

## Risk Class

`low`

Thin wrappers over already-installed `radix-ui` primitives; additive only.

## Impact Areas

- `src/components/ui/tooltip.tsx`, `popover.tsx`, `dialog.tsx`,
  `dropdown-menu.tsx` (new)
- `src/features/styleguide/StyleguideView.tsx` (add sections)

## Design Review

- What complexity is being introduced? Standard composable overlay wrappers;
  Radix owns focus management, portals, and dismissal.
- Which decisions are hidden inside the owning module? Surface tokens, spacing,
  and open/close animation classes.
- Is each new interface simpler than its implementation? Yes — callers compose
  named parts and get accessible overlays.
- What special cases exist, and can the design eliminate them? Portal/positioning
  handled uniformly by Radix; no per-call special cases.

## Checklist

- [x] Add `Tooltip` (+ `TooltipProvider`).
- [x] Add `Popover`.
- [x] Add `Dialog` (with header/title/description/footer/close).
- [x] Add `DropdownMenu` (trigger/content/item/separator/label).
- [x] Render each in the styleguide.
- [x] Add open/dismiss behavior tests.
- [x] Run verification.

## Test Plan

- Behavior and invariants to prove:
  - Dialog opens from its trigger and exposes a dialog role + title.
  - DropdownMenu opens and shows its items.
  - Popover opens and shows its content.
  - Tooltip shows content on hover/focus.
- Lowest stable test layer: RTL component tests (portals render to document.body).
- Relevant commands: `pnpm test`, `pnpm verify:fast`.

## Decisions

- Imported by file path (matching `ui` convention); no public-API budget change.
- Pass-through Radix parts (triggers/portals/close) re-exported as aliases;
  styled parts (content/overlay) are wrappers. Any resulting
  `react-refresh/only-export-components` warnings match the existing repo pattern
  and are warnings, not errors.

## Verification

- Command: `pnpm test src/components/ui src/features/styleguide`
- Outcome: passed (17 tests).
- Command: `pnpm test`
- Outcome: passed (110 tests, no regressions).
- Command: `pnpm verify:fast`
- Outcome: passed (exit 0, no lint errors).

## Runtime Evidence

- Styleguide screenshots are Phase 5b; not captured here.

## Follow-Up Debt

- Phase 1a/1d: beUI registry + Motion and selective motion primitives.
