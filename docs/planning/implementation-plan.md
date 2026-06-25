# Burnly Corrected Implementation Plan

## Status

Proposed corrected roadmap.

This document replaces the previous dashboard-first phase roadmap. It aligns
implementation with the current product source of truth:

- Burnly is tray-first.
- The compact tracker is the primary experience.
- The full desktop window is secondary.
- Budgets and heavy analytics are not the immediate product center.
- Future sync and leaderboard features are opt-in and later.

## Current Baseline

Burnly already has substantial implementation foundation:

- Tauri desktop app.
- React/TypeScript frontend.
- Rust application/backend layers.
- SQLite persistence.
- Typed IPC.
- Local `ccusage` sidecar integration.
- Claude Code, Codex, and OpenCode data import.
- Manual refresh.
- Overview, Calendar, Sessions, Settings, Diagnostics, and Budgets screens.
- Export, history deletion, diagnostics, and maintenance foundations.
- Linux packaging/install validation.

This foundation is useful, but the product experience must be redirected.

## Corrected Product Target

The first cohesive product should feel like:

```text
tray/menu bar icon
  -> compact token tracker panel
  -> optional full details window
```

The user should not need to open a full dashboard to answer the basic question:

```text
How much AI coding-tool usage have I burned today?
```

## Planning Principles

- Stop expanding dashboard scope until the tray-first product shape is settled.
- Treat the full window as detail mode, not the primary product.
- Keep budgets, export, and diagnostics available but visually secondary.
- Build design-system primitives around compact utility surfaces first.
- Preserve existing working ingestion/storage code unless it blocks the product
  correction.
- Avoid deleting useful implementation solely because the product direction
  changed.
- Do not add new implementation phases until product reset documents are agreed.

## Near-Term Work Categories

These are not implementation phases yet. They are corrected work categories that
future execution plans can be derived from.

### 1. Product Reset And Scope Cleanup

Goal:

Make documentation and planning stop pulling the product toward a full analytics
dashboard.

Deliverables:

- Updated product document.
- Drift analysis.
- Corrected implementation plan.
- Stale active/queued execution plans removed or completed.
- Design-system proposal rewritten for tray-first utility.

### 2. Design System For Compact Utility UI

Goal:

Create a small Burnly-owned design system that supports tray panel, compact
modal surfaces, and secondary full-window detail views.

Deliverables:

- Semantic tokens.
- Core primitives.
- Compact metric components.
- Status badges.
- Source badges.
- Token breakdown components.
- Empty/loading/error states.
- Drawer/detail patterns.

Important:

The design system should not assume a large dashboard is the primary surface.

### 3. Tray Panel Product Experience

Goal:

Make the tray/menu-bar panel the main Burnly experience.

Deliverables:

- Compact daily usage summary.
- This week and this month token summaries.
- Today's model usage allocation with coding-agent labels.
- Model trend compared with yesterday.
- Refresh status.
- Automatic refresh on startup, interval, and stale tray-panel open.
- Secondary manual retry only outside the primary tray layout.
- Open full details action.
- Clear empty/error states.

Linux should be proven first because it is the current tested install target.
Windows and macOS should follow later with platform-specific validation.

### 4. Full Details As Secondary Experience

Goal:

Keep the full window useful without letting it dominate the product.

Deliverables:

- Simplified navigation with `Summary`, `Sessions`, `History`, `Settings`, and
  `Diagnostics`.
- Detail views for sessions/history.
- Diagnostics and settings placed as support tools.
- Budgets hidden or de-emphasized from primary navigation until explicitly
  revalidated.
- Calendar-style history folded into `History` rather than kept as a separate
  primary destination.

### 5. Future Sync And Leaderboard Preparation

Goal:

Prepare the metric model for future opt-in web/social features without building
sync prematurely.

Deliverables:

- Document aggregate metrics that are safe to sync.
- Document privacy boundaries.
- Distinguish local-only detail from future public metrics.
- Avoid coupling current local UI to future leaderboard assumptions.

## Suggested Next Documentation Work

Before code work resumes:

1. Finish product drift analysis.
2. Rewrite design-system proposal for tray-first UI.
3. Decide whether the full window uses top tabs, sidebar, or a reduced support
   layout for the locked screen set.

## Suggested Next Implementation Shape

After documentation is approved, the next implementation should likely be:

```text
compact usage summary API
  -> tray panel layout
  -> design-system primitives needed by that panel
  -> full details entry point
```

This is intentionally narrower than a broad "Phase 11 dashboard exploration"
effort.

## What To Keep

Keep the existing code and docs that support:

- source collection,
- refresh correctness,
- local storage,
- typed IPC,
- privacy controls,
- diagnostics needed to explain missing data,
- package/install evidence,
- platform behavior evidence,
- historical completed execution plans.

## What To De-Emphasize

Do not let these drive near-term product shape:

- budget management,
- complex custom dashboards,
- advanced filtering,
- broad export workflows,
- enterprise diagnostics,
- public leaderboard UI.

Some of these may remain implemented, but they should not define the primary
experience.

## What To Remove From Forward Planning

Remove or rewrite forward-looking plans that assume:

- full dashboard as primary surface,
- budgets as core product,
- usage exploration as the immediate product center,
- cross-platform expansion before Linux tray-first experience is coherent,
- release-candidate polish before product shape is corrected.

## Verification Approach

For documentation reset:

- Markdown formatting must pass.
- Docs index must point to current source-of-truth documents.
- Active/queued execution plans should not contain stale direction.

For future implementation:

- Continue using `pnpm verify:fast`, `pnpm verify`, and relevant runtime gates.
- Runtime evidence should prioritize tray/menu-bar behavior.
- Full desktop evidence remains secondary.

## Open Decisions

- Should the full details window use top tabs, sidebar, or another reduced
  support layout?
- Should the product support a small always-on floating window later?
- Which aggregate metrics are safe candidates for future leaderboard sync?

## Current Recommendation

Do not continue phase-based implementation yet.

First finish the documentation reset and product correction. Then define one
small execution plan for the tray-first compact tracker experience.
