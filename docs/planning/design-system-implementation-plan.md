# Burnly Design System Implementation Plan

## Status

In progress. Phase 0 complete. Phase 1 complete. Phase 2a (compact metric
components) complete and verified. Next: Phase 2b (allocation & trend).

Progress log:

- **2026-06-26 — Phase 0a + 0b complete.** shadcn tokens made authoritative,
  `color-scheme` driven by theme, `ThemeProvider` (light/dark/system) with
  `matchMedia` system-follow and `localStorage` persistence, pre-paint script
  added. Verified via `pnpm verify:fast` (exit 0) and `pnpm test` (93 passing).
  Execution plan:
  `docs/exec-plans/completed/2026-06-26_design-system-phase-0a-theme-foundation.md`.
- **2026-06-26 — Phase 0c complete.** Accessible monochrome `ThemeToggle`
  (Light/Dark/System) in `src/components/ui/theme-toggle.tsx`, token-based, with
  a shared `installMatchMedia` test helper and a `@` alias added to
  `vitest.config.ts`. Verified via `pnpm verify:fast` (exit 0) and `pnpm test`
  (96 passing). Execution plan:
  `docs/exec-plans/completed/2026-06-26_design-system-phase-0c-theme-toggle.md`.
- **2026-06-26 — Phase 1b + 5a complete.** Token-based `Badge`, `Skeleton`,
  `Separator` in `src/components/ui/`; a `#/styleguide` reference surface
  (`src/features/styleguide/StyleguideView.tsx`) rendering tokens, typography,
  and all current primitives with a live theme toggle; `App.tsx` surface routing
  extended for `#/styleguide`. Verified via `pnpm verify:fast` (exit 0) and
  `pnpm test` (103 passing). Execution plan:
  `docs/exec-plans/completed/2026-06-26_design-system-phase-1b-5a-primitives-styleguide.md`.
- **2026-06-26 — Phase 1c-1 complete.** Token-based inline primitives `Card`
  (composable), `Tabs` (pill style), and `Switch` over already-installed
  `radix-ui`, rendered in the styleguide. Verified via `pnpm verify:fast`
  (exit 0) and `pnpm test` (106 passing). Execution plan:
  `docs/exec-plans/completed/2026-06-26_design-system-phase-1c-1-inline-primitives.md`.
- **2026-06-26 — Phase 1c-2 complete.** Token-based overlay primitives `Tooltip`,
  `Popover`, `Dialog`, and `DropdownMenu` over `radix-ui`, rendered in the
  styleguide with open/dismiss tests. Verified via `pnpm verify:fast` (exit 0)
  and `pnpm test` (110 passing). Execution plan:
  `docs/exec-plans/completed/2026-06-26_design-system-phase-1c-2-overlay-primitives.md`.
- **2026-06-26 — Phase 1d complete.** Installed `motion@12.42.0` (pinned),
  registered the beUI shadcn registry (`@beui`) in `components.json`, and adopted
  beUI's `AnimatedNumber` adapted into `src/components/ui/animated-number.tsx`
  (reduced-motion-safe, animate-on-change). Demoed in the styleguide. Verified
  via `pnpm verify:fast` (exit 0) and `pnpm test` (112 passing). Execution plan:
  `docs/exec-plans/completed/2026-06-26_design-system-phase-1d-beui-motion.md`.
- **2026-06-26 — Phase 2a complete.** Burnly compact metric components
  `CompactMetric` (primary; value is a `ReactNode`, e.g. `AnimatedNumber`) and
  `MetricRow` (secondary pair) in `src/components/burnly/metric.tsx`, exported via
  the burnly barrel and demoed in the styleguide. Verified via `pnpm verify:fast`
  (exit 0) and `pnpm test` (115 passing). Execution plan:
  `docs/exec-plans/completed/2026-06-26_design-system-phase-2a-metrics.md`.

This is the master plan for building Burnly's **UI design system** (visual/component
system), not software architecture. It operationalizes the draft at
`docs/planning/_WIP/design-system-engineering-proposal.md` into phased,
executable work.

## Goal

Build one coherent, monochrome, calm utility design system that covers the whole
app — tray panel first, full desktop window second — so feature code stops
hand-writing one-off Tailwind strings and instead composes named, token-backed
components.

## Aesthetic Target

Reference feel: clean, minimal, monochrome, generous whitespace, restrained
motion, precise typography (the Aside-style look the user shared).

Burnly should feel: calm, fast, private, precise, lightweight. It should not feel
like an admin dashboard or a gamified app.

## Confirmed Direction

These were decided with the product owner and are not open for re-litigation
inside execution plans:

- **Palette:** default monochrome (black/white + neutral grays). No brand accent
  hue. Use shadcn's `neutral` base color.
- **Theme:** support light and dark. Provide a theme switch and a `system` option
  that auto-follows the OS theme.
- **Tokens:** adopt shadcn semantic token names directly (`--background`,
  `--card`, `--primary`, `--muted-foreground`, etc.). Do **not** add a second
  Burnly-specific token layer.
- **Component sources:** shadcn (via CLI) for most primitives; beUI
  (`https://beui.dev`, distributed through the shadcn registry) for motion-aware
  components. Tweak and customize both as needed once vendored.
- **CLI vs hand-write:** use the shadcn CLI for behavior/accessibility-heavy
  primitives (Dialog, Tooltip, Popover, DropdownMenu, Tabs, Switch); hand-write
  trivial ones (Badge, Skeleton, Separator) to avoid dead variants.
- **Verification of looks:** a `#/styleguide` route renders every token,
  primitive, component, and state, captured as Playwright screenshot evidence.
  No Storybook.
- **Scope:** global design system, applied across tray and full window.

## Current Baseline

- shadcn fully configured (`components.json`: `radix-nova` style, `neutral` base,
  CSS variables on, aliases set). Registry list is empty.
- `global.css` already contains the full shadcn neutral token set (light + dark),
  but it is largely unused — components hardcode `zinc-*` / `cyan-*` and bypass
  the tokens.
- No theme system: `:root` hardcodes `color-scheme: dark`; nothing toggles the
  `.dark` class. **(Resolved in Phase 0a/0b — theme system now exists in
  `src/lib/theme/`.)**
- Existing components: `ui/Button` (full cva variants), `burnly/CompactCard`,
  `burnly/StatusPill`. Tray feature has ad-hoc `PrimaryMetric`,
  `SecondaryMetricRow`, `ModelUsageAllocation`.
- React 19.1, Tailwind 4.3, Radix, cva, tailwind-merge, Geist, tw-animate-css
  present. Motion **installed** (`motion@12.42.0`, Phase 1d) and the beUI registry
  configured.

## Guiding Principles

- **Tray-driven extraction.** Build components the app actually uses now; extract
  from real surfaces rather than speculatively building a 20-component inventory.
  Honors `design-principles.md` (YAGNI, no abstractions for hypothetical reuse).
- **Tokens are the only color source.** After Phase 0, no feature or component
  may hardcode `zinc`/`cyan`/raw hex. Color comes from semantic tokens.
- **Monochrome first.** Semantic color (e.g. destructive red) is used only where
  meaning genuinely requires it. State is conveyed by text + icon + weight, not
  decorative color.
- **Primitives are domain-free.** `components/ui` knows nothing about Burnly
  concepts or IPC. `components/burnly` may know domain concepts but never calls
  IPC; it receives data via props.
- **Motion is restrained and reduced-motion-safe.** Every animation respects
  `prefers-reduced-motion`.

---

## Phases

Each phase is sized to become one or more execution plans under
`docs/exec-plans/`. Subphases are the likely execution-plan boundaries.

### Phase 0: Token & Theming Foundation

Goal: make shadcn tokens the single source of truth and make theme switching
real. Nothing visual ships until this is correct.

- **0a — Token reset. ✅ Done (2026-06-26).** shadcn neutral tokens are now
  authoritative; hardcoded `color-scheme: dark` removed and `color-scheme` is
  driven by theme (`:root` light, `.dark` dark). Typography/spacing/radius scales
  remain on the existing token set; deeper scale tuning folds into later phases.
- **0b — Theme provider & persistence. ✅ Done (2026-06-26).** `ThemeProvider`
  in `src/lib/theme/` supports `light`/`dark`/`system`, follows the OS via
  `matchMedia`, persists to `localStorage`, applies `.dark` at the document root,
  and a pre-paint script prevents flash. Default choice is `dark` until component
  token migration completes (then switch default to `system`).
- **0c — Theme toggle control. ✅ Done (2026-06-26).** Accessible monochrome
  `ThemeToggle` (Light/Dark/System) at `src/components/ui/theme-toggle.tsx`,
  built on `useTheme` and semantic tokens. Hand-written for now; beUI's animated
  version can replace the internals in Phase 1d. Wiring into Settings is Phase 4d;
  showing it in the styleguide is Phase 5a.

Acceptance: app renders identically in dark mode after the reset (no regressions),
light mode renders correctly, switching themes is instant and persists across
restart, `system` follows OS changes live. (Phase 0 complete.)

### Phase 1: Primitive Layer (`components/ui`)

Goal: a complete, monochrome, token-backed primitive set.

- **1a — Tooling setup. ✅ Done (2026-06-26).** beUI registry (`@beui`) added to
  `components.json`; `motion@12.42.0` installed (pinned). Convention: adapt beUI
  source into Burnly-owned components under `src/components/ui/`.
- **1b — Static primitives (hand-written). ✅ Done (2026-06-26).** `Badge`,
  `Skeleton`, `Separator` added on the monochrome tokens; `Button` already on
  tokens. Rendered in the styleguide.
- **1c — Interactive primitives (Radix-backed).** All required `radix-ui`
  packages are already installed, so these are hand-written thin wrappers on
  tokens without the shadcn CLI/network.
  - **1c-1 — Inline primitives. ✅ Done (2026-06-26).** `Card` (composable),
    `Tabs` (pill style), `Switch`. Rendered in the styleguide.
  - **1c-2 — Overlay primitives. ✅ Done (2026-06-26).** `Tooltip`, `Popover`,
    `Dialog`, `DropdownMenu` (portal-based), rendered in the styleguide.
- **1d — Motion primitives (beUI, selective). ✅ Done (2026-06-26).** Adopted
  beUI's `AnimatedNumber` (adapted, reduced-motion-safe) for animated metric
  values. `Tabs`/`ThemeToggle` kept hand-written; decorative beUI components
  (dock, dynamic island, tilt, marquee, magnetic, morphing modal) deferred.

Acceptance: every primitive exists, is monochrome, respects reduced motion, is
keyboard-accessible, and is registered in the styleguide (Phase 5a scaffold ✅).
(Phase 1 complete.)

### Phase 2: Burnly Compact Components (`components/burnly`)

Goal: promote the tray's inline patterns into named, prop-driven domain
components built on Phase 1 primitives.

- **2a — Metrics. ✅ Done (2026-06-26).** `CompactMetric` (primary; `ReactNode`
  value so callers can pass `AnimatedNumber`) and `MetricRow` (secondary pair),
  in `src/components/burnly/metric.tsx`, demoed in the styleguide.
- **2b — Allocation & trend:** `ModelUsageRow` / `AllocationList` and
  `TrendIndicator`, extracted from `ModelUsageAllocation.tsx`.
- **2c — Status & states:** `FreshnessStatus` (built on `StatusPill`),
  `EmptyState`, `ErrorState`, `OpenDetailsButton`.

Acceptance: components are pure (props in, no IPC), cover empty/partial/error/
loading states, and are exercised by React tests at the behavior level.

### Phase 3: Tray Panel Refactor (proving surface)

Goal: rebuild the existing tray panel entirely on the new system — the first
end-to-end proof.

- Replace all inline utilities in `TrayPanel.tsx` and its components with
  Phase 1/2 components and tokens.
- Tighten layout/typography toward the Aside aesthetic.
- Keep tray v1 scope: no cost, no source split, no budgets/filters.

Acceptance: tray renders with zero hardcoded colors, visual parity-or-better,
existing tray tests pass, runtime evidence (`pnpm evidence:desktop`) captured for
empty/populated/error/refreshing states in both themes.

### Phase 4: Full Desktop Window Reskin

Goal: apply the system to the secondary surface and correct its navigation.

- **4a — Shell & navigation.** Replace the ad-hoc tab bar in `App.tsx` with the
  `Tabs` primitive and the corrected nav set: Summary, Sessions, History,
  Settings, Diagnostics. Budgets de-emphasized (hidden/advanced). Calendar folded
  into History.
- **4b — Summary (Overview) reskin.** The `Open details` landing surface.
- **4c — Sessions & History reskin.**
- **4d — Settings reskin** (includes the theme toggle from 0c).
- **4e — Diagnostics reskin** (export/maintenance kept, visually secondary).

Acceptance: each screen uses only system components/tokens; no functional
regressions; tests pass; light + dark both verified.

### Phase 5: Styleguide, Evidence & Documentation

Goal: a living reference and a repeatable visual gate. Scaffolded early
(alongside Phase 1) and finalized here.

- **5a — Styleguide route.** A `#/styleguide` surface that renders every token,
  primitive, component, and state. Grows as components land.
- **5b — Visual evidence.** Playwright capture of the styleguide and key surfaces
  in both themes, wired into the evidence pipeline.
- **5c — Design-system doc.** A maintained doc describing tokens, components,
  usage rules, and do/don't; promote the `_WIP` proposal out of `_WIP` or
  replace it; update `docs/README.md`.

Acceptance: styleguide covers 100% of system components; screenshots committed as
evidence; docs index points to the living design-system doc.

---

## Cross-Cutting Concerns

- **Architecture harness.** Adding components may move the public-API budget
  (`scripts/harness/public-api-budget.json`) and trigger the duplication report
  (`jscpd`). Update budgets deliberately, with justification, per AGENTS.md.
- **Boundary rule.** Components must not import from `src/ipc/` transport details
  or call Tauri. Feature hooks own data; components render props.
- **Accessibility.** Keyboard reachability, visible focus, non-color-only status,
  contrast on both themes, reduced-motion — verified per phase.
- **Bundle weight.** Motion is a new runtime dependency; keep beUI adoption
  surgical to protect the lightweight-utility goal.
- **No big-bang rewrite.** Each phase leaves the app shippable; the tray and full
  window are migrated incrementally onto the system.

## Verification

- Per change: `pnpm verify:fast` while iterating; `pnpm verify` before completing
  a phase.
- Frontend behavior: `pnpm test` (Vitest + RTL).
- Visual: `pnpm evidence:desktop` / styleguide screenshots — looks are judged by
  human review of evidence, never by automated tests alone.
- Harness: `pnpm architecture:check`, `pnpm public-api:check`.

## Decisions To Confirm

1. **Theme persistence location. ✅ Resolved — webview `localStorage`** (UI-only,
   implemented in Phase 0b). Revisit only if the backend needs the preference.
2. **Motion dependency. ✅ Resolved — yes, surgical.** `motion@12.42.0` installed
   (Phase 1d); only `AnimatedNumber` adopted so far. Decorative beUI components
   stay deferred.
3. **Status color in a monochrome system. ⏳ Tentatively locked: monochrome +
   icon/text, red reserved strictly for errors.** Confirm before Phase 2c.

## Out Of Scope

- Source-split and cost UI (no caller in tray v1).
- Decorative beUI components (tilt, dock, dynamic island, marquee, magnetic,
  morphing modal).
- Budgets/calendar redesign beyond de-emphasis.
- Charts/data-viz system (ECharts theming) — separate later effort if needed.

## Sequencing Summary

```text
Phase 0  Tokens + theming        (foundation, blocks everything)  [✅ complete]
Phase 1  Primitives (ui/)        + Phase 5a styleguide scaffold   [✅ complete]
Phase 2  Burnly components       (extract from tray)              [2a ✅ · 2b/2c ⏳]
Phase 3  Tray refactor           (prove the system)
Phase 4  Full window reskin      (apply broadly)
Phase 5  Styleguide/evidence/docs (finalize)
```
