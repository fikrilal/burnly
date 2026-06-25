# 2026-06-26 Design System Phase 0a: Theme Foundation

## Objective

Make shadcn semantic tokens the authoritative color source and add real theme
infrastructure (light / dark / system) without changing the current visual
appearance. This is the foundation for the design system implementation plan
(`docs/planning/design-system-implementation-plan.md`).

## Acceptance Criteria

- A `ThemeProvider` supports `light`, `dark`, and `system` choices.
- `system` follows the OS preference live via `matchMedia`.
- The selected theme is persisted to `localStorage` and restored on launch.
- The `.dark` class is applied to `document.documentElement`; `color-scheme` is
  driven by the theme (light in `:root`, dark in `.dark`).
- A synchronous pre-paint script applies the stored/default theme before first
  paint (no flash of wrong theme).
- Default choice is `dark`, so there is no visual regression while feature
  components still hardcode dark colors (migration happens in later phases).
- `useTheme()` exposes `{ choice, resolvedTheme, setChoice }` and throws outside
  the provider.
- No component restyle in this chunk; existing screens render unchanged.

## Risk Class

`low`

Additive infrastructure; default behavior preserves the current dark appearance.

## Impact Areas

- `src/lib/theme/` (new)
- `src/main.tsx` (wrap App with ThemeProvider)
- `index.html` (pre-paint theme script)
- `src/styles/global.css` (color-scheme handling)

## Design Review

- What complexity is being introduced? A small theme controller with pure
  resolution logic separated from React/DOM side effects.
- Which decisions are hidden inside the owning module? Storage key, default
  choice, system detection, and class/color-scheme application.
- Is each new interface simpler than its implementation? Yes — `useTheme()`
  exposes three fields and hides matchMedia, persistence, and DOM wiring.
- What special cases exist, and can the design eliminate them? localStorage
  access failures are swallowed; SSR guards are intentionally omitted because the
  app is a browser-only Tauri webview.

## Checklist

- [x] Add pure logic in `src/lib/theme/theme.ts` (types, constants, `resolveTheme`,
      guards, storage read, class apply).
- [x] Add `src/lib/theme/theme-context.ts` (context + `useTheme`).
- [x] Add `src/lib/theme/ThemeProvider.tsx` (provider component only).
- [x] Add `src/lib/theme/index.ts` re-exports.
- [x] Wrap `App` with `ThemeProvider` in `src/main.tsx`.
- [x] Add pre-paint script to `index.html` (key/default in sync with module).
- [x] Drive `color-scheme` from theme in `src/styles/global.css`.
- [x] Add unit tests for pure logic and provider behavior.
- [x] Run verification.

## Test Plan

- Behavior and invariants to prove:
  - `resolveTheme` maps choice + system preference to light/dark correctly.
  - Stored choice is restored; invalid/missing falls back to default.
  - `setChoice` persists and updates the resolved theme.
  - `system` reacts to matchMedia changes.
  - `useTheme` throws outside a provider.
- Lowest stable test layer: Vitest unit tests for pure logic; RTL for provider.
- Fixtures or fakes: a controllable `matchMedia` stub in the provider test.
- Relevant commands: `pnpm test`, `pnpm typecheck`, `pnpm lint`, `pnpm format:check`,
  `pnpm verify:fast`.

## Decisions

- Default theme choice is `dark` for now; switch the default to `system` once
  component token migration (Phase 3/4) is complete, to avoid native-control
  mismatch on light OSes while panels are still hardcoded dark.
- Theme preference persists in webview `localStorage` (UI-only), not the Rust
  settings store, per the master plan's confirmed decision.
- SSR guards omitted intentionally (browser-only webview).
- Registered `src/lib/theme/index.ts` in the public-API budget at 3 exports
  (`scripts/harness/public-api-budget.json`): one barrel exposing the provider,
  the `useTheme` hook + context type, and the theme constants/types.

## Verification

- Command: `pnpm test src/lib/theme`
- Outcome: passed (14 tests).
- Command: `pnpm test`
- Outcome: passed (93 tests, no regressions).
- Command: `pnpm verify:fast`
- Outcome: passed (exit 0) after registering the new barrel in the public-API
  budget.

## Runtime Evidence

- Not required for this chunk (no visual change). Visual evidence begins when
  components adopt tokens in later phases.

## Follow-Up Debt

- None.
