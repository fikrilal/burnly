# 2026-09-03 Antigravity Baseline Attribution 03 Profile-2 Historical Repair Service

## Objective

Implement the crash-safe historical repair service (`AntigravityBaselineRepairService`)
to deterministically reclassify the pre-existing bootstrap cohort on Burnly
`0.1.29` installations from `first_seen` / `legacy_unknown` to
`undated_baseline`.

The service enforces strict eligibility: because cache reconciliation commits
batches of 20 conversations before outer import completion, any prior profile-2
attempt (even with `records_seen = 0` or failed/cancelled status) could have
written cache rows. The service conservatively rejects any installation with
earlier profile-2 attempts, ensuring repair executes only when the initial run is
provably the first cache write.

## Scope

- `src-tauri/src/infrastructure/database/antigravity_baseline_repair.rs`
- Unit and integration tests in `antigravity_baseline_repair.rs`
- Diagnostics event integration (`antigravity.baseline_repair_applied`,
  `antigravity.baseline_repair_skipped`)

## Out Of Scope

- Canonical daily fact replacement (Chunk 05).
- Collect-sync outbox scheduling (Chunk 05).
- Collector adapter profile bump (Chunk 04).

## Risk Class

`high` (reclassifies historical cache data on upgraded installations).

## Impact Areas

- Historical Antigravity usage cache records
- Diagnostic events and audit ledger

## Design Review

### Strict Eligibility Proof (First Cache Write Invariant)

In `AntigravityUsageCacheClient::reconcile_usage`, batches of 20 conversations
(`CACHE_RECONCILIATION_CONVERSATION_BATCH_SIZE`) commit to
`antigravity_usage_cache` iteratively. If a refresh is cancelled or crashes
mid-run, `import_runs` is marked failed/cancelled or left incomplete, often with
`records_seen = 0`. Those committed rows retain `first_seen_at_ms` from the
failed run. A later successful run would not re-insert them, meaning a time
window based on the later run would fail to match or misclassify history.

Therefore, automatic repair requires:

1. **Initial Full Daily Import**: An `import_runs` row for `antigravity` and
   `profile_version = 2` where `projection = 'daily'`, `scope_kind = 'full'`,
   and `status = 'succeeded'` ($I_{\text{daily}}$).
2. **Correlated Full Session Import**: In the same job (`refresh*run_id =
   I*{\text{daily}}.\text{refresh*run_id}$), a matching successful full session
   import ($I*{\text{session}}$).
3. **Absolute Absence of Prior Profile-2 Attempts**:
   ```sql
   SELECT 1 FROM import_runs
   WHERE source_id = ?1
     AND profile_version = 2
     AND id < ?2
   LIMIT 1;
   ```
   If ANY prior profile-2 run exists—regardless of `status` or `records_seen`—
   eligibility fails. We cannot prove $I_{\text{daily}}$ was the first cache
   write.
4. **Exact Strict Timestamp Bounds**:
   ```text
   window_start_ms = min(I_daily.started_at_ms, I_session.started_at_ms)
   window_end_ms   = max(I_daily.finished_at_ms, I_session.finished_at_ms)
   ```
   No arbitrary $\pm 5$-second expansion.
5. **Fail-Safe Skip**: If any condition fails, record `stage = 'skipped'` with
   the exact reason code (`prior_profile2_runs_exist`,
   `missing_matching_session_run`, `no_profile2_full_run`) and emit
   `antigravity.baseline_repair_skipped`. Do not guess.

### Reclassification Scope

Update both `first_seen` (CLI) and `legacy_unknown` (App/IDE) records created
during the proven window:

```sql
UPDATE antigravity_usage_cache
SET calendar_attribution = 'undated_baseline'
WHERE timestamp_origin IN ('first_seen', 'legacy_unknown')
  AND first_seen_at_ms >= ?1 AND first_seen_at_ms <= ?2;
```

Records with `source_reported` timestamps remain untouched.

### Runtime Owner and Stage Execution

- **Runtime Owner**: `AntigravityCollector::collect` invokes
  `AntigravityBaselineRepairService::ensure_cache_reclassified` at the beginning
  of collection before scanning artifacts or querying the cache.
- **Stage Progression**:
  1. If `antigravity_baseline_repair_state` has `stage IN ('complete', 'skipped')`,
     return immediately (no-op).
  2. If `stage == 'not_started'`:
     - Evaluate eligibility against `import_runs`.
     - If ineligible: transition to `stage = 'skipped'` with audit reason and
       emit diagnostic event.
     - If eligible: execute reclassification within an atomic SQLite transaction,
       record `records_reclassified`, and transition to
       `stage = 'cache_reclassified'`.
  3. If already `stage == 'cache_reclassified'` or higher (e.g. from an earlier
     attempt where collection or refresh later failed): return immediately
     without re-executing SQL.
- **Subsequent Stages**: Canonical fact correction and sync scheduling are NOT
  performed here; they require the profile-3 full daily and session imports to
  both succeed, orchestrated in Chunk 05.

## Checklist

- [x] Create `src-tauri/src/infrastructure/database/antigravity_baseline_repair.rs`.
- [x] Implement strict eligibility check rejecting any prior profile-2 `import_runs` rows.
- [x] Implement reclassification covering both `first_seen` and `legacy_unknown` origins within exact bounds.
- [x] Persist `stage = 'cache_reclassified'` upon successful commit.
- [x] Add unit tests verifying:
  - Proven initial run reclassifies both CLI `first_seen` and App/IDE `legacy_unknown` rows.
  - Presence of an earlier failed run (even with `records_seen = 0`) safely skips repair (`skip_reason: "prior_profile2_runs_exist"`).
  - Missing matching session import safely skips repair (`skip_reason: "missing_matching_session_run"`).
  - Pruned history safely skips repair (`skip_reason: "no_profile2_full_run"`).
  - Re-invoking an already reclassified repair does not duplicate SQL execution.
- [x] Verify: `cargo test --manifest-path src-tauri/Cargo.toml antigravity_baseline_repair`.

## Test Plan

- **Invariants to Prove**:
  - Reclassification never runs if an earlier profile-2 attempt existed.
  - `source_reported` rows are never altered.
  - Both CLI and App/IDE pre-existing cohorts are repaired.
- **Commands**:
  - `cargo test --manifest-path src-tauri/Cargo.toml antigravity_baseline_repair`
  - `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
  - `pnpm architecture:check`
  - `pnpm verify:fast`
  - `pnpm verify`
  - `pnpm verify:runtime`

## Decisions

- **Zero-tolerance for prior runs**: Rejecting any earlier run (even failed) is
  the only way to mathematically guarantee that unrecorded cache batch writes did
  not precede the analyzed interval.
- **Batch size correction**: Aligned documentation with the 20-conversation
  batch constant in `usage_cache.rs`.
- **Diagnostic event recording**: Integrated `antigravity.baseline_repair_applied`
  and `antigravity.baseline_repair_skipped` with structured payload context.

## Verification

- Command: `cargo test --manifest-path src-tauri/Cargo.toml antigravity_baseline_repair`
  - Outcome: passed (5 passed, 0 failed; all strict eligibility and idempotency tests passed).
- Command: `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
  - Outcome: passed (clean, 0 warnings, 0 errors).
- Command: `pnpm architecture:check`
  - Outcome: passed ("Architecture boundary check passed.").
- Command: `pnpm verify:fast`
  - Outcome: passed (harness checks, Prettier, ESLint, TypeScript, Clippy, jscpd clean).
- Command: `pnpm verify`
  - Outcome: passed (full local gate, 688 Rust unit tests passed, 117 Vitest tests passed).
- Command: `pnpm verify:runtime`
  - Outcome: passed ("Desktop runtime evidence passed.").

## Follow-Up Debt

- None.
