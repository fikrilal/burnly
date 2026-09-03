# 2026-09-03 Antigravity Baseline Attribution 02 Cache Store Attribution Logic

## Objective

Update the Antigravity usage cache store (`SqliteAntigravityUsageCacheStore`) to
evaluate baseline progress when resolving record timestamps, assigning
`UndatedBaseline` during pending baseline scans and `Dated` for post-baseline
records.

Critically, handle both `Unresolved` and `LegacyUnknown` origins: any record
lacking a genuine source-reported timestamp during a pending baseline (including
App/IDE records currently carrying `LegacyUnknown`) must receive
`UndatedBaseline` to prevent inflation. Additionally, restrict `read_for_scope`
so that only `Dated` records can be read for calendar aggregation.

## Scope

- `src-tauri/src/application/ports/antigravity_usage_cache.rs`
- `src-tauri/src/infrastructure/database/antigravity_cache_store.rs`
- Unit tests in `antigravity_cache_store.rs`

## Out Of Scope

- Modifying baseline state tables or running repair (Chunks 01 and 03).
- Removing App/IDE `LegacyUnknown` conversion from the collector client (Chunk 04).
- Canonical daily usage reconciliation (Chunk 05).

## Risk Class

`medium` (alters core timestamp resolution and query filtering for Antigravity cache).

## Impact Areas

- Antigravity cache reconciliation and scope query SQL

## Design Review

### Timestamp & Attribution Resolution

Update `resolve_timestamp` in `antigravity_cache_store.rs`:

```text
Inputs:
- record: &CachedAntigravityUsageRecord
- existing: Option<&ExistingRecord>
- baseline_status: AntigravityBaselineStatus (Pending or Complete)
- collected_at: DateTime<Utc>

Resolution for new records (existing is None):
1. SourceReported timestamp present:
   -> observed_at = source_time
   -> timestamp_origin = SourceReported
   -> calendar_attribution = Dated
2. Source timestamp missing (Unresolved OR LegacyUnknown):
   - If baseline_status == Pending:
     -> observed_at = record.observed_at.unwrap_or(collected_at)
     -> timestamp_origin = record.timestamp_origin (or FirstSeen if Unresolved)
     -> calendar_attribution = UndatedBaseline
   - If baseline_status == Complete:
     -> observed_at = record.observed_at.unwrap_or(collected_at)
     -> timestamp_origin = FirstSeen
     -> calendar_attribution = Dated

Resolution for existing records (existing is Some):
- Retain existing.calendar_attribution and existing.timestamp_origin.
- Upstream Timestamp Upgrade: If existing was UndatedBaseline or LegacyUnknown,
  and the incoming record now supplies a genuine SourceReported timestamp,
  upgrade to Dated with SourceReported origin.
```

By explicitly evaluating `LegacyUnknown` alongside `Unresolved`, App and IDE
records without source timestamps are attributed as `UndatedBaseline` during
baseline even if an upstream layer supplies `LegacyUnknown`.

### Scope Query Filtering

In `read_for_scope`:

```sql
SELECT variant, conversation_id, response_id, raw_model_id, model_label,
       api_provider, input_tokens, output_tokens, thinking_output_tokens,
       response_output_tokens, cache_read_tokens, cache_write_tokens,
       source_record_index, observed_at_ms, timestamp_origin, calendar_attribution
FROM antigravity_usage_cache
WHERE calendar_attribution = 'dated'
  AND observed_at_ms >= ?1 AND observed_at_ms < ?2
  AND ( ... )
ORDER BY observed_at_ms ASC, id ASC;
```

Undated baseline records are completely excluded from `read_for_scope`.

## Checklist

- [x] Add `calendar_attribution` field to `CachedAntigravityUsageRecord`.
- [x] Update `AntigravityUsageCacheUpsert` / `AntigravityUsageCache::reconcile` to accept `baseline_status` (or query baseline store within transaction).
- [x] Implement attribution resolution rules in `resolve_timestamp`, covering both `Unresolved` and `LegacyUnknown`.
- [x] Update SQL `INSERT ... ON CONFLICT(dedupe_key)` to persist `calendar_attribution`.
- [x] Update `read_for_scope` query to filter `calendar_attribution = 'dated'`.
- [x] Add unit tests verifying:
  - New `Unresolved` record during `Pending` baseline resolves to `UndatedBaseline`.
  - New `LegacyUnknown` record during `Pending` baseline resolves to `UndatedBaseline`.
  - New `SourceReported` record during `Pending` baseline resolves to `Dated`.
  - New `Unresolved` or `LegacyUnknown` record during `Complete` baseline resolves to `Dated`.
  - Existing `UndatedBaseline` record re-scanned preserves attribution.
  - Existing `UndatedBaseline` re-scanned with new source timestamp upgrades to `Dated`.
  - `read_for_scope` returns only `Dated` records.
- [x] Verify: `cargo test --manifest-path src-tauri/Cargo.toml antigravity_cache_store`.

## Test Plan

- **Invariants to Prove**:
  - Neither `Unresolved` nor `LegacyUnknown` records can become `Dated` during a `Pending` baseline scan.
  - `UndatedBaseline` rows never emerge from `read_for_scope`.
  - Baseline transitions deterministically govern `UndatedBaseline` vs `Dated`.
  - Re-scanning unchanged records is idempotent.
- **Commands**:
  - `cargo test --manifest-path src-tauri/Cargo.toml antigravity_cache_store`
  - `cargo test --manifest-path src-tauri/Cargo.toml collectors::antigravity`
  - `pnpm architecture:check`
  - `pnpm verify:fast`
  - `pnpm verify`
  - `pnpm verify:runtime`

## Decisions

- **Defensive handling of `LegacyUnknown` in cache store**: Handling
  `LegacyUnknown` in `resolve_timestamp` ensures correctness regardless of
  whether upstream collectors send `Unresolved` or `LegacyUnknown`.
- **In-transaction baseline status query**: Querying `antigravity_baseline_state`
  directly inside the SQLite transaction during `reconcile` guarantees atomic
  consistency without changing the `AntigravityUsageCache` trait signature.

## Verification

- Command: `cargo test --manifest-path src-tauri/Cargo.toml antigravity_cache_store`
  - Outcome: passed (8 passed, 0 failed; all existing tests + comprehensive `baseline_attribution_logic_governs_dated_vs_undated_baseline`).
- Command: `cargo test --manifest-path src-tauri/Cargo.toml collectors::antigravity`
  - Outcome: passed (80 passed, 0 failed).
- Command: `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
  - Outcome: passed (clean, 0 warnings).
- Command: `pnpm architecture:check`
  - Outcome: passed ("Architecture boundary check passed.").
- Command: `pnpm verify:fast`
  - Outcome: passed (harness checks, Prettier, ESLint, TypeScript, Clippy, jscpd clean).
- Command: `pnpm verify`
  - Outcome: passed (full local gate, 683 Rust tests passed, 117 Vitest tests passed).
- Command: `pnpm verify:runtime`
  - Outcome: passed ("Desktop runtime evidence passed.").

## Follow-Up Debt

- None.
