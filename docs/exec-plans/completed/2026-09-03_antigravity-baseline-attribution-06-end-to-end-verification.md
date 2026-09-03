# 2026-09-03 Antigravity Baseline Attribution 06 End-to-End Verification & Evidence

## Objective

Execute comprehensive end-to-end integration tests, runtime smoke tests, and
desktop evidence collection for the Antigravity baseline-attribution fix.
Validate fresh installation across CLI and App/IDE variants, profile-2 upgrade
repair, multi-account and signed-out sync behavior, projection failure recovery
with forced full-scope retry, intermediate stage resumption (`canonical_corrected`),
post-refresh error handling, and event invalidation.

## Scope

- End-to-end integration tests under `tests/` and `src-tauri/tests/`
- Runtime evidence generation under `docs/runtime-evidence/`
- Full repository verification gates

## Out Of Scope

- Modifying core application or domain code (already delivered in Chunks 01–05).

## Risk Class

`low` (verification, test harnesses, and runtime evidence collection).

## Impact Areas

- Test suites, verification reports, and documentation

## Test Scenarios

### Scenario 1: Fresh Installation with Large Historical CLI and App/IDE Corpus

- Seed a mock environment with 10,000 pre-existing timestamp-less records
  split between CLI SQLite databases and App/IDE SQLite databases.
- Run initial refresh.
- **Expected Outcome**:
  - All 10,000 records persist in `antigravity_usage_cache` as `UndatedBaseline`.
  - App/IDE records are not converted to dated `LegacyUnknown` records.
  - Today shows `0` tokens in `tray_summary`.
  - `antigravity_baseline_state` marks all variants `Complete`.
  - Zero duplicate tokens on second refresh.

### Scenario 2: Fresh Installation with Mixed Timestamps

- Seed historical records containing 5,000 source-reported timestamps and
  5,000 missing timestamps.
- Run initial refresh.
- **Expected Outcome**:
  - Source-reported records populate historical calendar days.
  - Undated records are excluded from calendar totals.
  - Daily token totals equal the sum of source-reported records only.

### Scenario 3: Genuinely New Post-Baseline Activity

- Following a completed baseline, inject 2 new responses lacking source
  timestamps.
- Run incremental refresh.
- **Expected Outcome**:
  - New records resolve to `Dated` with `first_seen` timestamp origin.
  - Today's token total increases by the exact tokens of the two responses.
  - Data quality is reported as `Partial` with the appropriate warning.

### Scenario 4: Profile-2 Upgrade & Historical Repair (Signed In)

- Seed an existing Burnly `0.1.29` SQLite database matching the production
  diagnostic report (7,064 first-seen/legacy-unknown records, 452M tokens on Today).
- Apply migration `0013` and start the application with an authenticated account.
- **Expected Outcome**:
  - Audit ledger verifies strict eligibility (first cache write proven).
  - Stages advance: `cache_reclassified` -> `canonical_corrected` ->
    `sync_scheduled` -> `complete`.
  - Today's daily tokens drop from 452M to 0.
  - Collect-sync outbox contains prepared batch tombstoning the inflated day.
  - Event sink publishes `usage_changed = true`.

### Scenario 5: Multi-Account & Signed-Out Repair

- Seed an installation where both Account A and Account B exist in
  `collect_sync_state`. The user is currently signed out.
- Apply migration `0013` and trigger refresh.
- **Expected Outcome**:
  - Stages advance: `cache_reclassified` -> `canonical_corrected` ->
    `sync_scheduled` -> `complete`.
  - Today's daily tokens drop from 452M to 0 locally.
  - Both Account A and Account B have the tombstone scope merged into their
    respective `pending_scope_json` in `collect_sync_state`.
  - When Account B signs in first, it uploads tombstones for Account B.
  - Account A's pending scope remains intact; when Account A signs in later,
    it uploads tombstones for Account A.

### Scenario 6: Profile-3 Projection Failure & Forced Full Retry

- Simulate a failure during profile-3 Session collection after Daily succeeds.
- **Expected Outcome**:
  - Daily import records success, but Session import records failure.
  - Repair pipeline HALTS. Canonical correction does NOT execute.
  - Stage remains `cache_reclassified`.
  - On the next refresh, `requires_full_scope()` forces `CollectionScope::Full`
    for BOTH Daily and Session (preventing Daily from reverting to incremental).
  - Upon both projections succeeding, canonical correction executes and repair
    advances to `complete`.

### Scenario 7: Prior Failed Run Rejection

- Seed a profile-2 installation where an earlier run failed before the first
  successful run.
- **Expected Outcome**:
  - Repair safely transitions to `skipped` (`reason: "prior_profile2_runs_exist"`).
  - Diagnostic `antigravity.baseline_repair_skipped` is emitted.
  - No historical records are altered.

### Scenario 8: Interrupted Baseline Recovery

- Simulate a refresh cancellation after 50% of conversation batches commit.
- **Expected Outcome**:
  - Baseline state remains `Pending`.
  - Subsequent refresh triggers full collection.
  - Second pass resumes without duplicate cache records and transitions to
    `Complete`.

### Scenario 9: Post-Refresh Repair Failure Resumability

- Simulate a failure during `on_refresh_completed` (e.g. transient database lock).
- **Expected Outcome**:
  - Refresh completes without crashing the coordinator.
  - Diagnostic warning `antigravity.baseline_repair_failed` is recorded.
  - Repair stage remains at its uncompleted state.
  - Subsequent refresh retries and successfully completes the repair.

### Scenario 10: Intermediate Resumption from `canonical_corrected`

- Seed an installation in `stage = 'canonical_corrected'` (e.g. crash after canonical
  correction before sync scheduling).
- Run an incremental refresh.
- **Expected Outcome**:
  - `on_refresh_completed` skips the outcome gate because stage is already
    `canonical_corrected`.
  - Sync scheduling executes and merges scope across all accounts in `collect_sync_state`.
  - Advances to `sync_scheduled` -> `complete`.
  - Event sink publishes `usage_changed = true`.

## Checklist

- [x] Add integration test suite for Scenarios 1–3 in `src-tauri/src/infrastructure/collectors/antigravity/adapter.rs`.
- [x] Add integration test suite for Scenarios 4–7 (profile-2 upgrade, multi-account, projection failure retry) in `src-tauri/src/infrastructure/database/antigravity_baseline_repair.rs`.
- [x] Add integration test suite for Scenarios 8–10 (interruption, error resumability, and intermediate stage resumption) in `src-tauri/src/infrastructure/collectors/antigravity/adapter.rs`, `src-tauri/src/infrastructure/database/antigravity_baseline_repair.rs`, and `src-tauri/src/application/refresh/tests.rs`.
- [x] Run fast verification gate: `pnpm verify:fast`.
- [x] Run full local gate: `pnpm verify`.
- [x] Run desktop runtime gate: `pnpm verify:runtime`.
- [x] Record verification commands, outcomes, and report in runtime evidence directory (`docs/runtime-evidence/2026-09-03-antigravity-baseline-attribution/README.md`).

## Verification

- `cargo test --manifest-path src-tauri/Cargo.toml antigravity_baseline_repair`: 10 passed.
- `cargo test --manifest-path src-tauri/Cargo.toml antigravity_baseline_store`: 5 passed.
- `cargo test --manifest-path src-tauri/Cargo.toml antigravity_cache_store`: 13 passed.
- `cargo test --manifest-path src-tauri/Cargo.toml scenario_`: 4 passed.
- `cargo test --manifest-path src-tauri/Cargo.toml merge_pending_scope_for_all_accounts`: 1 passed.
- `cargo test --manifest-path src-tauri/Cargo.toml baseline_repair_`: 2 passed.
- `pnpm architecture:check`: Passed.
- `pnpm verify:fast`: Passed.
- `pnpm verify`: Passed (709 Rust unit/integration tests + 117 TS tests passed; all security, contract, lint, formatting, duplication checks passed).
- `pnpm verify:runtime`: Passed (Vite build, Tauri prerequisite evidence, contract evidence, lifecycle/tray evidence, and background scheduler evidence passed).
- Runtime evidence documentation: Created at `docs/runtime-evidence/2026-09-03-antigravity-baseline-attribution/README.md`.

## Follow-Up Debt

- None.
