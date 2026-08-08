//! Refresh execution flow and terminalization.
//!
//! This module owns the side-effect-heavy part of a refresh after the
//! coordinator has accepted a request and spawned the worker thread.

use chrono::DateTime;

use crate::application::collection::{CollectionProjection, CollectionResult, CollectorFailure};
use crate::application::cost::{gap_fill_daily, gap_fill_session, BurnlyCostCalculator};
use crate::application::diagnostics::{
    DiagnosticArea, DiagnosticCode, DiagnosticContext, DiagnosticEvent, DiagnosticSeverity,
    DiagnosticSummary,
};
use crate::application::ports::clock::Clock;
use crate::application::ports::collector::{CancellationSignal, Collector};
use crate::application::ports::diagnostic_recorder::DiagnosticRecorder;
use crate::application::ports::run_store::RunStore;
use crate::application::ports::usage_store::UsageStore;
use crate::application::reconciliation::{
    DailyReconciliationRequest, ImportCollector, ImportOutcome, ImportRunCompletion, ImportRunSpec,
    JobKey, RefreshOutcome, RefreshRunCompletion, RefreshRunId, RefreshRunSpec, RefreshTrigger,
    SessionReconciliationRequest, SourceId,
};

use crate::application::collect_sync::{CommittedDailyTarget, CommittedDailyUpload};
use crate::application::collection::CollectionScope;
use crate::domain::source::SourceKey;

use super::coordinator::BudgetEvaluationRunner;
use super::outcome::{
    clamp_count, run_error, ExecutionFailure, ExecutionResult, RunOutcome, TargetRunAccumulator,
};
use super::request_plan::{planned_collection_request, RefreshScopePolicy};
use super::target::{
    import_timezone, projection_label, records_seen, refresh_targets, RefreshTarget,
};

pub(super) struct RefreshExecution<'a> {
    pub(super) collector: &'a dyn Collector,
    pub(super) run_store: &'a dyn RunStore,
    pub(super) usage_store: &'a dyn UsageStore,
    pub(super) budget_evaluator: &'a dyn BudgetEvaluationRunner,
    pub(super) clock: &'a dyn Clock,
    pub(super) diagnostic_recorder: &'a dyn DiagnosticRecorder,
    pub(super) app_version: &'a str,
    pub(super) aggregation_timezone: String,
}

pub(super) fn execute_refresh(
    context: RefreshExecution<'_>,
    trigger: RefreshTrigger,
    scope_policy: RefreshScopePolicy,
    job_id: &str,
    started_at_ms: i64,
) -> ExecutionResult {
    let job_key = match JobKey::new(job_id) {
        Ok(job_key) => job_key,
        Err(_) => return failed_result(&context, false),
    };
    let spec = match RefreshRunSpec::new(job_key, trigger, context.app_version.to_owned()) {
        Ok(spec) => spec,
        Err(_) => return failed_result(&context, false),
    };
    let refresh_run_id = match context.run_store.begin_refresh_run(spec, started_at_ms) {
        Ok(id) => id,
        Err(_) => return failed_result(&context, false),
    };

    let result = execute_open_refresh(
        &context,
        refresh_run_id,
        scope_policy,
        job_id,
        started_at_ms,
    );
    match result {
        Ok(result) => result,
        Err(failure) => {
            if let Some(import_run_id) = failure.import_run_id {
                let _ = context.run_store.complete_import_run(
                    import_run_id,
                    ImportRunCompletion {
                        outcome: ImportOutcome::Failed,
                        records_seen: failure.records_seen,
                        records_rejected: failure.records_rejected,
                        finished_at_ms: failure.finished_at_ms,
                        error: run_error(failure.code, failure.summary),
                    },
                );
            }
            let _ = context.run_store.complete_refresh_run(
                refresh_run_id,
                RefreshRunCompletion {
                    outcome: RefreshOutcome::Failed,
                    finished_at_ms: failure.finished_at_ms,
                    error: run_error(failure.code, failure.summary),
                },
            );
            ExecutionResult {
                outcome: RunOutcome::Failed,
                finished_at_ms: failure.finished_at_ms,
                usage_changed: failure.usage_changed,
                committed_daily_upload: failure.committed_daily_upload,
            }
        }
    }
}

fn execute_open_refresh(
    context: &RefreshExecution<'_>,
    refresh_run_id: RefreshRunId,
    scope_policy: RefreshScopePolicy,
    job_id: &str,
    started_at_ms: i64,
) -> Result<ExecutionResult, ExecutionFailure> {
    let requested_at = DateTime::from_timestamp_millis(started_at_ms)
        .ok_or_else(|| failure(context, "refresh.time", "Refresh time is invalid."))?;

    let mut aggregate = TargetRunAccumulator::default();
    let mut first_error = None;
    let mut usage_changed = false;
    let mut finished_at_ms = started_at_ms;
    let mut committed_daily_upload = CommittedDailyUpload {
        targets: Vec::new(),
        full_refresh_complete: matches!(scope_policy, RefreshScopePolicy::Full),
    };

    for target in refresh_targets() {
        let source_id = context
            .run_store
            .resolve_source(target.source, started_at_ms)
            .map_err(|_| {
                failure(
                    context,
                    "refresh.source",
                    "Could not resolve the usage source.",
                )
            })?;
        let request = planned_collection_request(
            context.run_store,
            job_id,
            target,
            requested_at,
            &context.aggregation_timezone,
            scope_policy,
        )
        .map_err(|error| failure(context, error.code(), error.summary()))?;
        let collection = match context.collector.collect(request, &NeverCancelled) {
            Ok(collection) => collection,
            Err(failure) => {
                committed_daily_upload.full_refresh_complete = false;
                aggregate.record(RunOutcome::Failed);
                finished_at_ms = context.clock.now_epoch_ms();
                if first_error.is_none() {
                    first_error = run_error(failure.code.code(), failure.to_string());
                }
                // Best-effort attribution: never abort the refresh loop if diagnostics fail.
                record_target_collection_failure(
                    context.diagnostic_recorder,
                    target,
                    &failure,
                    finished_at_ms,
                );
                continue;
            }
        };
        let result = persist(
            context,
            refresh_run_id,
            source_id,
            started_at_ms,
            &collection,
        )?;
        aggregate.record(result.outcome);
        if !matches!(result.outcome, RunOutcome::Succeeded) {
            committed_daily_upload.full_refresh_complete = false;
        }
        usage_changed = usage_changed || result.usage_changed;
        finished_at_ms = result.finished_at_ms;
        if matches!(result.outcome, RunOutcome::Succeeded | RunOutcome::Partial)
            && matches!(collection.projection(), CollectionProjection::Daily)
        {
            if let Some(target_upload) =
                committed_daily_target(target.source, collection.metadata().effective_scope())
            {
                committed_daily_upload.targets.push(target_upload);
            }
        }
    }

    let outcome = aggregate.outcome();
    context
        .run_store
        .complete_refresh_run(
            refresh_run_id,
            RefreshRunCompletion {
                outcome: outcome.refresh_outcome(),
                finished_at_ms,
                error: match outcome {
                    RunOutcome::Succeeded => None,
                    RunOutcome::Partial | RunOutcome::Failed => first_error,
                },
            },
        )
        .map_err(|_| ExecutionFailure {
            import_run_id: None,
            records_seen: 0,
            records_rejected: 0,
            finished_at_ms,
            usage_changed,
            code: "refresh.completion",
            summary: "Could not complete the refresh run.",
            committed_daily_upload: committed_daily_upload.clone(),
        })?;

    Ok(ExecutionResult {
        outcome,
        finished_at_ms,
        usage_changed,
        committed_daily_upload,
    })
}

fn committed_daily_target(
    source: SourceKey,
    scope: &CollectionScope,
) -> Option<CommittedDailyTarget> {
    match scope {
        CollectionScope::Full => Some(CommittedDailyTarget {
            source_key: source.as_str().to_owned(),
            start_date: chrono::NaiveDate::from_ymd_opt(1970, 1, 1)?,
            end_date: chrono::NaiveDate::from_ymd_opt(2099, 12, 31)?,
            full_history: true,
        }),
        CollectionScope::Incremental(range) => Some(CommittedDailyTarget {
            source_key: source.as_str().to_owned(),
            start_date: range.start_date(),
            end_date: range.end_date(),
            full_history: false,
        }),
    }
}

fn persist(
    context: &RefreshExecution<'_>,
    refresh_run_id: RefreshRunId,
    source_id: SourceId,
    now_ms: i64,
    collection: &CollectionResult,
) -> Result<ExecutionResult, ExecutionFailure> {
    let metadata = collection.metadata();
    let import_collector = ImportCollector::new(
        metadata.collector().as_str(),
        metadata.collector_version(),
        metadata.profile_version(),
    )
    .map_err(|_| {
        failure(
            context,
            "refresh.metadata",
            "Collector metadata is invalid.",
        )
    })?;
    let import_spec = ImportRunSpec::new(
        refresh_run_id,
        source_id,
        import_collector,
        collection.projection(),
        metadata.effective_scope().clone(),
        import_timezone(collection.projection(), &context.aggregation_timezone),
    )
    .map_err(|_| failure(context, "refresh.import", "Import metadata is invalid."))?;
    let import_run_id = context
        .run_store
        .begin_import_run(import_spec, now_ms)
        .map_err(|_| failure(context, "refresh.import", "Could not begin the import run."))?;

    let collection_outcome = collection.outcome();
    let records_seen = records_seen(collection);
    let records_rejected = clamp_count(collection.rejection_count());
    reconcile_collection(context, source_id, import_run_id, now_ms, collection).map_err(|_| {
        import_failure(
            context,
            import_run_id,
            records_seen,
            records_rejected,
            false,
            "refresh.reconciliation",
            "Could not reconcile collected usage.",
        )
    })?;

    let outcome = RunOutcome::from_collection(collection_outcome);
    let finished_at_ms = context.clock.now_epoch_ms();

    context
        .run_store
        .complete_import_run(
            import_run_id,
            ImportRunCompletion {
                outcome: outcome.import_outcome(),
                records_seen,
                records_rejected,
                finished_at_ms,
                error: None,
            },
        )
        .map_err(|_| {
            import_failure(
                context,
                import_run_id,
                records_seen,
                records_rejected,
                true,
                "refresh.import_completion",
                "Could not complete the import run.",
            )
        })?;
    Ok(ExecutionResult {
        outcome,
        finished_at_ms,
        usage_changed: true,
        committed_daily_upload: CommittedDailyUpload::default(),
    })
}

fn reconcile_collection(
    context: &RefreshExecution<'_>,
    source_id: SourceId,
    import_run_id: crate::application::reconciliation::ImportRunId,
    now_ms: i64,
    collection: &CollectionResult,
) -> Result<(), crate::application::ports::usage_store::UsageStoreError> {
    // Gap-fill: replace zero-with-positive-tokens costs with Burnly-calculated
    // values from the embedded pricing snapshot, when the snapshot prices the
    // model. Applies uniformly to every collector's candidates.
    let calculator = BurnlyCostCalculator::new();
    match collection.projection() {
        CollectionProjection::Daily => {
            let mut candidates = collection.daily_candidates().to_vec();
            for candidate in candidates.iter_mut() {
                gap_fill_daily(candidate, calculator.snapshot());
            }
            let reconciliation = DailyReconciliationRequest::new(
                source_id,
                import_run_id,
                collection.metadata().effective_scope().clone(),
                collection.outcome(),
                now_ms,
                candidates,
            );
            context.usage_store.reconcile_daily(reconciliation)?;
            let _ = context
                .budget_evaluator
                .evaluate_after_commit(&context.aggregation_timezone, now_ms);
            Ok(())
        }
        CollectionProjection::Session => {
            let mut candidates = collection.session_candidates().to_vec();
            for candidate in candidates.iter_mut() {
                gap_fill_session(candidate, calculator.snapshot());
            }
            let reconciliation = SessionReconciliationRequest::new(
                source_id,
                import_run_id,
                collection.metadata().effective_scope().clone(),
                collection.outcome(),
                now_ms,
                candidates,
            );
            context
                .usage_store
                .reconcile_session(reconciliation)
                .map(|_| ())
        }
    }
}

fn failed_result(context: &RefreshExecution<'_>, usage_changed: bool) -> ExecutionResult {
    ExecutionResult {
        outcome: RunOutcome::Failed,
        finished_at_ms: context.clock.now_epoch_ms(),
        usage_changed,
        committed_daily_upload: CommittedDailyUpload::default(),
    }
}

/// Records which refresh target hard-failed and why, without collector stdout,
/// paths, or session identifiers. Safe keys only: source, projection, failureCode.
fn record_target_collection_failure(
    recorder: &dyn DiagnosticRecorder,
    target: RefreshTarget,
    failure: &CollectorFailure,
    created_at_ms: i64,
) {
    let Ok(code) = DiagnosticCode::new("collection.target_failed") else {
        return;
    };
    let Ok(summary) = DiagnosticSummary::new("Collection failed for one refresh target.") else {
        return;
    };
    // Controlled vocabulary only (source keys, projection labels, failure codes).
    let context_json = format!(
        r#"{{"source":"{}","projection":"{}","failureCode":"{}"}}"#,
        target.source.as_str(),
        projection_label(target.projection),
        failure.code.code()
    );
    let Ok(context) = DiagnosticContext::new(context_json) else {
        return;
    };
    let Ok(event) = DiagnosticEvent::new(
        DiagnosticArea::Collector,
        DiagnosticSeverity::Warning,
        code,
        summary,
        Some(context),
        created_at_ms,
    ) else {
        return;
    };
    recorder.record(event);
}

fn failure(
    context: &RefreshExecution<'_>,
    code: &'static str,
    summary: &'static str,
) -> ExecutionFailure {
    ExecutionFailure {
        import_run_id: None,
        records_seen: 0,
        records_rejected: 0,
        finished_at_ms: context.clock.now_epoch_ms(),
        usage_changed: false,
        code,
        summary,
        committed_daily_upload: CommittedDailyUpload::default(),
    }
}

fn import_failure(
    context: &RefreshExecution<'_>,
    import_run_id: crate::application::reconciliation::ImportRunId,
    records_seen: u32,
    records_rejected: u32,
    usage_changed: bool,
    code: &'static str,
    summary: &'static str,
) -> ExecutionFailure {
    ExecutionFailure {
        import_run_id: Some(import_run_id),
        records_seen,
        records_rejected,
        finished_at_ms: context.clock.now_epoch_ms(),
        usage_changed,
        code,
        summary,
        committed_daily_upload: CommittedDailyUpload::default(),
    }
}

struct NeverCancelled;

impl CancellationSignal for NeverCancelled {
    fn is_cancelled(&self) -> bool {
        false
    }
}
