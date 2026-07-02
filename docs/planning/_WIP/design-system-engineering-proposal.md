# WIP: Tray-First Design System Engineering Proposal

## Status

Draft proposal.

This document replaces the previous dashboard-oriented design-system proposal.
It aligns the design system with Burnly's corrected product direction:

```text
tray-first compact token tracker
  -> optional full details window
```

## Problem

Burnly currently has working product foundations, but the UI lacks a coherent
system and has drifted toward a full desktop dashboard.

The design system must correct that trajectory.

It should make the compact tray/menu-bar experience feel intentional before
expanding the full desktop window.

## Goal

Create a small Burnly-owned design system for:

- compact tray panels,
- small modal/detail surfaces,
- concise metric cards,
- source/token status displays,
- secondary full-window details.

The system should support quick reading and low-friction monitoring.

## Non-Goals

- Building a large dashboard component library.
- Optimizing first for complex data exploration.
- Creating generic saved/custom view infrastructure.
- Making budgets a primary design axis.
- Adding decorative motion.
- Adding Storybook before the component model stabilizes.

## Current Stack

Keep the current stack:

- React.
- TypeScript.
- Tailwind CSS v4.
- shadcn token conventions.
- Radix primitives.
- Lucide icons.
- `class-variance-authority`.
- `tailwind-merge`.

The stack is sufficient. The missing layer is Burnly-specific product design.

## Design System Principles

### Compact First

Components should work in small spaces.

The tray panel should be the proving surface for the system.

### Data Over Decoration

Every visual element should help the user read usage, source, status, or action.

Avoid decorative effects that do not improve comprehension.

### Utility Feel

Burnly should feel like a serious local utility:

- calm,
- fast,
- private,
- precise,
- lightweight.

### Secondary Detail

Full-window components should support deeper inspection, but they should not
force the product toward a dashboard-first layout.

## Third-Party Component Position

### Tailwind

Keep Tailwind. Use it through Burnly-owned components and semantic tokens rather
than repeated one-off utility strings in feature screens.

### shadcn

Use shadcn as the baseline style/token convention for primitives.

Good candidates:

- Button.
- Card.
- Badge.
- Dialog.
- Tooltip.
- Popover.
- Switch.
- Checkbox.
- Skeleton.

### Radix

Use Radix for accessibility-sensitive primitives:

- dialog,
- popover,
- tooltip,
- tabs,
- switch,
- dropdown.

### beUI

Do not adopt beUI broadly.

Potential future candidates:

- number animation for metric changes,
- animated badge for refresh state,
- drawer only if full-window details need it.

Avoid initially:

- tilt cards,
- dock,
- dynamic island,
- marquee,
- morphing modal,
- magnetic buttons.

Rule:

No beUI component should enter the app until a specific product interaction
needs it. If adopted, it should be wrapped or adapted into Burnly-owned
components.

## Proposed Component Layers

```text
tokens
  -> generic UI primitives
  -> Burnly compact components
  -> tray panel / full details screens
```

## Layer 1: Tokens

Define tokens for:

- app background,
- compact panel background,
- elevated panel,
- card,
- border,
- text primary,
- text secondary,
- text muted,
- accent,
- success,
- warning,
- error,
- unavailable,
- source colors,
- token category colors.

The token set should support dark mode first.

## Layer 2: Generic UI Primitives

Target path:

```text
src/components/ui/
```

Initial primitives:

- `Button`
- `Card`
- `Badge`
- `Tooltip`
- `Popover`
- `Dialog`
- `Skeleton`
- `Separator`
- `ScrollArea` if needed
- `EmptyState`
- `ErrorState`

Rules:

- No Burnly domain concepts.
- No IPC.
- No feature hooks.
- Accessibility defaults must be handled here where practical.

## Layer 3: Burnly Compact Components

Target path:

```text
src/components/burnly/
```

Initial components:

- `CompactMetric`
- `CompactMetricRow`
- `SourceUsageRow`
- `SourceBadge`
- `TokenTotal`
- `MiniTrend`
- `RefreshStatus`
- `DataFreshness`
- `DataQualityNote`
- `OpenDetailsButton`

Rules:

- May understand Burnly concepts such as source, token category, cost
  availability, and refresh state.
- Should receive data through props.
- Should not call IPC directly.

## Layer 4: Product Surfaces

Primary:

- tray/menu-bar panel.

Secondary:

- full details window.
- settings.
- diagnostics.

The design system should be validated against the tray panel first.

## Tray Panel Layout Target

Candidate compact layout:

```text
Burnly                         refreshed 2m ago

Today
42.1k tokens

This week              This month
183.2k tokens          612.9k tokens

Model usage
GPT-5.1                25.0k tokens
Codex                  ↗ 8.5% vs yesterday

Claude Sonnet          12.0k tokens
Claude Code            ↘ 3.2% vs yesterday

GLM-5.2                 3.0k tokens
ZCode                   → 0.0% vs yesterday

[Open details]
```

Optional additions:

- warning when source import failed.

Do not add:

- large charts,
- cost,
- source split,
- complex filters,
- budget setup,
- multi-tab navigation inside the tray panel.

## Full Details Layout Target

The full window can keep detailed views, but the layout should feel secondary.

Target navigation:

- Summary.
- Sessions.
- History.
- Settings.
- Diagnostics.

`Open details` from the tray panel lands on `Summary`.

Calendar-style history belongs inside `History`, not as a separate primary
destination.

Budgets should be hidden, moved to an advanced area, or explicitly revalidated
before returning to primary navigation.

## Motion Policy

Allowed:

- short hover transitions,
- refresh spinner,
- subtle metric update animation,
- panel open/close transition,
- status badge transition.

Avoid:

- 3D effects,
- bouncy navigation,
- animated backgrounds,
- cursor-following effects,
- decorative continuous motion.

All motion must respect reduced-motion preferences.

## Accessibility Requirements

- Tray panel must be keyboard reachable.
- Buttons need visible focus states.
- Status cannot rely only on color.
- Tooltips must not contain required information.
- Text contrast must work on dark surfaces.
- Reduced-motion must be respected.
- Compact UI must remain readable at small window sizes.

## Implementation Shape

### DS-1: Token Reset

Deliver:

- Burnly semantic tokens in `global.css`.
- Source/status/token colors.
- Compact panel surface tokens.

### DS-2: Compact Primitives

Deliver:

- `Card`.
- `Badge`.
- `Tooltip`.
- `Skeleton`.
- `EmptyState`.
- `ErrorState`.
- `Separator`.

### DS-3: Burnly Compact Components

Deliver:

- `CompactMetric`.
- `SourceUsageRow`.
- `RefreshStatus`.
- auto-refresh freshness state.
- `DataFreshness`.
- `DataQualityNote`.

### DS-4: Tray Panel Prototype

Deliver:

- Static tray panel layout using real or fixture-shaped props.
- No new broad data exploration scope.

### DS-5: Full Details Reskin

Deliver:

- Apply primitives to existing full-window surfaces after tray panel direction
  is approved.

## Verification

For design-system work:

- `pnpm format:check`.
- `pnpm typecheck`.
- relevant React tests.
- screenshots/manual runtime evidence for tray panel.

Do not claim visual quality from automated tests alone.

## Open Decisions

- Should the tray panel be implemented as a Tauri window or another platform
  surface?
- Should budgets be hidden from top-level navigation?
- Should future leaderboard metrics influence compact panel labels now?

## Recommendation

Build the design system only as far as needed to support the compact tray
tracker and secondary details.

Do not build a dashboard-first design system.
