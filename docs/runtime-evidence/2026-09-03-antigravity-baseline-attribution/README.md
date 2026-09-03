# Antigravity Baseline Attribution & Historical Repair Runtime Evidence — September 3, 2026

## Result

The Antigravity baseline attribution and historical repair subsystem has been fully implemented,
integrated, and verified end-to-end across all 10 target scenarios. Historical conversation
records lacking native source timestamps are attributed as undated baseline records during
initial profile-3 collection rather than inflating Today's token totals. For existing profile-2
installations affected by the timestamp inference bug, the repair service safely verifies
eligibility via strict audit ledger proof, tombstones empty historical daily and session records
(`record_state = 'removed'`), schedules collect-sync tombstones across all registered accounts,
and resumes cleanly across interrupted runs or transient failures without coordinator deadlock.

## Verified Scenarios & Invariant Evidence

| Scenario                                                            | Invariant / Behavior Tested                                                                                                                                                                                                     | Verification Outcome                                                                                                                              |
| :------------------------------------------------------------------ | :------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | :------------------------------------------------------------------------------------------------------------------------------------------------ |
| **Scenario 1**: Fresh Installation with Large CLI & App/IDE Corpus  | Historical records without source timestamps persist as `UndatedBaseline`. App/IDE records are never converted to dated `LegacyUnknown`. Today shows `0` daily candidates. Subsequent collections add zero duplicate records.   | `scenario_1_fresh_installation_with_cli_and_app_ide_corpus_yields_zero_today_tokens` passed.                                                      |
| **Scenario 2**: Fresh Installation with Mixed Timestamps            | Source-reported timestamps populate historical calendar days with complete data quality. Undated records are excluded from daily totals.                                                                                        | `initial_baseline_scan_with_source_timestamped_records_yields_dated_daily_tokens` passed.                                                         |
| **Scenario 3**: Genuinely New Post-Baseline Activity                | Prompts executed after baseline completion infer `first_seen` timestamps, populate Today's token totals, and report `Partial` data quality.                                                                                     | `post_baseline_new_prompt_yields_daily_tokens_with_inferred_first_seen` passed.                                                                   |
| **Scenario 4**: Profile-2 Upgrade & Historical Repair (Signed In)   | Eligible databases reclassify `first_seen` records within the initial run interval to `undated_baseline`, tombstone canonical empty dates and sessions, schedule sync tombstones, and notify the UI via `usage_changed = true`. | `canonical_repair_tombstones_empty_dates_and_sessions_and_advances_to_complete` passed.                                                           |
| **Scenario 5**: Multi-Account & Signed-Out Repair                   | Repair merges the repaired upload date range into `pending_scope_json` for all accounts in `collect_sync_state`. Signed-out state prevents immediate upload kick while preserving pending scope for subsequent login.           | `signed_out_account_preserves_merged_scope_until_login` passed.                                                                                   |
| **Scenario 6**: Profile-3 Projection Failure & Forced Full Retry    | Partial failure (Daily succeeded, Session failed) halts repair in `cache_reclassified`. `requires_full_scope()` forces `CollectionScope::Full` for both projections on retry. Upon full success, repair completes.              | `scenario_6_profile3_failure_recovery_with_forced_full_scope_and_completion` passed.                                                              |
| **Scenario 7**: Prior Failed Run Rejection                          | Installations with an earlier failed run before the first successful profile-2 run safely skip repair (`status = 'skipped'`, reason: `prior_profile2_runs_exist`), recording a diagnostic event.                                | `presence_of_earlier_failed_run_safely_skips_repair` passed.                                                                                      |
| **Scenario 8**: Interrupted Baseline Recovery                       | Refresh cancellation during baseline extraction keeps baseline `Pending`. A subsequent refresh resumes with full collection, transitions to `Complete`, and creates no duplicate cache records.                                 | `scenario_8_interrupted_baseline_recovery_resumes_without_duplicates` passed.                                                                     |
| **Scenario 9**: Post-Refresh Repair Failure Resumability            | Transient errors in post-refresh repair hooks fail open for the refresh run, record an `antigravity.baseline_repair_failed` diagnostic warning, and resume successfully on the next refresh invocation.                         | `baseline_repair_failure_emits_diagnostic_and_does_not_fail_refresh` and `scenario_9_post_refresh_repair_failure_resumes_on_next_refresh` passed. |
| **Scenario 10**: Intermediate Resumption from `canonical_corrected` | If the application terminates after canonical correction but before sync scheduling, subsequent refreshes skip the full-outcome gate, resume sync scheduling, and complete the baseline.                                        | `resumption_skips_outcome_gate_when_stage_is_canonical_corrected` passed.                                                                         |

## Verification Gates & Commands Executed

1. **Focused Subsystem Unit & Integration Tests**:
   - `cargo test --manifest-path src-tauri/Cargo.toml antigravity_baseline_repair` (10 passed)
   - `cargo test --manifest-path src-tauri/Cargo.toml antigravity_baseline_store` (5 passed)
   - `cargo test --manifest-path src-tauri/Cargo.toml antigravity_cache_store` (13 passed)
   - `cargo test --manifest-path src-tauri/Cargo.toml scenario_` (4 passed)
   - `cargo test --manifest-path src-tauri/Cargo.toml merge_pending_scope_for_all_accounts` (1 passed)
   - `cargo test --manifest-path src-tauri/Cargo.toml baseline_repair_` (2 passed)

2. **Architecture & Contract Boundary Gate**:
   - `pnpm architecture:check`
   - Output: `Architecture boundary check passed.` (0 violations across application, domain, infrastructure, IPC, and platform layers).

3. **Fast Verification Gate**:
   - `pnpm verify:fast`
   - Output: Prettier, ESLint, TypeScript check, unit test runs, and duplication report all passed.

4. **Full Local Verification Gate**:
   - `pnpm verify`
   - Output: 709 Rust unit/integration tests + 117 TypeScript tests passed; all security, packaging, contracts, migrations, sidecar, and cost-pricing checks passed.

5. **Desktop Runtime Gate & Platform Evidence**:
   - `pnpm verify:runtime`
   - Output: Vite production build transformed 2438 modules; Tauri prerequisites, IPC contract registry, IPC bridge tests, platform lifecycle/tray tests, and background refresh scheduler tests passed cleanly.
