# Burnly Product Document

## Product Summary

Burnly is a tray-first AI coding-tool token tracker.

It runs locally, watches usage from supported tools such as Claude Code, Codex,
OpenCode, Pi, and experimental native collector sources such as Cline, ZCode,
and Antigravity, and gives developers a compact view of their current token
usage without requiring them to open a full desktop window.

The entire local experience is a small tray/menu-bar panel. There is no full
desktop window. Local detail surfaces such as settings live as tabs inside the
tray panel. Reporting, history, and leaderboards are deferred to a future web
product that derives them from synced usage data.

See `docs/planning/tray-only-decision.md` for the decision that removed the full
desktop window.

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

### Explicit source status

Every source should have a user-facing support status. Supported sources are
expected to work from stable local usage data. Experimental sources are usable
but may need follow-up if an upstream tool changes its local data format.
Unsupported sources should be visible in the roadmap rather than silently
implied.

Current source status:

| Tool        | Status            | Product note                                                                                                                                                                                                                                                                     |
| ----------- | ----------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Claude Code | Supported         | Collected through the bundled `ccusage` collector.                                                                                                                                                                                                                               |
| Codex       | Supported         | Collected through the bundled `ccusage` collector.                                                                                                                                                                                                                               |
| OpenCode    | Supported         | Collected through the bundled `ccusage` collector.                                                                                                                                                                                                                               |
| Pi          | Supported         | Collected through the bundled `ccusage` collector. Model labels keep the `[pi]` prefix.                                                                                                                                                                                          |
| Cline CLI   | Experimental      | Collected through Burnly's native local collector.                                                                                                                                                                                                                               |
| ZCode       | Experimental      | Collected through Burnly's native local SQLite collector.                                                                                                                                                                                                                        |
| Antigravity | Experimental      | Collected through Burnly's native collector across Antigravity 2.0, IDE, and CLI variants. CLI usage is read from local SQLite/protobuf metadata. App/IDE usage prefers runtime metadata sync, with experimental SQLite fallback and cached records when runtime is unavailable. |
| Cursor      | Not supported yet | Roadmap investigation.                                                                                                                                                                                                                                                           |
| Windsurf    | Not supported yet | Roadmap investigation.                                                                                                                                                                                                                                                           |
| Aider       | Not supported yet | Roadmap investigation.                                                                                                                                                                                                                                                           |
| Roo Code    | Not supported yet | Roadmap investigation.                                                                                                                                                                                                                                                           |
| Continue    | Not supported yet | Roadmap investigation.                                                                                                                                                                                                                                                           |
| Gemini CLI  | Not planned       | Deprecated upstream.                                                                                                                                                                                                                                                             |

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
- entry point to local detail tabs (settings now; sessions later).

The panel should be compact enough to open frequently during normal work.

## Local Detail: Tray Tabs

Burnly has no full desktop window. Detail that does not fit the default tray
summary is added as tabs inside the tray panel.

Planned local tabs:

- `Settings`
- `Sessions` (later)

Local tabs exist for what must stay on-device — settings, and session detail that
carries sensitive local metadata such as project paths. They should remain
compact and consistent with the tray panel; they must not recreate a dashboard.

## Reporting Lives On The Web

Calendar, usage reports, history exploration, trends, and comparison surfaces are
deferred to a future web product. The web derives them from synced aggregate
daily usage facts, so they are not built into the local app.

The local app answers "how much have I used today, this week, this month?" The
web app answers "show me my history and how I compare." Local tracking remains
useful on its own and never requires an account.

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
- Local tray tabs for settings now and sessions later.
- Local data storage.
- Privacy controls for project/path handling.

### Removed From Local Or Deferred To Web

These are not part of the local app. Reporting-style surfaces are deferred to the
future web product; the rest are removed:

- budgets (removed),
- calendar and usage reports (deferred to web),
- history exploration (deferred to web),
- heavy dashboards and custom views (deferred to web),
- advanced exports (removed),
- database maintenance and recovery tools (removed),
- enterprise reporting (not planned).

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
2. The user opens the tray model allocation and, later, a local Sessions tab.
3. For history and deeper reporting, the user opens the future web product.
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

The initial implementation accumulated a full-window dashboard, budgets,
diagnostics, export, history, and database-recovery work. As of 2026-06-26 the
local product is corrected to **tray-only** (see
`docs/planning/tray-only-decision.md`):

- the tray panel is the entire local experience,
- there is no full desktop window,
- local detail (settings now, sessions later) lives as tray tabs,
- budgets, export, history, diagnostics, and recovery are removed from local,
- calendar, usage reports, and history exploration are deferred to a future web
  product that derives them from synced daily usage facts,
- future leaderboard affects metric design but remains opt-in, web-only, and
  later.

Future plans should be evaluated against this document before implementation.
