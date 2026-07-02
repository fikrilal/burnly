# Burnly Tray-First Implementation Plan

## Status

Linux tray-first MVP completed. Partially superseded by the tray-only decision.

This document records the tray-first implementation direction and completed
Linux execution sequence. As of 2026-06-26, Burnly is **tray-only**: the full
desktop window and the `Open details` action are being removed (see
`docs/planning/tray-only-decision.md`). References below to a full desktop window
or `Open details` are historical; local detail now lives as tray tabs. Future
platform expansion should still be split into focused execution plans under
`docs/exec-plans/`.

## Product Contract

Source of truth:

- [Tray panel product contract](../product/tray-panel-contract.md)

The target product shape is:

```text
tray/menu bar icon
  -> compact token tracker panel
  -> local detail tabs (settings now, sessions later)
```

The tray panel is Burnly's entire local surface. There is no full desktop
window.

## Current Baseline

Burnly currently has:

- working local collection through bundled `ccusage`,
- Claude Code, Codex, and OpenCode imports,
- SQLite persistence,
- daily/model/session usage facts,
- refresh coordinator,
- background refresh scheduler,
- typed IPC,
- native tray menu,
- full desktop window,
- Overview, Calendar, Sessions, Settings, Diagnostics, and Budgets screens,
- Linux debug package install evidence.

This is enough foundation to build the tray-first product without rewriting the
collector/storage layers.

## Target Outcome

Burnly should expose a compact tray panel containing:

- today total tokens,
- this week total tokens,
- this month total tokens,
- today's model usage allocation,
- coding-agent/source label per model row,
- trend compared with yesterday,
- freshness state,
- `Open details` action.

Tray v1 intentionally omits:

- cost,
- source split,
- budget setup,
- export,
- diagnostics detail,
- complex filters,
- primary manual refresh button.

`Open details` opens/focuses the full desktop window and lands on `Summary`.

## Architecture Direction

### Data

Add one purpose-built compact tray summary read model.

Do not make the tray panel compose broad Overview, Calendar, Session, or Budget
queries in React.

The read model should be owned by Rust application usage code and backed by
SQLite queries.

Required data:

- reporting-timezone-aware today total,
- reporting-timezone-aware week total,
- reporting-timezone-aware month total,
- today's model rows,
- yesterday model totals for trend comparison,
- coding-agent/source label per model row,
- last successful refresh timestamp,
- freshness/data-quality state.

### IPC

Expose a dedicated typed IPC command for the compact tray summary.

The wire contract must not include cost or source split in tray v1.

Frontend code should consume this through `src/ipc/` only.

### Refresh

Auto-refresh is primary.

Refresh should happen:

- on app start when appropriate,
- on the existing scheduled background interval,
- when the tray panel opens and data is stale.

Tray-open refresh must be throttled and must not start overlapping jobs. Existing
refresh coordinator coalescing should remain authoritative.

Manual refresh is secondary recovery/debug behavior and should not be a primary
tray-panel action.

### Tray Panel Window

The existing native tray menu cannot render the required compact React layout.

The likely implementation is a dedicated compact Tauri window or route-backed
panel opened from tray interaction.

The platform layer owns:

- tray/menu-bar events,
- compact panel creation/focus/hide behavior,
- stale tray-open refresh trigger,
- full-window open/focus behavior.

### UI

Build only the design-system primitives needed by tray v1:

- compact main metric,
- secondary metric row,
- allocation row,
- trend indicator,
- freshness/status indicator,
- compact empty state,
- compact partial/error state,
- `Open details` action.

Do not redesign the full desktop app in the tray-first implementation pass.

### Full Details

Full desktop redesign is later.

For tray-first work, full details only needs a stable `Summary` landing target.
If a full Summary route/screen is not ready, the first implementation may use a
minimal landing view that can later be redesigned.

## Implementation Sequence

### Step 1: Compact Summary Read Model

Purpose:

Create the authoritative data contract before building tray UI or window
behavior.

Expected execution plan:

- [Tray compact summary read model](../exec-plans/completed/2026-06-25_tray-compact-summary-read-model.md)

### Step 2: Tray Auto-Refresh And Compact Window

Purpose:

Make tray interaction open the compact product surface and wire refresh behavior
correctly.

Expected execution plan:

- [Tray auto-refresh and compact window](../exec-plans/completed/2026-06-25_tray-auto-refresh-window.md)

### Step 3: Compact Tray UI

Purpose:

Render the tray panel using the compact summary contract and minimal reusable UI
components.

Expected execution plan:

- [Tray compact UI](../exec-plans/completed/2026-06-25_tray-compact-ui.md)

### Step 4: Linux Runtime Evidence

Purpose:

Prove the installed Linux app behaves like a tray-first product with real local
data.

Expected execution plan:

- [Tray Linux runtime evidence](../exec-plans/completed/2026-06-25_tray-linux-runtime-evidence.md)

## Testing Strategy

Behavior and invariants to prove:

- today/week/month totals use reporting timezone correctly,
- model usage is today's usage only,
- trend compares today against yesterday,
- no cost/source split leaks into the tray v1 contract,
- auto-refresh does not start overlapping refresh jobs,
- tray panel open uses stale-data throttling,
- `Open details` lands on Summary.

Lowest stable test layers:

- Rust usage query tests for compact summary,
- SQLite store tests for aggregation behavior,
- IPC contract and generated TypeScript checks,
- platform lifecycle/tray tests,
- React tests for compact tray states.

Runtime evidence:

- installed Linux package,
- tray interaction opens compact panel,
- real local data is visible,
- auto-refresh/freshness state is observable,
- `Open details` opens the full window on Summary.

## Risks

### Tray Window Behavior

Linux tray behavior can vary by desktop environment and tray host.

Mitigation:

Validate Linux first with installed-package evidence before extending to Windows
and macOS.

### Product Drift

The tray panel can easily become a small dashboard.

Mitigation:

Enforce tray v1 omissions: no cost, no source split, no filters, no budgets, no
diagnostics details.

### Data Contract Drift

Using existing Overview data may accidentally reintroduce dashboard assumptions.

Mitigation:

Use a dedicated compact summary contract.

### Refresh Spam

Refreshing on every tray open could run collectors too often.

Mitigation:

Add stale-data throttling and rely on refresh coordinator coalescing.

## Out Of Scope

- Full desktop redesign.
- Budgets redesign or removal.
- Source split in tray.
- Cost in tray.
- Export from tray.
- Diagnostics details in tray.
- Complex filters.
- Windows/macOS tray validation.
- Website, sync, or leaderboard implementation.

## Open Decisions

- Exact compact panel dimensions.
- Dedicated compact Tauri window vs route-backed panel in existing window.
- Full details navigation layout after tray work is proven.
- Whether budget UI is hidden, advanced, or removed later.

## Exit Criteria

Tray-first implementation is ready for product reassessment when:

- the installed Linux app opens a compact tray panel,
- the panel displays real local data,
- the panel auto-refreshes without a primary refresh button,
- `Open details` works,
- the UI feels like a compact tracker rather than a desktop dashboard.
