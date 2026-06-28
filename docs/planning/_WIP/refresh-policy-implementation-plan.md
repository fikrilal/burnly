# Refresh Policy Implementation Plan

## Status

High-level implementation plan.

The accepted product policy is `docs/product/refresh-policy.md`.

## Recommendation

Implement this as several execution plans, not one large change.

The work crosses refresh policy planning, import-run state queries, coordinator
collection scopes, tray-open freshness behavior, and verification. Splitting it
keeps each change reviewable and gives us clean rollback points if a policy
assumption is wrong.

## Chunk 1: Policy Planner And Import State

Goal: create the application-layer policy decision point without changing user
visible refresh behavior yet.

Scope:

- Add a refresh policy planner in the application layer.
- Derive source/projection import state from existing successful import runs.
- Return planned scope per refresh target:
  - full when no successful baseline exists,
  - catch-up incremental with a two-day lookback when a baseline exists,
  - today-only freshness for tray-open policy calls.
- Add focused unit tests for first install, catch-up after a gap, two-day
  lookback, and today-only freshness.

Out of scope:

- changing scheduler interval,
- changing tray-open refresh behavior,
- adding manual full resync UI.

## Chunk 2: Coordinator Incremental Catch-Up

Goal: use the planner for automatic catch-up refresh paths.

Scope:

- Wire scheduled, startup-after-gap, and resume refresh through the policy
  planner.
- Update coordinator collection requests to use target-specific scopes.
- Preserve full refresh for missing baseline.
- Verify reconciliation still treats scoped absence correctly.
- Add integration tests around generated collector scopes and import-run
  persistence.

Out of scope:

- tray-open freshness optimization,
- manual resync UI,
- collector adapter rewrites.

## Chunk 3: Tray Freshness Refresh

Goal: make tray-open stale refresh fast and aligned with the primary product
job: "how many tokens did I burn today?"

Scope:

- Route tray-open stale refresh through today-only freshness policy when a
  baseline exists.
- Keep baseline/full behavior when no prior import exists.
- Recalculate week and month summaries from SQLite after today's row updates.
- Tune stale/throttle constants only if tests or runtime evidence show the
  current values are wrong.
- Add tests for today-only tray-open scope and baseline fallback.

Out of scope:

- active-coding file watching,
- changing scheduled fallback interval,
- user-configurable refresh settings.

## Chunk 4: Manual Refresh Semantics

Goal: align manual refresh with the accepted catch-up policy.

Scope:

- Route the existing manual refresh action through incremental catch-up when a
  baseline exists.
- Preserve full baseline behavior when no prior import exists.
- Keep explicit full resync as a later, separate product affordance unless the
  UI is ready to expose it clearly.
- Add tests for manual catch-up scope and baseline fallback.

Out of scope:

- broad settings redesign,
- diagnostics/support workflows.

## Chunk 5: Compatibility And Repair

Goal: handle cases where existing stored data cannot safely be incrementally
updated.

Scope:

- Define compatibility identity for source, projection, timezone, and
  collector/profile version.
- Fall back to full or targeted rebuild when compatibility changes require it.
- Keep failures observable and avoid silently deleting canonical data.

Out of scope:

- background storage migrations unless the chosen compatibility model requires
  them.

## Verification Expectations

Each execution plan should record commands and outcomes in the active plan.

Minimum gates for code chunks:

- `pnpm lint`
- `pnpm verify:fast`

Run the full gate before merging the completed policy series:

- `pnpm verify`
- `pnpm verify:runtime`
- `pnpm architecture:check`

Runtime evidence is required only if tray-visible behavior or desktop runtime
integration changes:

- `pnpm evidence:desktop`
