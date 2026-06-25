# Burnly Product Document

## Product Summary

Burnly is a tray-first AI coding-tool token tracker.

It runs locally, watches usage from supported tools such as Claude Code, Codex,
and OpenCode, and gives developers a compact view of their current token usage
without requiring them to open a full desktop window.

The primary experience is a small tray/menu-bar panel. A full desktop window is
available only for details, settings, diagnostics, and future account/sync
features.

## Product Positioning

Burnly is not primarily a dashboard.

Burnly is not primarily a budget manager.

Burnly is not primarily an enterprise reporting tool.

Burnly is a lightweight tracker for developers who want to know how much AI
coding-tool usage they are burning through during the day.

## Vision

Make AI coding-tool usage visible with minimal friction.

Burnly should feel like a small utility that is always available from the system
tray, not like an application the user must manage.

Longer term, Burnly can optionally connect to a web product for sync,
leaderboards, profiles, and community features. Local tracking remains the
foundation.

## Product Principles

### Tray-first

The default interaction is quick open, quick read, quick close.

The user should be able to answer "how much have I used today?" from the tray
panel in seconds.

### Compact by default

The primary UI should be small and focused.

The full desktop window is secondary and should not become the product's center
of gravity.

### Tracker before analytics

The core product value is current and recent token usage tracking.

Advanced analytics, custom reports, budget management, and complex filtering are
secondary.

### Local-first privacy

Burnly's local experience must not require an account.

Burnly should not collect prompts, responses, source code, or file contents.

Project paths and names can reveal sensitive information and must remain under
user control.

### Honest usage data

Different tools report usage differently.

Burnly should clearly distinguish measured tokens, estimated cost, unavailable
cost, and incomplete source data.

### Future social features are opt-in

Sync, leaderboard, public profiles, and community features are future optional
experiences.

Nothing local should become public without explicit user action.

## Target Users

### Primary Users

- Developers who use AI coding tools daily.
- Developers who use more than one coding assistant.
- Developers who want a lightweight token tracker in the tray/menu bar.
- Developers who want private local tracking before opting into any web/social
  feature.

### Future Users

- Developers who want to compare optional public activity metrics.
- Developers who may later want cross-device web sync or public activity
  surfaces.
- Teams or communities that want opt-in aggregate visibility.

Team and organization workflows are not part of the immediate product center.

## Core User Problems

- Token usage is easy to lose track of during the day.
- Usage is split across different tools.
- Existing reports require commands, dashboards, or manual checking.
- Developers want quick visibility without opening a heavy app.
- Developers may want future streaks, rankings, or public activity metrics, but
  only after local tracking is reliable and private.

## Primary Experience: Tray Panel

The tray/menu-bar panel is Burnly's main product surface.

It should show:

- today's total token usage,
- this week's token usage,
- this month's token usage,
- today's model usage with coding-agent labels,
- model usage trend compared with yesterday,
- freshness state,
- entry point to full details.

The panel should be compact enough to open frequently during normal work.

## Secondary Experience: Full Desktop Window

The full desktop window exists for tasks that do not fit in the tray panel.

Appropriate full-window surfaces:

- `Summary`
- `Sessions`
- `History`
- `Settings`
- `Diagnostics`

`Summary` is the default landing view when the user selects `Open details` from
the tray panel.

`History` owns calendar-style history. Calendar should not remain a separate
primary top-level destination unless it is revalidated later.

Diagnostics can contain support actions such as export, maintenance, and
detailed import/refresh evidence.

The full window should support detail, but it should not define the product's
primary identity.

## Future Web And Leaderboard Direction

Burnly may later connect to a website for:

- optional account,
- optional sync,
- public profile,
- leaderboard,
- streaks,
- activity summaries,
- community comparisons.

Likely leaderboard metrics:

- daily tokens,
- weekly tokens,
- active days,
- streaks,
- session count,
- source diversity,
- anonymized project count.

Leaderboard data must be opt-in and should avoid exposing prompts, responses,
source code, local file paths, or sensitive project names.

## MVP Scope

### Included

- Local app install.
- Tray/menu-bar availability.
- Compact tray panel.
- Automatic/background refresh.
- Manual refresh only as a secondary recovery/debug action.
- Local usage collection for supported sources.
- Today's usage summary.
- This week and this month token summaries.
- Today's model usage allocation.
- Recent usage trend.
- Full details window.
- Full details default landing on Summary.
- Basic history.
- Basic sessions view.
- Settings.
- Diagnostics sufficient to explain missing data.
- Local data storage.
- Privacy controls for project/path handling.

### De-emphasized

These may exist internally or in secondary screens, but they should not dominate
the product:

- budgets,
- standalone Calendar top-level navigation,
- heavy dashboards,
- complex custom views,
- advanced exports,
- maintenance tools,
- enterprise reporting.

### Not Included In Immediate MVP

- Required account.
- Required cloud sync.
- Public leaderboard.
- Team workspace.
- Organization reporting.
- Billing reconciliation.
- Prompt, response, or source-code tracking.
- Generic query builder.

## Key User Journeys

### Quick Check

1. The user opens Burnly from the tray/menu bar.
2. The tray panel shows today's tokens, week/month tokens, model usage, and
   freshness state.
3. The user closes the panel and continues working.

### Automatic Refresh

1. Burnly refreshes automatically on startup, on a background interval, and when
   the tray panel opens if data is stale.
2. The tray panel shows freshness state.
3. If a source fails, Burnly shows a compact warning and offers diagnostics from
   the full window.

### Inspect Details

1. The user notices high usage in the tray panel.
2. The user opens full details.
3. Burnly shows sessions, sources, models, and recent history.
4. The user identifies what caused the usage.

### Future Opt-In Sync

1. The user opens account/sync setup.
2. Burnly explains exactly which metrics can sync.
3. The user opts in.
4. Only selected aggregate metrics are synced.

## Design Direction

Burnly should feel:

- compact,
- calm,
- data-focused,
- utility-like,
- fast,
- private.

It should not feel:

- like a large admin dashboard,
- like an enterprise analytics suite,
- like a gamified app before the user opts into social features,
- like a budget/payments product.

## Success Measures

Near-term success should be measured by:

- successful local source detection,
- successful refresh,
- tray panel open frequency,
- quick-check completion,
- percentage of users who return after first day,
- percentage of users who open details from tray,
- user confidence that usage data is accurate enough.

No success measure should reward higher token consumption.

Future web/social success can include opt-in profile or leaderboard activity,
but only after the local tracker is valuable on its own.

## Product Roadmap Shape

### Stage 1: Local Tray Tracker

Deliver a reliable local tray-first token tracker with compact daily usage and
optional full details.

### Stage 2: Better Local Detail

Improve sessions, history, source/model/project breakdowns, and data-quality
explanations inside the full window.

### Stage 3: Optional Sync Foundation

Add account and sync only for selected aggregate metrics.

### Stage 4: Public Profile And Leaderboard

Add opt-in public activity surfaces after privacy controls and metric semantics
are clear.

### Stage 5: Team Or Community Expansion

Consider team/community use only after personal tracking and optional public
profiles are proven.

## Current Product Correction

The current implementation has accumulated full-window dashboard, budget,
diagnostic, export, and release-preparation work. Some of that code can remain
useful, but the product direction is corrected here:

- tray-first tracker is primary,
- full desktop app is secondary,
- budgets are not core,
- dashboard exploration is not the immediate center,
- future leaderboard affects metric design but remains opt-in and later.

Future plans should be evaluated against this document before implementation.
