# 2026-09-03 Antigravity Baseline Attribution 04 Adapter Orchestration & Profile Bump

## Objective

Bump the Antigravity collector profile version to `3` to trigger full
reconciliation across all installations.

Crucially, remove the variant-specific conversion in `cached_record_from_usage`
that transformed App/IDE `Unresolved` records into `LegacyUnknown` with
`database.modified_at`. App, IDE, and CLI records must uniformly preserve
`Unresolved` when upstream activity timestamps are absent so that the cache store
can attribute them as `UndatedBaseline`.

Wire the scope evaluator through application refresh execution: store the
`AntigravityBaselineRepairCoordinator` port reference in `RefreshExecution`, pass
it from `RefreshCoordinator` into `planned_collection_request`, and update
request-planning tests. Enforce the full-scope retry invariant: while
Antigravity's repair stage is `cache_reclassified` (or baseline is `Pending`),
`requires_full_scope()` returns `Ok(true)` and forces `CollectionScope::Full`
during scope selection before passing to `collection_request(...)`.

Update `AntigravityCollector` to invoke cache reclassification before scanning,
orchestrate per-variant baseline lifecycles, handle zero-artifact variants,
filter `undated_baseline` records out of daily and session mapping, and emit
bounded diagnostic counters.

## Scope

- `src-tauri/src/infrastructure/collectors/antigravity/mod.rs` (`PROFILE_VERSION = 3`)
- `src-tauri/src/infrastructure/collectors/antigravity/adapter.rs`
- `src-tauri/src/infrastructure/collectors/antigravity/usage_cache.rs`
- `src-tauri/src/infrastructure/collectors/antigravity/mapper.rs`
- `src-tauri/src/application/refresh/execution.rs` (`RefreshExecution` field and parameter passing)
- `src-tauri/src/application/refresh/coordinator.rs` (pass coordinator into `RefreshExecution`)
- `src-tauri/src/application/refresh/request_plan.rs` (signature change and scope override)
- Unit and fixture tests in `collectors/antigravity/` and `refresh/request_plan.rs`

## Out Of Scope

- Canonical database repair execution (Chunk 05).
- Cloud sync outbox scheduling (Chunk 05).

## Risk Class

`medium` (profile bump forces full reconciliation on next refresh).

## Impact Areas

- Antigravity collector capability descriptor and collection flow
- App/IDE cache record conversion in `usage_cache.rs`
- Mapping from cached records to candidate usage records
- Refresh execution context and request planning for Antigravity targets

## Design Review

### Uniform Unresolved Attribution (`usage_cache.rs`)

In `cached_record_from_usage` (`src-tauri/src/infrastructure/collectors/antigravity/usage_cache.rs:213`):

```rust
// BEFORE: Non-CLI records converted Unresolved to LegacyUnknown with modified_at
let (resolved_at, timestamp_origin) = match record.timestamp_origin {
    AntigravityTimestampOrigin::Unresolved
        if record.variant != AntigravityProductVariant::Cli =>
    {
        (
            Some(record.observed_at.unwrap_or(observed_at)),
            AntigravityTimestampOrigin::LegacyUnknown,
        )
    }
    _ => (record.observed_at, record.timestamp_origin),
};

// AFTER: Preserve Unresolved across all variants uniformly
let (resolved_at, timestamp_origin) = (record.observed_at, record.timestamp_origin);
```

By keeping `Unresolved` intact across CLI, App, and IDE, records lacking source
timestamps flow into the cache store's baseline resolution logic uniformly.

### Scope Evaluator Wiring and Signature Updates

1. **Context Struct (`execution.rs`)**:
   Add the repair coordinator port reference to `RefreshExecution`:
   ```rust
   pub(super) struct RefreshExecution<'a> {
       ...
       pub(super) baseline_repair_coordinator: &'a dyn AntigravityBaselineRepairCoordinator,
   }
   ```
2. **Coordinator Execution (`coordinator.rs`)**:
   In `finish_refresh` / worker thread execution, pass `self.baseline_repair_coordinator.as_ref()`
   into `RefreshExecution`.
3. **Execution Call Site (`execution.rs`)**:
   In `execute_open_refresh`, pass `context.baseline_repair_coordinator` into
   `planned_collection_request`:
   ```rust
   let request = planned_collection_request(
       context.run_store,
       context.baseline_repair_coordinator,
       job_id,
       target,
       profile,
       requested_at,
       &context.aggregation_timezone,
       scope_policy,
   )?;
   ```
4. **Request Plan Signature & Scope Selection (`request_plan.rs`)**:
   Update signature:

   ```rust
   pub(super) fn planned_collection_request(
       run_store: &dyn RunStore,
       repair_coordinator: &dyn AntigravityBaselineRepairCoordinator,
       job_id: &str,
       target: RefreshTarget,
       profile: &ProfileDescriptor,
       requested_at: DateTime<Utc>,
       aggregation_timezone: &str,
       scope_policy: RefreshScopePolicy,
   ) -> Result<CollectionRequest, RequestPlanError> {
       let scope = match scope_policy {
           RefreshScopePolicy::Full => crate::application::collection::CollectionScope::Full,
           RefreshScopePolicy::CatchUp | RefreshScopePolicy::Freshness => {
               // Fallible check: database errors fail closed to prevent accidental incremental scans.
               if target.source == SourceKey::Antigravity
                   && repair_coordinator
                       .requires_full_scope()
                       .map_err(|_| RequestPlanError::ImportStateUnavailable)?
               {
                   crate::application::collection::CollectionScope::Full
               } else {
                   let today = local_date(requested_at, aggregation_timezone)
                       .map_err(|_| RequestPlanError::InvalidTimezone)?;
                   let lookup = target
                       .import_lookup(aggregation_timezone, profile)
                       .map_err(|_| RequestPlanError::InvalidImportState)?;
                   let previous_import = run_store
                       .latest_successful_import(lookup)
                       .map_err(|_| RequestPlanError::ImportStateUnavailable)?;
                   let mode = match scope_policy {
                       RefreshScopePolicy::CatchUp => RefreshPlanMode::CatchUp,
                       RefreshScopePolicy::Freshness => RefreshPlanMode::Freshness,
                       RefreshScopePolicy::Full => unreachable!("full scope returned earlier"),
                   };
                   let plan = RefreshPolicyPlanner::new().plan(RefreshPlanRequest::new(
                       target.plan_target(aggregation_timezone),
                       mode,
                       today,
                       previous_import,
                   ));
                   plan.scope().clone()
               }
           }
       };

       collection_request(job_id, target, scope, requested_at, aggregation_timezone)
   }
   ```

5. **Test Fakes**: Update test suites in `request_plan.rs` to provide a mock/fake
   `AntigravityBaselineRepairCoordinator` returning `Ok(false)` or `Ok(true)`.

### Collection Entry Point & Baseline Lifecycle

1. **Pre-Collection Reclassification**:
   - At the beginning of `AntigravityCollector::collect`, call
     `AntigravityBaselineRepairService::ensure_cache_reclassified`.
   - If repair is in `not_started`, it verifies eligibility and reclassifies
     the profile-2 cache cohort to `undated_baseline` before any artifacts are
     read or queried from cache.
2. **Discovery & Zero-Artifact Handling**:
   - Query `AntigravityBaselineStore` for each variant (`antigravity`,
     `antigravity-ide`, `antigravity-cli`).
   - If a variant has no baseline row (`NotStarted`):
     - If discovered artifacts count is 0: mark `Complete` immediately.
     - If discovered artifacts count > 0: mark `Pending`.
3. **Reconciliation Context**:
   - Pass variant baseline status into `reconcile_usage`.
   - Records without source timestamps during `Pending` become `UndatedBaseline`.
4. **Completion Transition**:
   - Only when full discovery completes, all batches commit, and no collector
     failure occurs does a variant transition from `Pending` to `Complete`.
   - If collection fails or is cancelled, state remains `Pending`.
5. **Candidate Mapping**:
   - `map_daily` and `map_sessions` ignore records with `UndatedBaseline`.

### Diagnostics Counters

Add bounded counters to `AntigravityDiagnosticCounters`:

- `undated_baseline_records: u32`
- `dated_source_reported_records: u32`
- `dated_first_seen_records: u32`
- `baseline_variants_completed: u32`
- `baseline_variants_pending: u32`

## Checklist

- [ ] Bump `PROFILE_VERSION = 3` in `src-tauri/src/infrastructure/collectors/antigravity/mod.rs`.
- [ ] Update `cached_record_from_usage` in `usage_cache.rs` to preserve `Unresolved` for all variants.
- [ ] Wire `ensure_cache_reclassified` into `AntigravityCollector::collect` entry point.
- [ ] Add `baseline_repair_coordinator` to `RefreshExecution` in `execution.rs`.
- [ ] Pass coordinator reference from `RefreshCoordinator` into `RefreshExecution`.
- [ ] Update `planned_collection_request` signature in `request_plan.rs` to accept `repair_coordinator`.
- [ ] Implement fallible scope selection override (`requires_full_scope()?`) in `request_plan.rs`.
- [ ] Update request-planning test fakes and unit tests in `request_plan.rs`.
- [ ] Update `AntigravityCollector::collect` to manage baseline transitions and zero-artifact cases.
- [ ] Update `mapper.rs` (`map_daily`, `map_sessions`) to filter `calendar_attribution == UndatedBaseline`.
- [ ] Add bounded counters to `AntigravityDiagnosticCounters` and include in `antigravity.collection_completed` events.
- [ ] Add adapter tests with sanitized fixtures verifying:
  - Initial baseline scan with App/IDE undated records yields 0 daily tokens.
  - Initial baseline scan with CLI undated records yields 0 daily tokens.
  - Initial baseline scan with source-timestamped records yields dated daily tokens.
  - Zero-artifact variant marks baseline complete immediately.
  - Post-baseline new prompt across any variant yields daily tokens with partial quality.
- [ ] Verify: `cargo test --manifest-path src-tauri/Cargo.toml collectors::antigravity`.

## Test Plan

- **Invariants to Prove**:
  - App and IDE undated records are treated identically to CLI undated records.
  - `undated_baseline` records are excluded from daily and session candidates.
  - Refresh planner enforces `CollectionScope::Full` while repair is in `cache_reclassified`.
- **Commands**:
  - `cargo test --manifest-path src-tauri/Cargo.toml collectors::antigravity`
  - `cargo test --manifest-path src-tauri/Cargo.toml request_plan`
  - `pnpm collectors:fixtures`
  - `pnpm verify:fast`

## Decisions

- **Unify variant handling**: Removing the special-cased `LegacyUnknown` conversion
  for non-CLI variants eliminates the primary leak that allowed App/IDE historical
  records to bypass baseline attribution.
- **Fail-closed scope selection**: `requires_full_scope()?` errors out if the
  repair state cannot be read, preventing accidental incremental planning.
- **Context passing through RefreshExecution**: Storing the coordinator port in
  `RefreshExecution` follows existing patterns for `run_store`, `usage_store`,
  and `clock`.

## Verification

- Queued.

## Follow-Up Debt

- None.
