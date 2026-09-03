# 2026-08-30 Tray Summary Refresh And Data-Quality Status Separation

## Objective

Stop the tray header from reporting "Some sources failed" when collection
succeeded but today's usage contains inferred or otherwise partial attribution.
Represent refresh outcome and usage quality as separate tray-summary facts, then
derive accurate user-facing status copy with explicit precedence.

## Problem Evidence

Implementation context and the confirmed runtime investigation are recorded in
[`docs/planning/_WIP/tray-summary-status-separation-handoff.md`](../../planning/_WIP/tray-summary-status-separation-handoff.md).

- On 2026-08-30, diagnostics health was `ok`, refresh `3481` succeeded, every
  enabled source's latest import succeeded, zero records were rejected, and
  usage-integrity totals matched.
- The active Antigravity daily row was nevertheless `partial` because its
  records had `first_seen` timestamp origin rather than source-reported activity
  timestamps.
- `SqliteTraySummaryStore` currently reads both `has_partial_data` and the latest
  terminal refresh status, but `TraySummaryQuery` collapses them into one
  `OverviewDataStatus::Partial` value.
- The frontend maps every `partial` value to "Some sources failed", so a data
  attribution limitation is presented as a collector failure.

## Acceptance Criteria

- A successful refresh with complete usage displays the normal relative update
  time.
- A successful refresh with partial usage displays "Some usage is estimated".
- A partial refresh displays "Some sources failed", whether the retained usage
  rows are complete or partial.
- A failed refresh or tray-summary query failure displays "Refresh failed".
- An active refresh displays "Refreshing" without discarding the last
  authoritative summary.
- Empty usage remains an empty overview state and does not hide a failed or
  partial latest refresh in the header.
- Refresh outcome, usage quality, and data availability cross IPC as separate,
  validated fields.
- Existing databases obtain the corrected behavior immediately without a
  schema migration, data rewrite, or collector re-run.
- Diagnostics health remains independent from the overview header and is not
  queried to infer refresh or usage quality.
- Generated TypeScript, runtime validation, Rust mapping, frontend fixtures,
  and behavior tests agree on the new contract.

## Non-Goals

- Changing Antigravity timestamp extraction, cache reconciliation, or its
  `partial` data-quality classification.
- Changing OpenCode cumulative-recovery behavior or any other collector.
- Changing refresh coordination, scheduling, persistence, diagnostics health,
  or diagnostic retention.
- Adding a database migration or altering canonical usage rows.
- Adding source-level warning details or a tooltip in this chunk. The header
  copy is sufficient to correct the false failure claim.
- Redesigning status presentation outside the tray summary and shared status
  primitive needed by the tray.

## Risk Class

`medium` — this changes a generated IPC response and user-visible status
precedence across Rust and React. It does not mutate persisted data, collector
behavior, or release infrastructure.

## Impact Areas

- Tray-summary application read model and status derivation
- SQLite tray-summary adapter tests
- Usage IPC response mapping and generated TypeScript contract
- Frontend Zod validation and tray-summary fixtures
- Tray header status derivation and copy
- Shared freshness status vocabulary/styleguide, if the tray continues to use
  that primitive
- Focused Rust, contract, frontend, and desktop bridge verification

## Contract Design

The tray summary must expose three independent dimensions:

```ts
interface TraySummaryResponse {
  // Existing metrics, models, and timestamps remain unchanged.
  dataStatus: "current" | "stale" | "empty";
  dataQuality: "complete" | "partial";
  latestRefreshStatus: "succeeded" | "partial" | "failed" | "cancelled" | null;
}
```

`dataStatus` answers whether the summary has current, stale, or no usage data.
It must no longer encode collection failure or attribution quality.

`dataQuality` answers whether every active daily row for the reporting day is
complete. The existing `read_has_partial_today` query remains the source of
truth; `has_partial_data = true` maps to `partial`, otherwise `complete`.

`latestRefreshStatus` is the latest persisted terminal status already read from
`refresh_runs`. It is transported without collapsing `cancelled`, `failed`, or
`partial` into the data-status dimension.

No field is inferred from diagnostics events. No new storage interface is
needed because `TraySummaryStoreResult` already contains the required facts.

## Header Precedence

Implement one pure tray-owned derivation function and cover its complete
decision table. Highest precedence wins:

1. Tray-summary query error: `failed` / "Refresh failed".
2. Active refresh event or query fetch: `refreshing` / "Refreshing".
3. Latest persisted refresh `failed` or `cancelled`: `failed` / "Refresh
   failed". Preserve existing cancelled behavior in this focused fix.
4. Latest persisted refresh `partial`: `partial` / "Some sources failed".
5. Usage `dataQuality === "partial"`: `estimated` / "Some usage is
   estimated".
6. Otherwise use `dataStatus` and the existing relative last-successful-refresh
   timestamp presentation.

If both the latest refresh and usage quality are partial, the refresh outcome
wins because it identifies an actual source collection problem. The estimated
usage remains represented in the response for future detail surfaces.

## Design Review

- Complexity introduced: two small status enums and one pure presentation
  decision function replace one overloaded enum and scattered interpretation.
- Decisions hidden: the application query owns semantic facts, IPC owns their
  wire encoding, and the tray owns copy/precedence. SQLite continues to own only
  persisted reads.
- Interface depth: callers receive explicit status dimensions without knowing
  SQL rules, refresh tables, or canonical data-quality representation.
- Special cases: simultaneous partial refresh and partial usage are resolved by
  one documented precedence rule instead of component conditionals.
- Abstractions needed now: explicit status types are necessary because one
  value cannot truthfully represent both collection outcome and data quality.
- Existing ownership: `TraySummaryReadModel`, `TraySummaryResponse`, and
  `tray-utils` can absorb this work; no generic status service or new repository
  port is justified.

## Implementation Sequence

### 1. Separate application read-model facts

- Replace the overloaded `OverviewDataStatus` use in the tray read model with:
  - availability/freshness status containing only `Current`, reserved `Stale`,
    and `Empty`;
  - a usage-quality enum containing `Complete` and `Partial`;
  - the existing optional `PersistedRefreshStatus` as an explicit read-model
    field.
- Refactor `read_model` so empty/current derivation depends only on token/model
  availability. Map `has_partial_data` independently to usage quality and copy
  the latest refresh status independently.
- Remove the current precedence where `has_partial_data` can mask a failed
  refresh.
- Update application tests to prove all combinations independently, including:
  - empty + failed refresh;
  - populated + succeeded + complete;
  - populated + succeeded + partial quality;
  - populated + partial refresh + complete quality;
  - populated + partial refresh + partial quality;
  - populated + cancelled refresh.

### 2. Preserve storage responsibility

- Keep `read_has_partial_today` and `read_refresh_history` as separate queries.
- Do not alter their SQL or the database schema unless a focused test exposes a
  real defect.
- Update `SqliteTraySummaryStore` tests so one fixture proves both facts survive
  the adapter independently: a partial daily row and a partial latest refresh
  must be separately observable in `TraySummaryStoreResult`.
- Add or refine a fixture where the latest refresh succeeded but today's row is
  partial, matching the observed Antigravity case.

### 3. Extend the IPC contract

- Add `dataQuality` and `latestRefreshStatus` to `TraySummaryResponse`.
- Narrow tray `dataStatus` to `current | stale | empty`; do not change the
  similarly named overview or activity-calendar contracts.
- Add explicit Rust-to-wire mapping functions for data quality and persisted
  refresh status. `None` must serialize as `null`.
- Update the contract generator source and run `pnpm contracts:generate`; do
  not hand-edit generated output as the final source of truth.
- Update frontend Zod validation and IPC-client fixtures to require and validate
  the new fields and reject unknown enum values.
- Update the real Tauri bridge/bootstrap assertion so an empty database and a
  seeded partial-quality case prove the serialized dimensions.

### 4. Derive truthful tray presentation

- Replace the current `freshnessState(dataStatus, isRefreshing, isError)` helper
  with a tray status derivation that accepts all relevant facts:
  `dataStatus`, `dataQuality`, `latestRefreshStatus`, active-refresh state, and
  query-error state.
- Add an `estimated` presentation state with copy "Some usage is estimated".
- Keep `partial` reserved for a partial refresh and copy "Some sources failed".
- Keep `failed`, `refreshing`, empty/current/stale, and relative-update behavior
  consistent with the precedence above.
- Update `OverviewTab` to continue deriving only empty-content presentation from
  `dataStatus`; it must not use refresh status or usage quality to hide metrics.
- Update shared status examples/tests only as required to represent the new
  `estimated` state consistently. Avoid unrelated visual changes.

### 5. Prove regression behavior

- Add a pure frontend decision-table test at the helper layer for every
  precedence branch. This is the lowest stable layer for status policy.
- Add tray component tests that assert visible copy for:
  - successful + complete;
  - successful + partial quality;
  - partial refresh;
  - failed refresh;
  - active refresh;
  - simultaneous partial refresh and partial quality.
- Assert mutually exclusive copy so "Some sources failed" cannot appear in the
  successful partial-quality case.
- Keep authoritative previous summary data visible during background refresh or
  query failure, matching existing TanStack Query behavior.
- Run contract drift, architecture, focused Rust/frontend, fast, full, and
  desktop-runtime gates. Record exact outcomes below before moving this plan to
  `completed/`.

## Checklist

- [x] Separate availability, usage quality, and latest refresh outcome in the
      application read model.
- [x] Update application status-combination tests.
- [x] Preserve existing SQLite queries and add independent-fact adapter tests.
- [x] Extend Rust IPC response mapping with `dataQuality` and
      `latestRefreshStatus`.
- [x] Narrow only the tray-summary `dataStatus` contract.
- [x] Regenerate TypeScript contracts and update Zod/client validation tests.
- [x] Implement one pure tray header precedence function.
- [x] Add the `estimated` presentation state and accurate copy.
- [x] Update tray fixtures, component tests, shared status tests, and styleguide
      examples as needed.
- [x] Add desktop bridge evidence for separate refresh and quality fields.
- [x] Run focused verification and `pnpm contracts:check`.
- [x] Run `pnpm architecture:check` and `pnpm verify:fast`.
- [x] Run `pnpm verify` and `pnpm verify:runtime`.
- [x] Record outcomes, move this plan to `completed/`, and leave the worktree
      ready for review without committing unless explicitly requested.

## Test Plan

- Behavior and invariants to prove:
  - partial attribution never claims a source failed after a successful refresh;
  - partial and failed refreshes retain their failure-oriented copy;
  - simultaneous conditions follow the documented precedence;
  - empty/current data availability remains independent;
  - previous metrics remain visible during active refresh and background error;
  - IPC enum values and nullability remain exact across Rust, generated types,
    and Zod.
- Lowest stable test layer:
  - pure Rust read-model derivation for semantic facts;
  - real temporary SQLite for adapter facts;
  - Rust IPC serialization and generated-contract drift checks;
  - pure TypeScript helper tests for presentation precedence;
  - React component tests for visible header copy.
- Failure paths:
  - failed and cancelled latest refresh;
  - partial refresh with complete or partial retained usage;
  - tray-summary query failure with prior data;
  - malformed or unknown IPC status values;
  - no prior refresh and empty usage.
- Fixtures or fakes:
  - real temporary SQLite for persistence behavior;
  - existing application store fake for read-model combinations;
  - typed IPC invoker and event fakes at the frontend boundary;
  - no SQLite mocks and no collector mocks.
- Runtime or platform evidence:
  - desktop bridge test against temporary persisted data;
  - no production database mutation and no live collector invocation required.
- Relevant commands:
  - `cargo test --manifest-path src-tauri/Cargo.toml application::usage::tray_summary`
  - `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::database::tray_summary_store`
  - `cargo test --manifest-path src-tauri/Cargo.toml ipc::usage`
  - `pnpm test src/features/tray src/components/burnly/status.test.tsx src/ipc/client.test.ts`
  - `pnpm contracts:generate`
  - `pnpm contracts:check`
  - `pnpm architecture:check`
  - `pnpm verify:fast`
  - `pnpm verify`
  - `pnpm verify:runtime`

## Decisions

- Fix the model rather than replacing one misleading string with another. A
  copy-only patch cannot distinguish a real partial refresh from partial usage.
- Keep refresh outcome, usage quality, and availability separate through the
  application and IPC boundaries; combine them only at presentation.
- Preserve `cancelled` as an explicit wire value but retain the existing
  failure-oriented header behavior in this chunk.
- Give partial refresh precedence over estimated usage when both are true.
- Keep diagnostics health out of tray-header derivation because it has a
  different retention window and product meaning.
- Reuse existing persisted facts; do not add migration or compatibility state.
- Limit contract narrowing to `TraySummaryResponse`; other usage surfaces keep
  their existing semantics until separately reviewed.
- Review finding (P2): the tray response change is breaking — `dataStatus`
  changed meaning and lost variants while required fields were added — so the
  IPC major contract version is bumped from 1 to 2, keeping the bootstrap
  compatibility guard truthful for mixed old/new runtimes and frontends.

## Verification

- Command: focused Rust and frontend tests
- Outcome: passed — `tray_summary` (5), `tray_summary_store` (4), `ipc::usage`, and the Tauri bridge evidence all pass; `pnpm test` focused suites (55 tests) pass after restoring `NODE_ENV=development` in `vitest.config.ts` (React 19.2.7's production export omits `act`, which broke every component test at HEAD).
- Command: `pnpm contracts:generate`
- Outcome: passed — regenerated `src/ipc/generated/contracts.ts` with `dataQuality`, `latestRefreshStatus`, and `CONTRACT_VERSION = 2`.
- Command: `pnpm contracts:check`
- Outcome: passed — registry, generated bindings, the `= 2` harness assertion, and the v2 IPC fixtures agree.
- Command: `pnpm architecture:check`
- Outcome: passed — no boundary violations.
- Command: `pnpm verify:fast`
- Outcome: passed (exit 0).
- Command: `pnpm verify`
- Outcome: passed (exit 0) after `cargo fmt`.
- Command: `pnpm verify:runtime`
- Outcome: passed (exit 0) — "Desktop runtime evidence passed."
- Command: contract version bump (v1 → v2)
- Outcome: `response.rs` const, `contract.rs` registry test, harness regex + template const + fixture checks, IPC fixtures, `application/bootstrap.rs` service tests, `test_support` meta, and the App mismatch test (runtime 3 vs frontend 2) all updated and passing; the cloud collect-sync `COLLECT_CONTRACT_VERSION` is a separate server contract and was left unchanged.

## Runtime Evidence

- Required at the desktop bridge with temporary persisted data because the IPC
  response shape changes.
- Live production-database mutation is explicitly unnecessary and out of scope.

## Rollback And Stop Conditions

- Stop if separating the fields requires a database migration; the necessary
  facts already exist and a migration would indicate scope drift.
- Stop if contract generation changes `UsageOverviewResponse` or
  `ActivityCalendarResponse`; only the tray-summary contract is in scope.
- Stop if a proposed frontend shortcut derives collection failure from
  diagnostics health or usage quality.
- A safe rollback restores the previous read-model/IPC/frontend mapping as one
  unit. Do not leave generated contracts out of sync with Rust or Zod.

## Follow-Up Debt

- A source-level explanation or tooltip for partial attribution may be added in
  a separate UX chunk if users need to know which source used inferred data.
- The reserved `stale` status can receive an age policy separately; this fix
  does not invent one.
