# 2026-06-26 Design System Phase 1d: beUI + Motion (AnimatedNumber)

## Objective

Set up the beUI registry and Motion dependency, and adopt one beUI motion
primitive the app actually needs: an `AnimatedNumber` for animated metric values
(today/week/month token totals that animate when a refresh changes them).

## Acceptance Criteria

- `motion` is installed at a pinned exact version.
- beUI's registry (`@beui` → `https://beui.dev/r/{name}`) is configured in
  `components.json` for future CLI use.
- `AnimatedNumber` exists in `src/components/ui/`, adapted to Burnly conventions
  (`@/lib/cn`, no RSC directive, no viewport gating), animates on value change,
  and is reduced-motion-safe (jumps instantly when reduced motion is preferred).
- The styleguide shows `AnimatedNumber` with a control to change the value.
- Hand-written `Tabs`/`ThemeToggle` remain; beUI versions are deferred (YAGNI).

## Risk Class

`medium`

Adds a new runtime dependency (Motion) to a lightweight app; otherwise additive.

## Impact Areas

- `package.json` / lockfile (add `motion`)
- `components.json` (beUI registry)
- `src/components/ui/animated-number.tsx` (new, adapted from beUI, MIT)
- `src/features/styleguide/StyleguideView.tsx` (demo section)

## Design Review

- What complexity is being introduced? One small Motion-based number animation
  and a registry config entry.
- Which decisions are hidden inside the owning module? Easing, duration, and the
  reduced-motion short-circuit.
- Is each new interface simpler than its implementation? Yes — `AnimatedNumber`
  takes `value` (+ optional `format`/`duration`) and hides the animation.
- What special cases exist, and can the design eliminate them? Reduced-motion is
  handled as a single guarded path; no viewport gating needed for the tray.

## Checklist

- [x] Install `motion` (pinned exact).
- [x] Add the beUI registry to `components.json`.
- [x] Add `AnimatedNumber` adapted from beUI into `src/components/ui/`.
- [x] Add a styleguide demo section with a value control.
- [x] Add a reduced-motion behavior test.
- [x] Run verification.

## Test Plan

- Behavior and invariants to prove:
  - With reduced motion preferred, `AnimatedNumber` renders the formatted target
    value immediately.
  - A custom `format` is applied.
- Lowest stable test layer: RTL component test with a `matchMedia` stub.
- Relevant commands: `pnpm test`, `pnpm verify:fast`.

## Decisions

- Adopt only `AnimatedNumber` for now; `NumberTicker` (rolling digits) is flashier
  than the calm utility aesthetic wants. `Tabs`/`ThemeToggle` stay hand-written.
- Adapt beUI source into a Burnly-owned component (our `@/lib/cn`, no
  `"use client"`, animate-on-change without IntersectionObserver) rather than a
  raw CLI vendor, since the tray is always visible and our lib paths differ.
- beUI registry still configured so `pnpm dlx shadcn add @beui/<name>` works later.

## Verification

- Command: `pnpm test src/components/ui/animated-number.test.tsx`
- Outcome: passed (2 tests).
- Command: `pnpm test`
- Outcome: passed (112 tests, no regressions).
- Command: `pnpm verify:fast`
- Outcome: passed (exit 0). Initial run flagged
  `react-hooks/set-state-in-effect` on the reduced-motion branch; fixed by
  rendering the target value directly under reduced motion so `setState` only
  occurs inside Motion's async `onUpdate` callback.

## Motion dependency

- Added `motion@12.42.0` (pinned exact). New runtime dependency for a lightweight
  app; adoption kept surgical (one component) per the master plan's decision.

## Runtime Evidence

- Styleguide screenshots are Phase 5b; not captured here.

## Follow-Up Debt

- Phase 2: extract tray inline patterns into Burnly compact components (can use
  `AnimatedNumber` for the primary metric).
