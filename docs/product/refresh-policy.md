# Refresh Policy

## Status

Accepted product behavior.

This document defines Burnly's local refresh behavior after removing
user-configurable refresh settings. It is product policy first: engineering
details should follow this behavior unless a later product decision changes it.

## Context

Burnly's current refresh path is correctness-oriented but blunt.

Every trigger uses the same full-scope refresh path:

- startup stale refresh,
- tray-open stale refresh,
- resume refresh,
- scheduled refresh,
- manual refresh.

The coordinator builds `CollectionScope::Full` for daily and session collection
requests. The collector command only adds date bounds for
`CollectionScope::Incremental`, so the current scheduled refresh scans the full
available local history each time.

That is acceptable for early correctness, but it is too expensive for frequent
automatic refresh.

## Product Goal

Make automatic refresh cheap and reliable without losing historical correctness.

The policy should:

- run full refresh only when a baseline is missing or explicitly requested,
- use catch-up refresh for normal automatic operation,
- use today-only refresh when the tray opens and the user wants the freshest
  daily total,
- catch up across days when Burnly has not run recently,
- include a small lookback window for late or corrected local data,
- keep refresh behavior deterministic and observable.

## Non-Goals

- Adding a user-configurable refresh interval.
- Adding a user-facing full-resync button in the same implementation chunk.
- Rewriting collector adapters.
- Dropping existing full-refresh support.
- Building file watching before the incremental policy exists.

## Proposed Policy

### Initial Baseline

If a source/projection has no prior successful import, run a full refresh for
that missing source/projection.

This is the first-install path and the fallback for newly supported sources.

### Catch-Up Refresh

Scheduled, startup-after-gap, resume, and normal manual refresh should use an
incremental catch-up scope.

The scope should be based on the last successful import for each
source/projection, not just today's date.

Example:

```text
last successful daily import: 2026-06-20
today: 2026-06-28
incremental scope: 2026-06-18..2026-06-28
```

The two-day overlap is intentional. It lets Burnly reconcile late-arriving or
corrected records near the boundary.

### Tray Freshness Refresh

Opening the tray is a high-intent moment: the user wants to know how many tokens
they burned today.

If the tray's today summary is stale, Burnly should run a today-only refresh.

```text
today: 2026-06-28
tray freshness scope: 2026-06-28..2026-06-28
```

This path optimizes for speed and perceived freshness. It should update the
cached today row, after which week and month summaries can be recalculated from
SQLite using existing historical rows plus the updated today row.

The two-day lookback is not required for every tray-open freshness refresh. It
belongs to catch-up paths that protect against gaps, late writes, and boundary
corrections.

### Manual Refresh

Normal manual refresh should run the same incremental catch-up policy used by
startup, resume, and scheduled refresh.

This keeps manual refresh useful after Burnly has been closed for several days
without turning it into a full historical rescan.

### Manual Full Resync

A later, separate explicit action can run full refresh on demand.

Use cases:

- user wants to repair trust in local data,
- collector behavior changed,
- source identity changed,
- diagnostics or support asks for a full rebuild,
- future storage repair needs a safe resync path.

Full resync should not be the default manual refresh action.

## Trigger Matrix

```text
No successful import for source/projection       -> full
First install baseline                           -> full
Scheduled refresh                                -> incremental + lookback
Resume refresh                                   -> incremental + lookback
Startup-after-gap refresh                        -> incremental + lookback
Tray-open stale refresh                          -> today only
Normal manual refresh                            -> incremental + lookback
Explicit manual "resync all"                     -> full
Collector/profile incompatible with stored data  -> full or targeted rebuild
```

## Incremental Scope Rules

There are two incremental refresh shapes:

- catch-up refresh,
- freshness refresh.

### Catch-Up Scope

The incremental range should be computed per source/projection.

For daily imports, the identity should include:

- source,
- projection,
- aggregation timezone,
- collector/profile version when compatibility requires it.

For session imports, the identity should include:

- source,
- projection,
- collector/profile version when compatibility requires it.

Suggested range calculation:

```text
lookback_days = 2
start = min(today, last_successful_scope_end_date - 2 days)
end = today
```

The two-day lookback is a policy decision, not a placeholder. If a successful
refresh ran at 10 AM and the user kept using agents afterward, that calendar day
is not complete. Re-reading the prior two days gives Burnly room for same-day
continuation, timezone boundaries, late local writes, and collector finalization
quirks while still avoiding a full history scan.

### Freshness Scope

Freshness refresh is intentionally narrower.

For tray-open stale refresh:

```text
start = today
end = today
```

This scope should only be used after a baseline exists. If no successful import
exists for the source/projection, use the baseline/full path instead.

Open question:

- Should the lower bound use `last_successful_scope_end_date` or the latest
  successfully reconciled usage date/session activity? The import-run scope is
  simpler and more explicit; latest data date may be more accurate when imports
  return empty ranges.

## State Model

Avoid relying only on a global `last_successful_refresh_at_ms`.

Refresh policy needs source/projection-aware state.

Candidate state source:

- successful import runs, filtered by source, projection, timezone, and profile,
  using their declared `scope_kind`, `scope_start_date`, and `scope_end_date`.

If querying import runs becomes awkward, add a dedicated refresh cursor table
owned by the refresh/reconciliation boundary.

Cursor shape, if needed:

```text
source_key
projection
aggregation_timezone nullable
collector_key
collector_version/profile_version compatibility identity
last_successful_scope_start_date nullable
last_successful_scope_end_date nullable
last_successful_import_at_ms
```

Prefer deriving from existing run data first unless it creates unclear or slow
queries.

## Refresh Interval

Do not use a fixed two-minute full refresh.

After incremental refresh exists, a more aggressive interval can be considered,
but the default should still avoid unnecessary process churn.

Recommended starting point:

```text
scheduled fallback: 15 minutes
tray-open: today-only refresh if stale after 1-2 minutes
resume/startup: catch-up refresh if stale
future active-coding signal: 5 minutes while activity is recent
failure path: exponential backoff
```

## Failure Behavior

If incremental refresh fails:

- keep existing data,
- record the failed run,
- do not advance the cursor,
- retry later with the same or wider scope.

If an incremental scope repeatedly fails due to collector incompatibility:

- surface a stable error,
- allow explicit full resync,
- avoid silently deleting existing canonical data.

## Engineering Notes

The implementation plan is tracked in
`docs/planning/_WIP/refresh-policy-implementation-plan.md`.

Recommended implementation shape:

1. Add a refresh policy planner in the application layer.
2. Query source/projection import state from the run store.
3. Return either `Full`, catch-up `Incremental(start, end)`, or freshness
   `Incremental(today, today)` per target.
4. Update `RefreshCoordinator::collection_request` to accept target-specific
   scope.
5. Keep reconciliation behavior unchanged; it already handles idempotent
   upserts and absence lifecycle by scope.
6. Add tests for first install, catch-up after a week, lookback overlap,
   tray-open today-only refresh, and per-source/projection fallback to full.

## Risks

- A too-narrow incremental range can miss corrected historical data.
- A too-wide range can preserve the performance problem.
- Session data may not map cleanly to daily ranges for every collector.
- Profile/version compatibility needs a clear rule before old data is skipped or
  rebuilt.

## Follow-Up Decisions

- Whether to derive cursor state from import runs or store a dedicated cursor.
- How to handle collector/profile compatibility changes.
- Whether session imports should use the same date window as daily imports.

## Current Recommendation

Implement two source/projection-aware incremental modes:

- catch-up refresh with a two-day lookback,
- tray freshness refresh for today only.

Keep full refresh for:

- missing baseline,
- explicit future resync,
- compatibility rebuilds.

Keep the scheduled fallback at 15 minutes until incremental refresh is proven
cheap and reliable.
