# 2026-09-03 Antigravity Baseline Attribution 01 Database Migration & Port Types

## Objective

Introduce database migration `0013_antigravity_baseline_attribution.sql` and the
corresponding application port interfaces. The schema and ports provide:

1. `calendar_attribution` column on `antigravity_usage_cache` (`dated` vs
   `undated_baseline`).
2. `antigravity_baseline_state` table tracking per-variant baseline progress
   (`pending`, `complete`).
3. `antigravity_baseline_repair_state` table tracking multi-stage crash-safe
   repair progress (`not_started`, `cache_reclassified`, `canonical_corrected`,
   `sync_scheduled`, `complete`, `skipped`).
4. Authoritative application port traits and domain types for baseline tracking,
   scope evaluation, and repair coordination:
   - `AntigravityBaselineStore`
   - `AntigravityBaselineRepairCoordinator`
   - `TargetRunOutcome`, `TargetExecutionOutcome`, and `RepairCompletion`
   - `AntigravityCalendarAttribution`

## Scope

- `src-tauri/migrations/0013_antigravity_baseline_attribution.sql`
- `src-tauri/src/infrastructure/database/migrations.rs`
- `src-tauri/src/application/ports/antigravity_usage_cache.rs`
- `src-tauri/src/application/ports/antigravity_baseline_store.rs`
- `src-tauri/src/application/ports/baseline_repair.rs`
- `src-tauri/src/infrastructure/database/antigravity_baseline_store.rs`
- Migration regression tests in `migrations.rs`

## Out Of Scope

- Modifying cache store timestamp resolution or queries (Chunk 02).
- Historical profile-2 repair execution (Chunk 03).
- Collector adapter orchestration and profile bump (Chunk 04).
- Canonical reconciliation replacement or collect-sync outbox push (Chunk 05).

## Risk Class

`medium` (database schema migration with strict check constraints and new tables).

## Impact Areas

- SQLite schema and forward-only migration sequence
- Application port contracts

## Design Review

### Schema Details

```sql
-- 1. Add calendar_attribution to antigravity_usage_cache
ALTER TABLE antigravity_usage_cache
    ADD COLUMN calendar_attribution TEXT NOT NULL DEFAULT 'dated'
    CHECK (calendar_attribution IN ('dated', 'undated_baseline'));

-- 2. Index for calendar-eligible scope queries
CREATE INDEX idx_antigravity_usage_cache_calendar_scope
    ON antigravity_usage_cache (variant, calendar_attribution, observed_at_ms);

-- 3. Durable baseline state per product variant
CREATE TABLE antigravity_baseline_state (
    variant TEXT PRIMARY KEY CHECK (
        variant IN ('antigravity', 'antigravity-ide', 'antigravity-cli')
    ),
    status TEXT NOT NULL CHECK (status IN ('pending', 'complete')),
    started_at_ms INTEGER NOT NULL CHECK (started_at_ms >= 0),
    completed_at_ms INTEGER CHECK (
        (status = 'pending' AND completed_at_ms IS NULL)
        OR (status = 'complete' AND completed_at_ms IS NOT NULL AND completed_at_ms >= started_at_ms)
    ),
    updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms >= started_at_ms)
) STRICT;

-- 4. Audit ledger for crash-safe baseline repair
CREATE TABLE antigravity_baseline_repair_state (
    repair_version INTEGER PRIMARY KEY CHECK (repair_version > 0),
    stage TEXT NOT NULL CHECK (
        stage IN (
            'not_started',
            'cache_reclassified',
            'canonical_corrected',
            'sync_scheduled',
            'complete',
            'skipped'
        )
    ),
    records_reclassified INTEGER NOT NULL DEFAULT 0 CHECK (records_reclassified >= 0),
    import_run_id INTEGER,
    interval_started_at_ms INTEGER CHECK (
        interval_started_at_ms IS NULL OR interval_started_at_ms >= 0
    ),
    interval_finished_at_ms INTEGER CHECK (
        interval_finished_at_ms IS NULL OR interval_finished_at_ms >= interval_started_at_ms
    ),
    stage_updated_at_ms INTEGER NOT NULL CHECK (stage_updated_at_ms >= 0),
    skip_reason TEXT,
    FOREIGN KEY (import_run_id) REFERENCES import_runs(id) ON DELETE SET NULL
) STRICT;
```

### Port Contracts

- `AntigravityCalendarAttribution`: `Dated`, `UndatedBaseline`.
- `AntigravityBaselineStatus`: `Pending`, `Complete`.
- `AntigravityBaselineStore` trait:
  - `get_status(variant)`
  - `begin_baseline(variant, started_at_ms)`
  - `complete_baseline(variant, completed_at_ms)`
  - `list_statuses()`
- `AntigravityBaselineRepairStage`:
  `NotStarted`, `CacheReclassified`, `CanonicalCorrected`, `SyncScheduled`,
  `Complete`, `Skipped`.
- Authoritative `AntigravityBaselineRepairCoordinator` trait (in `src-tauri/src/application/ports/baseline_repair.rs`):

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
      /// Fallible check: returns true if Antigravity requires a full collection scope
      /// because baseline is Pending or repair stage is cache_reclassified.
      fn requires_full_scope(&self) -> Result<bool, BaselineRepairError>;

      /// Current durable stage of the repair pipeline.
      fn current_stage(&self) -> Result<AntigravityBaselineRepairStage, BaselineRepairError>;

      /// Invoked after refresh execution finishes with target outcomes.
      /// Returns Ok(Some(RepairCompletion)) if canonical repair ran and succeeded.
      /// Returns Ok(None) if repair was not needed or not triggered.
      /// Returns Err(e) if canonical repair or sync scheduling failed.
      fn on_refresh_completed(
          &self,
          target_outcomes: &[TargetExecutionOutcome],
          now_ms: i64,
      ) -> Result<Option<RepairCompletion>, BaselineRepairError>;
  }
  ```

## Checklist

- [ ] Create `src-tauri/migrations/0013_antigravity_baseline_attribution.sql`.
- [ ] Register migration in `src-tauri/src/infrastructure/database/migrations.rs` and update `LATEST_SCHEMA_VERSION = 13`.
- [ ] Add `AntigravityCalendarAttribution` to `src-tauri/src/application/ports/antigravity_usage_cache.rs`.
- [ ] Define `AntigravityBaselineStore` in `src-tauri/src/application/ports/antigravity_baseline_store.rs`.
- [ ] Define `AntigravityBaselineRepairCoordinator`, `TargetExecutionOutcome`, and `RepairCompletion` in `src-tauri/src/application/ports/baseline_repair.rs`.
- [ ] Implement `SqliteAntigravityBaselineStore` in `src-tauri/src/infrastructure/database/antigravity_baseline_store.rs`.
- [ ] Add migration tests proving upgrade from version 12 to 13 preserves existing rows as `dated` and enforces constraints.
- [ ] Verify: `pnpm verify:fast`.

## Test Plan

- **Invariants to Prove**:
  - Existing cache rows default to `dated`.
  - Check constraints reject invalid attribution, baseline status, and repair stage values.
  - Foreign key check passes.
- **Test Layer**: Rust unit/migration tests in `migrations.rs` and `antigravity_baseline_store.rs`.
- **Commands**:
  - `cargo test --manifest-path src-tauri/Cargo.toml migrations`
  - `pnpm migrations:check`
  - `pnpm verify:fast`

## Decisions

- **Default value for `calendar_attribution`**: Defaults to `'dated'` so that
  existing cache rows remain valid upon migration. Reclassification of the
  profile-2 bootstrap cohort occurs in Chunk 03.
- **Fallible `requires_full_scope`**: Returning `Result<bool, BaselineRepairError>`
  ensures that database read failures fail closed and never fall back to an
  incremental collection.
- **Application port decoupling**: `AntigravityBaselineRepairCoordinator` lives
  in `application/ports/`, allowing `RefreshCoordinator` to orchestrate repair
  without depending on SQLite infrastructure code.

## Verification

- Queued (pending activation after OpenCode).

## Follow-Up Debt

- None.
