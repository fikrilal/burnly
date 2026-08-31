# Tray Summary Status Separation Handoff

## Handoff Status

**Diagnosed and planned; implementation has not started.**

- Repository: Burnly desktop
- Branch at handoff: `development`
- HEAD at handoff: `3f18cb6 docs(release): record v0.1.29 publication`
- Affected released version: `0.1.29`
- Implementation plan:
  [`docs/exec-plans/active/2026-08-30_tray-summary-status-separation.md`](../../exec-plans/active/2026-08-30_tray-summary-status-separation.md)
- This handoff and the execution plan are currently uncommitted. Preserve both
  unless the user explicitly asks to discard them.

## User-Visible Problem

The tray header displays:

> Some sources failed

This can happen while all of the following are true:

- diagnostics health is `ok` with no reasons;
- the latest refresh succeeded;
- every enabled source's latest import succeeded;
- every import rejected zero records;
- daily and model-usage integrity totals match;
- diagnostic events are informational only.

The message is therefore not stale, but it is false. The tray is presenting
partial usage attribution as a collector failure.

## Confirmed Runtime Evidence

### 2026-08-23 reproduction

The user's v0.1.29 diagnostic export showed six consecutive successful
refreshes after upgrading. Health was `ok`, all latest source imports succeeded,
and usage-integrity totals matched at `243,803,741` tokens.

The production SQLite database contained these active daily rows for
`2026-08-23` in `Asia/Jakarta`:

| Source      | Tokens      | Data quality |
| ----------- | ----------- | ------------ |
| Antigravity | 198,908,795 | partial      |
| Codex       | 43,714,255  | complete     |
| OpenCode    | 1,180,691   | partial      |

Why they were partial:

- Antigravity had usage records without source-reported activity timestamps.
  Burnly assigned stable `first_seen` timestamps.
- OpenCode contained one cumulative-recovery ledger record worth 783 tokens;
  the other 1,179,908 tokens were exact v2 message records.

No source had failed. The tray nevertheless rendered "Some sources failed".

### 2026-08-30 Antigravity reproduction

Before current-day Antigravity usage appeared, the diagnostic export was
healthy and the tray did not have a partial current-day Antigravity row.

After Antigravity became active:

- health remained `ok`;
- refreshes `3472` through `3481` all succeeded;
- Antigravity daily and session imports succeeded with zero rejected records;
- usage integrity matched at `38,010,296` tokens;
- the active Antigravity daily row contained `427,001` tokens and was marked
  `partial`;
- all Antigravity activity timestamps were `first_seen`; none were
  source-reported;
- the tray immediately displayed "Some sources failed" again.

The underlying two current-day Antigravity responses were first observed at
11:53:47 and 11:59:19 local time and totalled 211,137 and 215,864 tokens. Token
extraction and deduplication worked. Only time attribution was inferred.

The process/runtime counters were also expected:

- one `agy -c` process had been active since 11:31 local time;
- a second `agy -c` process started at 13:52:48;
- manual refresh `3481` started four seconds later and discovered two process
  candidates, four endpoints, and two successful identity probes;
- the manual refresh covered only `2026-08-30`, so it emitted one daily and one
  session candidate;
- its 761 extracted rows included conversation history, while only two cached
  records belonged to the current reporting day.

This Antigravity activity did not cause a refresh failure. It merely created a
current-day partial-quality row that exposed the status-model bug.

## Root Cause

The tray-summary pipeline already reads two independent facts from SQLite, but
the application layer collapses them into one enum.

### 1. Storage reads independent facts correctly

File:
`src-tauri/src/infrastructure/database/tray_summary_store.rs`

`read_tray_summary` reads:

- `has_partial_data` from current-day canonical usage rows;
- `latest_refresh_status` from the latest terminal `refresh_runs` row;
- `last_successful_refresh_at_ms` separately.

`read_has_partial_today` returns true when any active daily row satisfies:

```sql
data_quality <> 'complete' OR record_state = 'missing'
```

`read_refresh_history` independently returns one of `succeeded`, `partial`,
`failed`, or `cancelled`.

There is no storage defect and no missing database field.

### 2. Application collapses the facts

File: `src-tauri/src/application/usage/tray_summary.rs`

`TraySummaryStoreResult` contains both facts, but `TraySummaryReadModel` exposes
only `data_status: OverviewDataStatus`.

The current `data_status` function does this:

```rust
if has_partial_data || latest_refresh_status == Some(Partial) {
    return OverviewDataStatus::Partial;
}
```

It also maps failed/cancelled refreshes into `OverviewDataStatus::Failed`.
Consequently, `OverviewDataStatus` simultaneously means:

- usage availability/freshness;
- usage quality;
- latest refresh outcome.

Those are different product facts and cannot share truthful presentation copy.

One confirmed bad precedence also exists: partial usage currently takes
precedence over a failed refresh, so a failed refresh with partial retained data
can be downgraded to the less severe `partial` state.

### 3. IPC preserves only the collapsed value

Files:

- `src-tauri/src/ipc/usage.rs`
- `scripts/harness/check-contracts.mjs`
- `src/ipc/generated/contracts.ts`
- `src/ipc/client.ts`

`TraySummaryResponse` currently exposes only:

```ts
dataStatus: "current" | "stale" | "partial" | "failed" | "empty";
```

The frontend cannot recover the original refresh and quality facts because the
backend has already discarded their distinction.

### 4. Frontend maps every partial state to failure copy

Files:

- `src/features/tray/tray-utils.ts`
- `src/features/tray/TrayPanel.tsx`
- `src/components/burnly/status.tsx`

`freshnessState` receives the collapsed `dataStatus` plus only an
`isRefreshing` boolean and a query-error boolean. It does not receive the
latest terminal refresh status.

`HeaderStatus` then maps all `partial` values to:

```text
Some sources failed
```

This is the final point where the false statement becomes visible, but changing
only this string would create the opposite bug: a real partial refresh would be
described as estimated usage.

## Required Design

Expose three independent tray-summary fields:

```ts
interface TraySummaryResponse {
  dataStatus: "current" | "stale" | "empty";
  dataQuality: "complete" | "partial";
  latestRefreshStatus: "succeeded" | "partial" | "failed" | "cancelled" | null;
}
```

Semantics:

- `dataStatus`: whether summary data is current, stale, or empty;
- `dataQuality`: whether every active current-day usage row is complete;
- `latestRefreshStatus`: the latest persisted terminal refresh result.

All three facts already exist. No migration, data rewrite, collector rerun, or
new persistence port is needed.

## Accepted Header Precedence

Use one pure tray-owned decision function. Highest precedence wins:

1. Tray-summary query error -> "Refresh failed".
2. Active refresh -> "Refreshing".
3. Latest refresh failed or cancelled -> "Refresh failed".
4. Latest refresh partial -> "Some sources failed".
5. Data quality partial -> "Some usage is estimated".
6. Otherwise -> existing relative successful-refresh time.

When refresh status and data quality are both partial, the refresh outcome wins
because a source actually failed during collection.

Empty content remains controlled by `dataStatus === "empty"`; it must not hide
a failed or partial latest refresh in the header.

## Important Semantic Edge

The existing `has_partial_data` query covers both:

- `data_quality <> 'complete'`, such as Antigravity inferred timestamps or
  OpenCode cumulative recovery;
- `record_state = 'missing'`.

The accepted copy for this focused chunk is "Some usage is estimated". That is
accurate for the observed Antigravity/OpenCode cases but less precise for a
missing canonical row. Do not expand this implementation into source/reason
details. If product needs perfectly reason-specific copy, add a future contract
that transports quality reasons rather than inferring them in React.

## Exact Implementation Map

### Application

File: `src-tauri/src/application/usage/tray_summary.rs`

- Narrow tray data availability to current/stale/empty.
- Add complete/partial usage quality.
- Expose `Option<PersistedRefreshStatus>` in `TraySummaryReadModel`.
- Derive each field independently in `read_model`.
- Replace current combined-precedence tests with independent combination tests.

### Persistence

File: `src-tauri/src/infrastructure/database/tray_summary_store.rs`

- Preserve `read_has_partial_today` and `read_refresh_history` as separate
  queries.
- Do not change schema or SQL without evidence of another defect.
- Add a real-SQLite fixture for successful latest refresh plus partial current
  usage, matching Antigravity.

### IPC and contracts

Files:

- `src-tauri/src/ipc/usage.rs`
- `scripts/harness/check-contracts.mjs`
- `src/ipc/generated/contracts.ts`
- `src/ipc/client.ts`
- `src/ipc/client.test.ts`

- Add `dataQuality` and nullable `latestRefreshStatus`.
- Narrow only `TraySummaryResponse.dataStatus`.
- Do not change `UsageOverviewResponse` or `ActivityCalendarResponse`; their
  similarly named fields are outside this task.
- Update Rust wire mapping and Zod validation.
- Edit the contract-generator source, run `pnpm contracts:generate`, and keep
  generated output aligned. Do not treat a manual generated-file edit as the
  source of truth.

### Frontend presentation

Files:

- `src/features/tray/tray-utils.ts`
- new or existing tray-utils tests
- `src/features/tray/TrayPanel.tsx`
- `src/features/tray/OverviewTab.tsx`
- `src/features/tray/test_support.tsx`
- `src/features/tray/TrayPanel.overview.test.tsx`
- `src/components/burnly/status.tsx`
- `src/components/burnly/status.test.tsx`
- `src/features/styleguide/StyleguideView.tsx`, if the shared primitive gains
  an `estimated` example

- Implement the precedence as one pure function.
- Reserve `partial` presentation for partial refresh.
- Add `estimated` presentation for partial usage quality.
- Keep previous summary data visible during background refresh/error.
- Continue using only `dataStatus` to decide whether empty content is shown.

### Desktop bridge

File: `src-tauri/src/bootstrap.rs` and its existing tests.

- Extend the real command-bridge evidence to assert separate serialized fields.
- Use temporary persisted data. Do not mutate the user's production database or
  launch live collectors for this proof.

## Test Matrix

At minimum, prove these cases:

| Latest refresh | Data quality | Data status | Expected header          |
| -------------- | ------------ | ----------- | ------------------------ |
| succeeded      | complete     | current     | Updated ...              |
| succeeded      | partial      | current     | Some usage is estimated  |
| partial        | complete     | current     | Some sources failed      |
| partial        | partial      | current     | Some sources failed      |
| failed         | complete     | current     | Refresh failed           |
| failed         | partial      | current     | Refresh failed           |
| cancelled      | any          | any         | Refresh failed           |
| any            | any          | any         | Refreshing, while active |
| null           | complete     | empty       | Never updated / empty UI |

Also assert that successful + partial quality does not render "Some sources
failed" anywhere in the tray header.

## Verification Commands

Run focused checks first, then repository gates:

```sh
cargo test --manifest-path src-tauri/Cargo.toml application::usage::tray_summary
cargo test --manifest-path src-tauri/Cargo.toml infrastructure::database::tray_summary_store
cargo test --manifest-path src-tauri/Cargo.toml ipc::usage
pnpm test src/features/tray src/components/burnly/status.test.tsx src/ipc/client.test.ts
pnpm contracts:generate
pnpm contracts:check
pnpm architecture:check
pnpm verify:fast
pnpm verify
pnpm verify:runtime
```

Record command outcomes in the active execution plan. Move it to `completed/`
only after all required checks pass.

## Guardrails And Stop Conditions

- Do not implement a copy-only patch.
- Do not derive tray status from diagnostics health.
- Do not change collector behavior or partial-quality rules.
- Do not add a database migration; all required data already exists.
- Do not alter overview/activity-calendar contracts accidentally.
- Do not mutate or sanitize the user's production SQLite database to make the
  message disappear.
- Do not commit or push unless the user explicitly requests it.
- Stop and reassess if implementation requires a new storage abstraction or
  changes canonical reconciliation; that indicates scope drift.

## Expected Outcome

With the user's current Antigravity data, the tray should continue showing all
`38,010,296` tokens and Antigravity's `427,001`-token model row. The header must
change from:

> Some sources failed

to:

> Some usage is estimated

Diagnostics health remains `ok`, refresh/import history remains unchanged, and
no persisted usage is rewritten.
