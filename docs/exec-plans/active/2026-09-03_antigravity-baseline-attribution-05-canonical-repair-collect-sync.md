# 2026-09-03 Antigravity Baseline Attribution 05 Canonical Repair & Collect-Sync Correction

## Objective

Complete the crash-safe historical repair pipeline by authoritatively updating
canonical `daily_usage` and `sessions` records, using standard lifecycle
semantics (`record_state = 'removed'`), and scheduling cloud synchronization
fallibly and durably across multiple accounts and signed-out states.

Introduce `TargetExecutionOutcome` tracking in both `ExecutionResult` and
`ExecutionFailure`, define the application-layer `AntigravityBaselineRepairCoordinator`
port, inject it into `RefreshCoordinatorHooks`, and compose the SQLite
implementation in bootstrap. Handle post-refresh repair errors explicitly with
diagnostic warnings and propagate `usage_changed` to the refresh event sink.

Define explicit stage-specific resumption rules in `on_refresh_completed` so that
intermediate repair stages can safely resume without deadlocking on collection
scope gates.

## Scope

- `src-tauri/src/application/ports/baseline_repair.rs` (port interface and outcome types)
- `src-tauri/src/application/refresh/outcome.rs` (`TargetExecutionOutcome` in `ExecutionResult` and `ExecutionFailure`)
- `src-tauri/src/application/refresh/execution.rs` (accumulate target outcomes across success and failure paths)
- `src-tauri/src/application/refresh/coordinator.rs` (invoke repair hook, handle errors, combine `usage_changed`)
- `src-tauri/src/infrastructure/database/antigravity_baseline_repair.rs` (coordinator implementation & stage-specific resumption)
- `src-tauri/src/infrastructure/database/reconciliation/`
- `src-tauri/src/infrastructure/database/collect_sync_store.rs` (`merge_pending_scope_for_all_accounts`)
- `src-tauri/src/bootstrap/` (composition in runtime bootstrap)
- Integration tests in `src-tauri/tests/`

## Out Of Scope

- Modifying cloud API push DTOs (DTO v1 already supports `record_state = 'removed'`).
- Modifying UI components (tray summary store already excludes `record_state = 'removed'`).

## Risk Class

`high` (modifies persisted daily/session usage facts and schedules cloud sync uploads).

## Impact Areas

- Refresh coordinator, outcome structures, and failure handling
- Canonical daily and session persistence tables (`daily_usage`, `sessions`, `daily_model_usage`)
- Desktop collect-sync outbox and persistent repair state

## Design Review

### Architectural Boundary & Port Definition

To prevent application code (`RefreshCoordinator`) from depending directly on
infrastructure database code:

1. **Port Interface** (`src-tauri/src/application/ports/baseline_repair.rs`):

   ```rust
   #[derive(Debug, Clone, Copy, PartialEq, Eq)]
   pub(crate) enum TargetRunOutcome {
       Succeeded,
       Partial,
       Failed,
   }

   #[derive(Debug, Clone, PartialEq, Eq)]
   pub(crate) struct TargetExecutionOutcome {
       pub(crate) source: SourceKey,
       pub(crate) projection: CollectionProjection,
       pub(crate) effective_scope: CollectionScope,
       pub(crate) outcome: TargetRunOutcome,
   }

   #[derive(Debug, Clone, Copy, PartialEq, Eq)]
   pub(crate) struct RepairCompletion {
       pub(crate) usage_changed: bool,
   }

   pub(crate) trait AntigravityBaselineRepairCoordinator: Send + Sync {
       fn requires_full_scope(&self) -> Result<bool, BaselineRepairError>;
       fn current_stage(&self) -> Result<AntigravityBaselineRepairStage, BaselineRepairError>;
       fn on_refresh_completed(
           &self,
           target_outcomes: &[TargetExecutionOutcome],
           now_ms: i64,
       ) -> Result<Option<RepairCompletion>, BaselineRepairError>;
   }
   ```

2. **Hook Injection** (`src-tauri/src/application/refresh/coordinator.rs`):
   Add `baseline_repair_coordinator: Arc<dyn AntigravityBaselineRepairCoordinator>`
   to `RefreshCoordinatorHooks` (defaulting to a no-op fake in unit tests).
3. **Bootstrap Composition** (`src-tauri/src/bootstrap/`):
   Instantiate `SqliteAntigravityBaselineRepairCoordinator` wrapping `Database`
   and inject it into `RefreshCoordinatorHooks` during application startup.

### Per-Target Outcome Tracking (`outcome.rs` & `execution.rs`)

Both `ExecutionResult` and `ExecutionFailure` must preserve target outcomes across
normal completion and early failure paths:

```rust
pub(super) struct ExecutionResult {
    pub(super) outcome: RunOutcome,
    pub(super) finished_at_ms: i64,
    pub(super) usage_changed: bool,
    pub(super) committed_daily_upload: crate::application::collect_sync::CommittedDailyUpload,
    pub(super) target_outcomes: Vec<TargetExecutionOutcome>,
}

pub(super) struct ExecutionFailure {
    ...
    pub(super) target_outcomes: Vec<TargetExecutionOutcome>,
}
```

In `execute_refresh_job`, record each target's execution outcome into
`aggregate.target_outcomes` so that partial failure paths retain prior target
results.

### Post-Refresh Execution, Error Handling & Invalidation

In `RefreshCoordinator::finish_refresh` / worker thread:

```rust
let repair_completion = {
    let coordinator = self.baseline_repair_coordinator.clone();
    match coordinator.on_refresh_completed(&result.target_outcomes, result.finished_at_ms) {
        Ok(completion) => completion,
        Err(error) => {
            // Do not fail refresh or crash coordinator; record warning diagnostic.
            // Stage remains at current resumable position.
            self.record_diagnostic(
                DiagnosticSeverity::Warning,
                "antigravity.baseline_repair_failed",
                error.to_string(),
            );
            None
        }
    }
};

let usage_changed = result.usage_changed
    || repair_completion.map(|c| c.usage_changed).unwrap_or(false);

self.event_sink.publish(snapshot, usage_changed);
```

### Stage-Specific Resumption Rules (`on_refresh_completed`)

To ensure crash-safety across all intermediate stages without deadlock:

```rust
fn on_refresh_completed(
    &self,
    target_outcomes: &[TargetExecutionOutcome],
    now_ms: i64,
) -> Result<Option<RepairCompletion>, BaselineRepairError> {
    let mut current_stage = self.current_stage()?;

    // 1. Stage: CacheReclassified
    if current_stage == AntigravityBaselineRepairStage::CacheReclassified {
        let daily_full_success = target_outcomes.iter().any(|o| {
            o.source == SourceKey::Antigravity
                && o.projection == CollectionProjection::Daily
                && o.effective_scope == CollectionScope::Full
                && o.outcome == TargetRunOutcome::Succeeded
        });
        let session_full_success = target_outcomes.iter().any(|o| {
            o.source == SourceKey::Antigravity
                && o.projection == CollectionProjection::Session
                && o.effective_scope == CollectionScope::Full
                && o.outcome == TargetRunOutcome::Succeeded
        });

        if !daily_full_success || !session_full_success {
            // Halt: do not run canonical correction; wait for next full refresh retry.
            return Ok(None);
        }

        // Apply authoritative canonical correction (tombstones empty days/sessions)
        self.apply_canonical_correction(now_ms)?;
        self.set_stage(AntigravityBaselineRepairStage::CanonicalCorrected, now_ms)?;
        current_stage = AntigravityBaselineRepairStage::CanonicalCorrected;
    }

    // 2. Stage: CanonicalCorrected (Skips outcome gate! Can run during incremental refresh)
    if current_stage == AntigravityBaselineRepairStage::CanonicalCorrected {
        let upload_scope = self.compute_repair_upload_scope()?;
        self.collect_store.merge_pending_scope_for_all_accounts(&upload_scope, now_ms)?;
        if self.auth_state.is_signed_in() {
            self.collect_sync.kick();
        }
        self.set_stage(AntigravityBaselineRepairStage::SyncScheduled, now_ms)?;
        current_stage = AntigravityBaselineRepairStage::SyncScheduled;
    }

    // 3. Stage: SyncScheduled
    if current_stage == AntigravityBaselineRepairStage::SyncScheduled {
        self.baseline_store.complete_all_variants(now_ms)?;
        self.set_stage(AntigravityBaselineRepairStage::Complete, now_ms)?;
        return Ok(Some(RepairCompletion { usage_changed: true }));
    }

    Ok(None)
}
```

- **Idempotency**:
  - `apply_canonical_correction` sets `record_state = 'removed'` on dates lacking
    dated candidates; re-running it is a no-op on already-tombstoned dates.
  - `merge_pending_scope_for_all_accounts` unions the date range with existing
    pending scopes in `collect_sync_state`. Re-merging the same range is idempotent.
  - `complete_all_variants` sets baseline status to `Complete`.

### Canonical Lifecycle Semantics (Tombstones)

Existing Burnly removal policy preserves historical payloads while updating
lifecycle metadata. Empty dates resulting from reclassification are tombstoned
identically:

```sql
UPDATE daily_usage
SET record_state = 'removed',
    absence_count = 2,
    removed_at_ms = ?1
WHERE source_id = ?2
  AND usage_date IN (/* affected dates with zero remaining candidates */);
```

- **Payload Integrity**: Token amounts, cost micros, and `daily_model_usage`
  rows remain intact.
- **Query Effect**: `SqliteTraySummaryStore` queries `WHERE record_state <> 'removed'`,
  so tombstoned days vanish immediately from tray totals, trends, and model
  breakdowns without zeroing data.
- **Sessions**: Sessions whose records are all `undated_baseline` are similarly
  tombstoned (`record_state = 'removed'`, `absence_count = 2`, `removed_at_ms = now_ms`).
- **Check Constraints**: Matches the schema invariant:
  `(record_state = 'removed' AND absence_count >= 2 AND removed_at_ms IS NOT NULL)`.

### Account-Safe Fallible Sync Scheduling

`collect_sync_state` is keyed by `(user_id, client_device_id)`.

1. Implement `merge_pending_scope_for_all_accounts(&self, scope: &UploadScope, now_ms: i64) -> Result<usize, CollectSyncStoreError>`
   in `CollectSyncStore`:
   - Iterates across all existing accounts in `collect_sync_state`.
   - Merges `scope` into each account's `pending_scope_json`.
   - If currently signed in, kicks active sync worker immediately.
   - If currently signed out, each account preserves its pending scope until login.
   - Newly seen accounts perform full baseline uploads from clean local facts.
2. Only when `merge_pending_scope_for_all_accounts` successfully commits does
   the repair state advance to `stage = 'sync_scheduled'`.

## Checklist

- [x] Define `AntigravityBaselineRepairCoordinator` trait, `TargetRunOutcome`, `TargetExecutionOutcome`, and `RepairCompletion` in `src-tauri/src/application/ports/baseline_repair.rs`.
- [x] Add `target_outcomes` to `ExecutionResult` and `ExecutionFailure` in `src-tauri/src/application/refresh/outcome.rs` and populate in `execution.rs`.
- [x] Add `baseline_repair_coordinator` to `RefreshCoordinatorHooks` and call `on_refresh_completed` in `coordinator.rs`.
- [x] Handle post-refresh errors: log diagnostic warning, keep stage resumable, combine `result.usage_changed || repair.usage_changed`.
- [x] Implement stage-specific resumption in `SqliteAntigravityBaselineRepairCoordinator`:
  - Gate on outcomes only in `cache_reclassified`.
  - Skip outcome gate in `canonical_corrected` and retry sync scheduling.
  - Finish baseline completion in `sync_scheduled`.
- [x] Implement `merge_pending_scope_for_all_accounts` on `CollectSyncStore`.
- [x] Implement canonical daily and session tombstones (`record_state = 'removed'`).
- [x] Compose coordinator in `src-tauri/src/bootstrap/` and wire into `RefreshCoordinator`.
- [x] Add integration tests verifying:
  - If daily import fails or session import fails, repair stage stays `cache_reclassified` and canonical correction does not run.
  - On retry following partial failure, both projections run with `CollectionScope::Full`.
  - If sync scheduling fails after `canonical_corrected`, the next refresh skips the outcome gate and successfully resumes sync scheduling and completion.
  - When repair completes, event sink publishes `usage_changed = true`.
  - When repair returns an error, diagnostic warning is emitted and repair remains resumable.
  - Scope is merged into all accounts in `collect_sync_state`; signed-out accounts preserve scope until sign-in.
- [x] Verify: `cargo test --manifest-path src-tauri/Cargo.toml canonical_repair`.

## Test Plan

- **Invariants to Prove**:
  - `RefreshCoordinator` respects application-layer port boundary.
  - `canonical_corrected` stage resumes sync scheduling without requiring full refresh outcomes.
  - Post-refresh errors fail open for the refresh run while preserving resumable repair state.
  - Usage changes from canonical repair trigger event sink notifications.
  - Target outcomes are preserved through `ExecutionFailure`.
  - Retry after partial failure forces `CollectionScope::Full` for both projections.
- **Commands**:
  - `cargo test --manifest-path src-tauri/Cargo.toml canonical_repair`
  - `pnpm verify:fast`

## Decisions

- **Application port for repair**: Decoupling through `AntigravityBaselineRepairCoordinator`
  preserves the clean dependency direction: application refresh code does not
  depend on SQLite database infrastructure.
- **Stage-specific outcome gating**: Gating only `cache_reclassified` on full
  outcomes allows `canonical_corrected` to complete sync scheduling even during
  routine incremental refreshes.
- **Target outcome tracking across all paths**: Preserving `target_outcomes` in
  both `ExecutionResult` and `ExecutionFailure` ensures accurate visibility into
  prior steps if a later target fails.
- **Resumable repair failures**: Failing open with a diagnostic warning prevents
  background refresh jobs from crashing while keeping repair state safe to resume.

## Verification

- `cargo test --manifest-path src-tauri/Cargo.toml antigravity_baseline_repair`: 8 passed.
- `cargo test --manifest-path src-tauri/Cargo.toml merge_pending_scope_for_all_accounts`: 1 passed.
- `cargo test --manifest-path src-tauri/Cargo.toml baseline_repair_`: 2 passed.
- `pnpm architecture:check`: Passed.
- `pnpm verify:fast`: Passed.
- `pnpm verify`: Passed (705 Rust tests + 117 TS tests passed; all contract, lint, formatting, duplication checks passed).
- `pnpm verify:runtime`: Passed (desktop build, packaging, contract evidence, lifecycle and scheduler evidence passed).

## Follow-Up Debt

- None.
