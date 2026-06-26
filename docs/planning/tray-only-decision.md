# Decision: Burnly Is A Tray-Only Local Tracker

## Status

Accepted on 2026-06-26.

This decision supersedes the "full desktop window as secondary experience"
direction in earlier product and planning documents.

## Context

Burnly began as a tray-first AI coding-tool token tracker. During the initial
MVP it accumulated a full desktop window with Overview, Calendar, Sessions,
Budgets, Diagnostics, export, history, and database-maintenance/recovery
surfaces. The tray-first correction (see `product-drift-analysis.md`) reframed
the desktop window as secondary but kept it in the product.

After the tray panel was built and proven on Linux with real local data, the
full desktop window no longer earns its place:

- It duplicates reporting that belongs on a future web product.
- It is the main source of the codebase's size and over-engineering.
- The roadmap already plans a website for leaderboards and social features,
  which is the natural home for analytics, calendar, and history exploration.

## Decision

Burnly's local app is **tray-only**. There is no full desktop window.

- The local product is the compact tray panel plus its tracker spine
  (collect -> reconcile -> refresh -> compact tray summary).
- Local detail surfaces (e.g. Settings, and later Sessions) will be added as
  **tabs inside the tray panel**, not as a separate window.
- Reporting (calendar, overview/usage report, history, trends) is deferred to a
  future **web product** that derives it from synced aggregate daily usage
  facts. The local app does not render this reporting.
- Leaderboard and social features remain future, opt-in, and web-only.

## Scope Of Removal

Deleted from the local app (frontend and backend):

- Full desktop window and its shell/navigation.
- Overview, Calendar, Budgets, Sessions, and Diagnostics surfaces.
- Export, history listing, and history deletion.
- Database maintenance and recovery. On startup failure the tray shows an error
  state with no in-app repair path.
- Their IPC commands, application services, SQLite read stores, ports, and the
  budget domain type.

Kept:

- Tracker spine: collectors, reconciliation, refresh coordinator/scheduler, the
  compact tray summary read model, settings, bootstrap, tray + tray-panel window
  code, project-path privacy, migrations.
- Settings backend/IPC/data hook, so a future tray Settings tab drops in. Only
  the desktop Settings view is removed now.
- The data ingestion model is unchanged: daily, model, and session facts keep
  being reconciled into SQLite. Removing reporting removes only the read paths,
  not the stored facts.

## Rationale

- Avoids building analytics UI twice (desktop and web).
- The web is the correct home for cross-device reporting, sharing, and
  leaderboards, and can iterate without desktop releases.
- Calendar/overview/trends are pure derivations of synced daily facts, so no
  product capability is permanently lost by deleting the local query code.
- Keeps the local app small, fast, and utility-like, matching the product
  principles.

## Consequences

- The local app cannot show historical reporting or calendar views until the web
  product exists. The tray answers "how much today/this week/this month?"; the
  web will answer "show me history and how I compare."
- No in-app database recovery. A corrupt local database is resolved by resetting
  local data, not by an in-app maintenance tool.
- Sessions reporting does not move to web automatically, because session detail
  carries sensitive local metadata (project paths) that is not sync-safe. A
  future tray Sessions tab can expose it locally.
- The budgets SQLite table and migration `0003` remain in the schema (already
  applied); only the budget code is removed.
- Sync, accounts, and the web product are still undesigned and remain future
  work. This decision does not commit to a sync protocol.

## Implementation

Tracked by the tray-only strip execution plans:

- Phase overview: `docs/exec-plans/active/2026-06-26_tray-only-strip-overview.md`

## References

- `docs/product/product.md`
- `docs/planning/product-drift-analysis.md`
- `docs/planning/implementation-plan.md`
- `docs/planning/tray-first-implementation-plan.md`
