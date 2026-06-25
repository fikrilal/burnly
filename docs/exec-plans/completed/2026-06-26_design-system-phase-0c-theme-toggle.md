# 2026-06-26 Design System Phase 0c: Theme Toggle Control

## Objective

Provide a reusable, accessible, monochrome theme selection control (Light / Dark /
System) backed by the Phase 0b `ThemeProvider`. This completes the Phase 0
theming foundation's user-facing control.

## Acceptance Criteria

- A `ThemeToggle` component renders Light, Dark, and System options.
- The active choice is reflected with `aria-pressed`.
- Selecting an option calls `setChoice` and updates the resolved theme/`.dark`
  class immediately.
- The control is keyboard reachable with visible focus and uses only semantic
  tokens (no hardcoded `zinc`/`cyan`).
- Hand-written for now; beUI's animated Theme Toggle can replace the internals in
  Phase 1d without changing the component's public shape.
- Not yet wired into a visible surface (Settings wiring is Phase 4d; styleguide
  is Phase 5a). Verified via React tests.

## Risk Class

`low`

Additive, self-contained UI component with no IPC or platform coupling.

## Impact Areas

- `src/components/ui/theme-toggle.tsx` (new)
- `src/test/match-media.ts` (new shared test helper)
- `src/lib/theme/ThemeProvider.test.tsx` (use shared helper)

## Design Review

- What complexity is being introduced? A small presentational control over the
  existing `useTheme` hook; no new state ownership.
- Which decisions are hidden inside the owning module? Option ordering, icons,
  and monochrome styling.
- Is each new interface simpler than its implementation? Yes — `<ThemeToggle />`
  takes no required props and hides theme wiring.
- What special cases exist, and can the design eliminate them? None; the three
  choices map uniformly to `ThemeChoice`.

## Checklist

- [x] Extract `installMatchMedia` into `src/test/match-media.ts`.
- [x] Refactor `ThemeProvider.test.tsx` to use the shared helper.
- [x] Add `src/components/ui/theme-toggle.tsx` (token-based, accessible).
- [x] Add `src/components/ui/theme-toggle.test.tsx`.
- [x] Run verification.

## Test Plan

- Behavior and invariants to prove:
  - Active choice shows `aria-pressed="true"`; others `false`.
  - Selecting Light/Dark toggles the `.dark` class accordingly.
- Lowest stable test layer: RTL component tests within a real `ThemeProvider`.
- Fixtures or fakes: shared `installMatchMedia` test helper.
- Relevant commands: `pnpm test`, `pnpm verify:fast`.

## Decisions

- Imported by file path (`@/components/ui/theme-toggle`), matching the existing
  `ui` convention where `Button` is not re-exported through the barrel; no
  public-API budget change.
- Motion decision (#2) accepted as surgical; beUI internals deferred to Phase 1d.
- Added the `@` path alias to `vitest.config.ts` (mirroring `vite.config.ts`) so
  tests can resolve `@/` imports the way app/source code already does. This
  unblocks `@/`-style imports in all future component tests.

## Verification

- Command: `pnpm test src/components/ui src/lib/theme`
- Outcome: passed (17 tests).
- Command: `pnpm test`
- Outcome: passed (96 tests, no regressions).
- Command: `pnpm verify:fast`
- Outcome: passed (exit 0).

## Runtime Evidence

- Not required (component not yet wired into a visible surface).

## Follow-Up Debt

- Wire `ThemeToggle` into Settings (Phase 4d) and the styleguide (Phase 5a).
- Consider replacing internals with beUI Theme Toggle in Phase 1d.
