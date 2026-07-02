# Burnly Tray Panel Product Contract

## Status

Draft product contract.

This document defines the target content and layout for Burnly's primary tray
panel experience. It should guide design-system and implementation work.

## Product Role

The tray panel is Burnly's main product surface.

It should answer, at a glance:

```text
How many AI coding-tool tokens have I used today?
```

Detail that does not fit the default summary is added as tabs inside the tray
panel (settings now, sessions later). Reporting and history are deferred to a
future web product. There is no full desktop window.

## Layout Summary

The tray panel should use this hierarchy:

```text
Header / freshness

Main metric:
  Today token usage

Secondary metrics row:
  This week tokens     This month tokens

Model usage allocation:
  Model A              tokens / agent / change
  Model B              tokens / agent / change
  Model C              tokens / agent / change
  Model D              tokens / agent / change

Actions:
  Settings / Sessions tabs (sessions later)
```

## Visual Reference

The model usage allocation should follow the structure of an asset-allocation
list:

```text
MODEL USAGE

| GPT-5.1          25,000 tokens
| Codex              ↗ 8.5%

| Claude Sonnet    12,000 tokens
| Claude Code        ↗ 3.2%

| GPT-5              5,678 tokens
| Codex              ↗ 22.1%

| GLM-5.2            3,000 tokens
| ZCode              ↗ 0.5%
```

Each row should include:

- a thin color indicator,
- model name,
- token total,
- coding agent/source name,
- optional trend/change indicator.

## Required Content

### Header

The header should show:

- Burnly name or compact logo,
- freshness state.

Possible freshness copy:

- `Updated just now`
- `Updated 2m ago`
- `Refreshing...`
- `Refresh failed`
- `Some sources failed`

### Main Metric

The most prominent element is:

- today's total token usage.

Display requirements:

- visually largest number in the panel,
- clear label: `Today`,
- token unit visible,
- should not compete with cost or other secondary metrics.

Example:

```text
Today
42,180 tokens
```

### Secondary Metric Row

Below the main metric, show two compact metrics in a row:

- this week tokens,
- this month tokens.

Example:

```text
This week       This month
183,240         612,900
tokens          tokens
```

These are secondary. They should be readable but smaller than today's usage.

### Model Usage Allocation

Below the secondary metric row, show model usage allocation.

Default ranking:

- highest token usage first.

Default row count:

- all models with usage today.

Do not aggregate tray rows into a generic `Other` bucket. If an upstream source
cannot provide per-model splits, represent that limitation at collection time
with the source-specific model label, for example OpenCode-family
`Multiple models`.

Each model row should show:

- color indicator,
- model display name,
- token total,
- coding agent/source name,
- trend compared with yesterday.

Do not show percentage-of-usage text in the first version. We do not currently
have a product requirement for usage-share percentages in this compact panel.

Model usage period:

- today only.

Trend comparison:

- compare today's model usage against yesterday's model usage.

## Optional Content

These are intentionally omitted from the first tray panel version:

- estimated cost,
- source split,
- seven-day mini trend.

Failed source state can still appear as a compact freshness/status warning
because it affects trust in the displayed token data.

## Actions

The default summary is one tab of the tray panel. Navigation to other local tabs
(`Settings` now, `Sessions` later) is the primary action surface. There is no
`Open details` action and no full desktop window.

Refresh should not be a primary tray-panel button.

Auto-refresh behavior:

- refresh on app start,
- refresh on a background interval,
- refresh when the tray panel opens if data is stale,
- throttle tray-open refresh so collectors are not started repeatedly,
- never start overlapping refresh jobs,
- show refreshing, failed, or partial state safely.

Manual refresh behavior:

- available from a settings tab or a small overflow menu if needed,
- not part of the primary tray layout.

Reporting and history are not tray actions; they live on the future web product.

## Empty State

If no data exists:

```text
No usage collected today

Burnly will update automatically.
```

Also show a small hint:

```text
Burnly reads local usage from supported AI coding tools.
```

Do not show a large empty dashboard in the tray panel.

## Error / Partial State

If refresh fails:

```text
Refresh failed
Last successful update: 10:42
```

If only some sources fail:

```text
Some sources failed
Codex updated. Claude failed.
```

The tray panel should use safe, short messages. There is no diagnostics surface;
keep error copy compact and human-readable.

## Cost Position

Cost is omitted from the first tray panel version.

Reason:

- the product reset centers token tracking,
- cost availability differs by source/model,
- showing cost too prominently can pull Burnly back toward budget/cost-control
  positioning.

Cost can later appear as:

- a small secondary line under today's tokens,
- a tooltip/detail,
- a metric on the future web product.

## Source Split Position

Source split is omitted from the first tray panel version.

The first tray panel version should prioritize model usage allocation.

Source split can be added later if the panel has enough space or if user testing
shows source is more important than model.

## Size And Density

Target feel:

- compact,
- scannable,
- calm,
- data-dense enough to be useful,
- not a full dashboard.

The tray panel should avoid:

- multi-tab navigation,
- complex filters,
- large charts,
- budget setup,
- export controls,
- diagnostics detail.

## Design-System Implications

The design system should include compact components for:

- main metric,
- secondary metric,
- allocation row,
- trend indicator,
- refresh status,
- compact actions,
- empty state,
- partial/error state.

These should be designed for the tray panel first, then reused in tray tabs when
appropriate.

## Open Decisions

- Exact tray panel width and height.
