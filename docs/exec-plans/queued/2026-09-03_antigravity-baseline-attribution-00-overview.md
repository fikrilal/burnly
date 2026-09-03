# 2026-09-03 Antigravity Baseline Attribution 00 Overview

## Objective

Coordinate the remediation of the Antigravity baseline-attribution defect in
Burnly (Workstream B of
`docs/planning/_WIP/opencode-macos-and-antigravity-baseline-remediation-proposal.md`).

A fresh installation currently conflates _first-observation time_ with _activity
time_, assigning pre-existing historical Antigravity records without a
source-reported timestamp to the installation day (accounting for >450M tokens
attributed to "Today" after two prompts). This phase establishes durable
per-variant baseline tracking, unifies App/IDE and CLI timestamp resolution,
separates calendar eligibility (`dated` vs `undated_baseline`), excludes
undated baseline history from calendar totals, deterministically repairs
affected Burnly `0.1.29` installations via a multi-stage crash-safe pipeline,
and propagates canonical corrections to cloud sync safely across multiple
accounts and signed-out states.

## Acceptance Criteria

- Pre-existing Antigravity records lacking source timestamps discovered during
  initial baseline do not participate in calendar totals (`daily_usage`,
  `sessions`, `daily_model_usage`).
- App/IDE records without source timestamps no longer bypass attribution via
  `LegacyUnknown` conversion in `cached_record_from_usage`; both App/IDE and CLI
  records flow uniformly through baseline attribution.
- In-flight or legacy records carrying `LegacyUnknown` during baseline `Pending`
  are explicitly attributed as `UndatedBaseline`.
- Pre-existing undated records retain exact identity and token counters in the
  cache store so subsequent scans do not duplicate them.
- Source-timestamped records discovered during baseline are attributed to their
  source dates.
- Genuinely new timestamp-less records appearing _after_ baseline completion are
  attributed to their durable first-seen date with `DataQuality::Partial`.
- Variants with zero initial artifacts transition baseline to `Complete`
  immediately so future records are dated.
- Interrupted or failed baseline scans remain `Pending` and safely retry full
  reconciliation without duplicate tokens or premature completion.
- The repair pipeline follows an explicit, runtime-orchestrated sequence:
  `migration` -> `cache reclassification` -> `profile-3 full daily/session refresh`
  -> `canonical correction` -> `durable sync scheduling` -> `complete`.
- Intermediate repair stages in `on_refresh_completed` are fully resumable:
  - `cache_reclassified`: requires two successful full profile-3 outcomes, then
    corrects canonical facts and advances to `canonical_corrected`.
  - `canonical_corrected`: skips the outcome gate and retries sync scheduling,
    advancing to `sync_scheduled`.
  - `sync_scheduled`: finishes baseline and repair completion, advancing to
    `complete`.
  - Each transition is strictly idempotent.
- An authoritative application-layer port (`AntigravityBaselineRepairCoordinator`)
  mediates repair execution, injected into `RefreshCoordinatorHooks` and composed
  in bootstrap, preserving the hexagonal architecture boundary.
- Scope override wiring is complete: `RefreshExecution` stores the port reference,
  passed from `RefreshCoordinator` into `planned_collection_request`. Scope
  evaluation is fallible (`requires_full_scope() -> Result<bool, BaselineRepairError>`),
  forcing `CollectionScope::Full` while stage is `cache_reclassified` (or baseline
  is `Pending`).
- `ExecutionResult` and `ExecutionFailure` track per-target outcomes
  (`TargetExecutionOutcome`: `source`, `projection`, `effective_scope`, `outcome`).
- Post-refresh repair failures in `on_refresh_completed` are handled explicitly:
  errors do not mark repair complete, the stage remains resumable, and a
  diagnostic warning is recorded.
- Event publication reflects canonical changes: `on_refresh_completed` returns
  `Option<RepairCompletion>` carrying `usage_changed`, which is combined via
  `result.usage_changed || repair.usage_changed` before publishing.
- The profile-2 repair window is strictly proven (initial full daily + session
  runs with zero prior profile-2 attempts whatsoever and exact run timestamp
  bounds); otherwise automatic repair is safely skipped without heuristic
  guessing.
- Corrected daily usage facts and sessions follow established lifecycle
  semantics (`record_state = 'removed'`, `removed_at_ms = now`,
  `absence_count = 2`) leaving payload and model rows intact for audit.
- Canonical corrections schedule cloud sync outbox pushes in an account-safe
  manner by merging the upload scope into every existing local
  `collect_sync_state` account row; newly seen accounts receive the corrected
  full baseline automatically.
- All verification checks (`pnpm verify:fast`, `pnpm verify`,
  `pnpm verify:runtime`) pass.

## Risk Class

`high` (alters core usage ingestion, database schema, historical data repair,
and cloud synchronization for Antigravity).

## Impact Areas

- `src-tauri/migrations/0013_antigravity_baseline_attribution.sql`
- `src-tauri/src/infrastructure/database/migrations.rs`
- `src-tauri/src/application/ports/antigravity_usage_cache.rs`
- `src-tauri/src/application/ports/antigravity_baseline_store.rs`
- `src-tauri/src/application/ports/baseline_repair.rs` (authoritative port & outcome types)
- `src-tauri/src/application/refresh/` (`outcome.rs`, `execution.rs`, `coordinator.rs`, `request_plan.rs`)
- `src-tauri/src/infrastructure/database/antigravity_cache_store.rs`
- `src-tauri/src/infrastructure/database/antigravity_baseline_store.rs`
- `src-tauri/src/infrastructure/database/antigravity_baseline_repair.rs`
- `src-tauri/src/infrastructure/collectors/antigravity/` (`adapter.rs`, `mapper.rs`, `usage_cache.rs`, `mod.rs`)
- `src-tauri/src/infrastructure/database/reconciliation/`
- `src-tauri/src/bootstrap/` (dependency injection into `RefreshCoordinatorHooks`)

## Architecture and Invariants

1. **Boundary Independence**: Rust domain and application code remain free of
   SQLite and Tauri dependencies. Application ports (`AntigravityBaselineRepairCoordinator`,
   `AntigravityBaselineStore`) decouple `RefreshCoordinator` from infrastructure
   database implementations.
2. **Explicit Runtime Orchestration & Stage-Specific Resumption**:
   ```text
   Startup Migration (0013)
     ↓
   Cache Reclassification (AntigravityCollector::collect entry)
     ↓ [stage: cache_reclassified]
   Request Planning Scope Enforcement (RefreshExecution -> planned_collection_request calls requires_full_scope()?)
     ↓
   Profile-3 Refresh Execution (RefreshCoordinator)
     ↓
   Post-Refresh Coordinator Hook (on_refresh_completed):
     ├─ [stage == cache_reclassified]
     │    ├─ (Daily OR Session failed/not full) ──→ HALT (canonical correction bypassed; stage stays cache_reclassified)
     │    └─ (Both Daily AND Session succeeded with Full scope)
     │         ↓
     │       Canonical Correction (Tombstone empty days/sessions)
     │         ↓ [stage: canonical_corrected]
     │       (Fallthrough to sync scheduling)
     │
     ├─ [stage == canonical_corrected]
     │    (Skip outcome gate: canonical facts already corrected!)
     │    Account-Safe Sync Scheduling (Merge scope into all collect_sync_state rows)
     │      ↓ [stage: sync_scheduled]
     │    (Fallthrough to completion)
     │
     └─ [stage == sync_scheduled]
          Complete (Mark variant baselines Complete & repair complete)
            ↓ [stage: complete]
            ↓ Returns RepairCompletion { usage_changed: true }
   ```
3. **Per-Target Outcome Visibility**: `ExecutionResult` and `ExecutionFailure`
   carry `target_outcomes: Vec<TargetExecutionOutcome>` recording `source`,
   `projection`, `effective_scope`, and `outcome`.
4. **Resumable Post-Refresh Failures**: If `on_refresh_completed` encounters a
   storage failure, it returns `Err(err)`. `RefreshCoordinator` logs a
   diagnostic warning and leaves the repair stage at its current state for retry.
5. **Accurate Event Notification**: Canonical correction signals `usage_changed = true`
   via `RepairCompletion`, ensuring the event sink updates the tray UI.
6. **Account-Safe Cloud Sync**: `collect_sync_state` is keyed by `user_id` and
   `client_device_id`. The repair merges the tombstone scope into _every_
   existing account row on the device. An account never loses its correction to
   another account, and newly registered accounts upload clean baselines.
7. **Uniform Variant Attribution**: App, IDE, and CLI records follow the same
   unresolved-timestamp attribution rules. App/IDE records must not be
   converted to dated `LegacyUnknown` records prior to cache reconciliation.
8. **Lifecycle Semantics**: Empty historical days are tombstoned via standard
   `record_state = 'removed'` with `absence_count = 2` and `removed_at_ms`.
   Fact payload and model rows are preserved.
9. **Conservative Eligibility**: Historical repair interval strictly rejects any
   installation that had _any_ prior profile-2 run attempts, preventing
   misattribution from partial unrecorded cache writes.

## Chunks and Status

All Antigravity plans remain queued while the OpenCode implementation plan
remains active in `docs/exec-plans/active/`. Once OpenCode is complete, Chunk 01
will be activated.

| Chunk  | Title                                      | Status     | Location                                                                                                      |
| :----- | :----------------------------------------- | :--------- | :------------------------------------------------------------------------------------------------------------ |
| **01** | Database Migration & Port Types            | **Queued** | `docs/exec-plans/queued/2026-09-03_antigravity-baseline-attribution-01-database-migration-port-types.md`      |
| **02** | Usage Cache Store Attribution Logic        | Queued     | `docs/exec-plans/queued/2026-09-03_antigravity-baseline-attribution-02-cache-store-attribution.md`            |
| **03** | Profile-2 Historical Repair Service        | Queued     | `docs/exec-plans/queued/2026-09-03_antigravity-baseline-attribution-03-profile2-repair-service.md`            |
| **04** | Adapter Orchestration & Profile Bump       | Queued     | `docs/exec-plans/queued/2026-09-03_antigravity-baseline-attribution-04-adapter-orchestration-profile-bump.md` |
| **05** | Canonical Repair & Collect-Sync Correction | Queued     | `docs/exec-plans/queued/2026-09-03_antigravity-baseline-attribution-05-canonical-repair-collect-sync.md`      |
| **06** | End-to-End Verification & Evidence         | Queued     | `docs/exec-plans/queued/2026-09-03_antigravity-baseline-attribution-06-end-to-end-verification.md`            |

## Verification Plan

- Fast local gate: `pnpm verify:fast`
- Full local gate: `pnpm verify`
- Desktop runtime gate: `pnpm verify:runtime`
- Desktop evidence: `pnpm evidence:desktop`
