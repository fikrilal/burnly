//! The single process-wide refresh coordinator.
//!
//! It is the sole owner of refresh concurrency and the only submitter of
//! reconciliation work. A refresh collects `claude-code` daily usage outside any
//! write transaction, then reconciles the result and records run lifecycle. A
//! request that arrives while a refresh is active coalesces into the current run
//! rather than starting a competing job.

#![allow(
    dead_code,
    reason = "Coordinator entry points are invoked by the Phase 4F IPC commands"
)]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use chrono::{DateTime, Utc};

use crate::application::budget_evaluation::{BudgetEvaluationError, BudgetEvaluationService};
use crate::application::budget_notifications::BudgetNotificationService;
use crate::application::collection::{
    CollectionId, CollectionOutcome, CollectionProjection, CollectionRequest, CollectionResult,
    CollectionScope,
};
use crate::application::ports::clock::Clock;
use crate::application::ports::collector::{CancellationSignal, Collector};
use crate::application::ports::run_store::RunStore;
use crate::application::ports::usage_store::UsageStore;
use crate::application::reconciliation::{
    DailyReconciliationRequest, ImportCollector, ImportOutcome, ImportRunCompletion, ImportRunSpec,
    JobKey, RefreshOutcome, RefreshRunCompletion, RefreshRunSpec, RefreshTrigger, RunError,
    SessionReconciliationRequest,
};
use crate::domain::source::SourceKey;

use super::state::{RefreshSnapshot, RefreshStatus};

struct CoordinatorState {
    status: RefreshStatus,
    job_id: Option<String>,
    trigger: Option<RefreshTrigger>,
    last_successful_refresh_at_ms: Option<i64>,
}

impl CoordinatorState {
    fn snapshot(&self) -> RefreshSnapshot {
        RefreshSnapshot {
            status: self.status,
            job_id: self.job_id.clone(),
            trigger: self.trigger,
            last_successful_refresh_at_ms: self.last_successful_refresh_at_ms,
        }
    }
}

pub(crate) trait RefreshEventSink: Send + Sync {
    fn publish(&self, snapshot: RefreshSnapshot, usage_changed: bool);
}

pub(crate) trait BudgetEvaluationRunner: Send + Sync {
    fn evaluate_after_commit(
        &self,
        aggregation_timezone: &str,
        now_epoch_ms: i64,
    ) -> Result<(), BudgetEvaluationError>;
}

struct NoopRefreshEventSink;

impl RefreshEventSink for NoopRefreshEventSink {
    fn publish(&self, _snapshot: RefreshSnapshot, _usage_changed: bool) {}
}

struct NoopBudgetEvaluationRunner;

impl BudgetEvaluationRunner for NoopBudgetEvaluationRunner {
    fn evaluate_after_commit(
        &self,
        _aggregation_timezone: &str,
        _now_epoch_ms: i64,
    ) -> Result<(), BudgetEvaluationError> {
        Ok(())
    }
}

impl BudgetEvaluationRunner for BudgetEvaluationService {
    fn evaluate_after_commit(
        &self,
        aggregation_timezone: &str,
        now_epoch_ms: i64,
    ) -> Result<(), BudgetEvaluationError> {
        self.evaluate(aggregation_timezone, now_epoch_ms)
            .map(|_| ())
    }
}

impl BudgetEvaluationRunner for BudgetNotificationService {
    fn evaluate_after_commit(
        &self,
        aggregation_timezone: &str,
        now_epoch_ms: i64,
    ) -> Result<(), BudgetEvaluationError> {
        self.evaluate_and_deliver(aggregation_timezone, now_epoch_ms)
    }
}

pub(crate) struct RefreshCoordinatorHooks {
    event_sink: Arc<dyn RefreshEventSink>,
    budget_evaluator: Arc<dyn BudgetEvaluationRunner>,
}

impl RefreshCoordinatorHooks {
    pub(crate) fn new(
        event_sink: Arc<dyn RefreshEventSink>,
        budget_evaluator: Arc<dyn BudgetEvaluationRunner>,
    ) -> Self {
        Self {
            event_sink,
            budget_evaluator,
        }
    }
}

#[derive(Clone)]
pub(crate) struct RefreshCoordinator {
    collector: Arc<dyn Collector>,
    run_store: Arc<dyn RunStore>,
    usage_store: Arc<dyn UsageStore>,
    budget_evaluator: Arc<dyn BudgetEvaluationRunner>,
    clock: Arc<dyn Clock>,
    event_sink: Arc<dyn RefreshEventSink>,
    app_version: String,
    aggregation_timezone: Arc<Mutex<String>>,
    sequence: Arc<AtomicU64>,
    state: Arc<Mutex<CoordinatorState>>,
}

impl RefreshCoordinator {
    pub(crate) fn new(
        collector: Arc<dyn Collector>,
        run_store: Arc<dyn RunStore>,
        usage_store: Arc<dyn UsageStore>,
        clock: Arc<dyn Clock>,
        app_version: impl Into<String>,
        aggregation_timezone: impl Into<String>,
    ) -> Self {
        Self::with_event_sink(
            collector,
            run_store,
            usage_store,
            clock,
            Arc::new(NoopRefreshEventSink),
            app_version,
            aggregation_timezone,
        )
    }

    pub(crate) fn with_event_sink(
        collector: Arc<dyn Collector>,
        run_store: Arc<dyn RunStore>,
        usage_store: Arc<dyn UsageStore>,
        clock: Arc<dyn Clock>,
        event_sink: Arc<dyn RefreshEventSink>,
        app_version: impl Into<String>,
        aggregation_timezone: impl Into<String>,
    ) -> Self {
        Self::with_event_sink_and_budget_evaluator(
            collector,
            run_store,
            usage_store,
            clock,
            RefreshCoordinatorHooks::new(event_sink, Arc::new(NoopBudgetEvaluationRunner)),
            app_version,
            aggregation_timezone,
        )
    }

    pub(crate) fn with_event_sink_and_budget_evaluator(
        collector: Arc<dyn Collector>,
        run_store: Arc<dyn RunStore>,
        usage_store: Arc<dyn UsageStore>,
        clock: Arc<dyn Clock>,
        hooks: RefreshCoordinatorHooks,
        app_version: impl Into<String>,
        aggregation_timezone: impl Into<String>,
    ) -> Self {
        Self {
            collector,
            run_store,
            usage_store,
            budget_evaluator: hooks.budget_evaluator,
            clock,
            event_sink: hooks.event_sink,
            app_version: app_version.into(),
            aggregation_timezone: Arc::new(Mutex::new(aggregation_timezone.into())),
            sequence: Arc::new(AtomicU64::new(0)),
            state: Arc::new(Mutex::new(CoordinatorState {
                status: RefreshStatus::Idle,
                job_id: None,
                trigger: None,
                last_successful_refresh_at_ms: None,
            })),
        }
    }

    pub(crate) fn snapshot(&self) -> RefreshSnapshot {
        self.lock_state().snapshot()
    }

    pub(crate) fn set_aggregation_timezone(&self, timezone: impl Into<String>) {
        *self
            .aggregation_timezone
            .lock()
            .expect("aggregation timezone lock is poisoned") = timezone.into();
    }

    fn aggregation_timezone(&self) -> String {
        self.aggregation_timezone
            .lock()
            .expect("aggregation timezone lock is poisoned")
            .clone()
    }

    /// Skeleton cancellation: moves an active run toward `cancelling`. Cooperative
    /// interruption of the running collector is completed in Phase 7.
    pub(crate) fn cancel(&self) -> RefreshSnapshot {
        let mut state = self.lock_state();
        if state.status.is_active() {
            state.status = RefreshStatus::Cancelling;
        }
        state.snapshot()
    }

    pub(crate) fn request_refresh(&self, trigger: RefreshTrigger) -> RefreshSnapshot {
        let now_ms = self.clock.now_epoch_ms();
        let job_id = self.next_job_id(now_ms);

        let snapshot = {
            let mut state = self.lock_state();
            if state.status.is_active() {
                return state.snapshot();
            }
            state.status = RefreshStatus::Running;
            state.job_id = Some(job_id.clone());
            state.trigger = Some(trigger);
            state.snapshot()
        };
        self.event_sink.publish(snapshot.clone(), false);

        let worker = self.clone();
        if thread::Builder::new()
            .name("burnly-refresh".to_owned())
            .spawn(move || worker.finish_refresh(trigger, job_id, now_ms))
            .is_err()
        {
            let failed = {
                let mut state = self.lock_state();
                state.status = RefreshStatus::Failed;
                state.snapshot()
            };
            self.event_sink.publish(failed.clone(), false);
            return failed;
        }
        snapshot
    }

    fn lock_state(&self) -> std::sync::MutexGuard<'_, CoordinatorState> {
        self.state.lock().expect("refresh state lock is poisoned")
    }

    fn next_job_id(&self, now_ms: i64) -> String {
        let sequence = self.sequence.fetch_add(1, Ordering::Relaxed);
        format!("refresh-{now_ms}-{sequence}")
    }

    fn finish_refresh(&self, trigger: RefreshTrigger, job_id: String, started_at_ms: i64) {
        let result = self.execute(trigger, &job_id, started_at_ms);
        let snapshot = {
            let mut state = self.lock_state();
            state.status = result.outcome.status();
            if matches!(result.outcome, RunOutcome::Succeeded) {
                state.last_successful_refresh_at_ms = Some(result.finished_at_ms);
            }
            state.snapshot()
        };
        self.event_sink.publish(snapshot, result.usage_changed);
    }

    fn execute(
        &self,
        trigger: RefreshTrigger,
        job_id: &str,
        started_at_ms: i64,
    ) -> ExecutionResult {
        let job_key = match JobKey::new(job_id) {
            Ok(job_key) => job_key,
            Err(_) => return self.failed_result(false),
        };
        let spec = match RefreshRunSpec::new(job_key, trigger, self.app_version.clone()) {
            Ok(spec) => spec,
            Err(_) => return self.failed_result(false),
        };
        let refresh_run_id = match self.run_store.begin_refresh_run(spec, started_at_ms) {
            Ok(id) => id,
            Err(_) => return self.failed_result(false),
        };

        let result = self.execute_open_refresh(refresh_run_id, job_id, started_at_ms);
        match result {
            Ok(result) => result,
            Err(failure) => {
                if let Some(import_run_id) = failure.import_run_id {
                    let _ = self.run_store.complete_import_run(
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
                let _ = self.run_store.complete_refresh_run(
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
                }
            }
        }
    }

    fn execute_open_refresh(
        &self,
        refresh_run_id: crate::application::reconciliation::RefreshRunId,
        job_id: &str,
        started_at_ms: i64,
    ) -> Result<ExecutionResult, ExecutionFailure> {
        let requested_at = DateTime::from_timestamp_millis(started_at_ms)
            .ok_or_else(|| self.failure("refresh.time", "Refresh time is invalid."))?;

        let mut aggregate = RunOutcome::Succeeded;
        let mut usage_changed = false;
        let mut finished_at_ms = started_at_ms;
        let aggregation_timezone = self.aggregation_timezone();

        for target in refresh_targets() {
            let source_id = self
                .run_store
                .resolve_source(target.source, started_at_ms)
                .map_err(|_| {
                    self.failure("refresh.source", "Could not resolve the usage source.")
                })?;
            let request =
                self.collection_request(job_id, target, requested_at, &aggregation_timezone)?;
            let collection =
                self.collector
                    .collect(request, &NeverCancelled)
                    .map_err(|failure| {
                        self.failure(
                            failure.code.code(),
                            "The collector could not complete the refresh.",
                        )
                    })?;
            let result = self.persist(
                refresh_run_id,
                source_id,
                started_at_ms,
                &aggregation_timezone,
                &collection,
            )?;
            aggregate = aggregate.combine(result.outcome);
            usage_changed = usage_changed || result.usage_changed;
            finished_at_ms = result.finished_at_ms;
        }

        self.run_store
            .complete_refresh_run(
                refresh_run_id,
                RefreshRunCompletion {
                    outcome: aggregate.refresh_outcome(),
                    finished_at_ms,
                    error: None,
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
            })?;

        Ok(ExecutionResult {
            outcome: aggregate,
            finished_at_ms,
            usage_changed,
        })
    }

    fn collection_request(
        &self,
        job_id: &str,
        target: RefreshTarget,
        requested_at: DateTime<Utc>,
        aggregation_timezone: &str,
    ) -> Result<CollectionRequest, ExecutionFailure> {
        let collection_id = CollectionId::new(format!(
            "{job_id}:{}:{}",
            target.source.as_str(),
            projection_label(target.projection)
        ))
        .map_err(|_| self.failure("refresh.request", "Refresh request is invalid."))?;

        match target.projection {
            CollectionProjection::Daily => CollectionRequest::daily(
                collection_id,
                target.source,
                CollectionScope::Full,
                aggregation_timezone.to_owned(),
                requested_at,
            )
            .map_err(|_| self.failure("refresh.request", "Refresh request is invalid.")),
            CollectionProjection::Session => Ok(CollectionRequest::session(
                collection_id,
                target.source,
                CollectionScope::Full,
                requested_at,
            )),
        }
    }

    fn persist(
        &self,
        refresh_run_id: crate::application::reconciliation::RefreshRunId,
        source_id: crate::application::reconciliation::SourceId,
        now_ms: i64,
        aggregation_timezone: &str,
        collection: &CollectionResult,
    ) -> Result<ExecutionResult, ExecutionFailure> {
        let metadata = collection.metadata();
        let import_collector = ImportCollector::new(
            metadata.collector().as_str(),
            metadata.collector_version(),
            metadata.profile_version(),
        )
        .map_err(|_| self.failure("refresh.metadata", "Collector metadata is invalid."))?;
        let import_spec = ImportRunSpec::new(
            refresh_run_id,
            source_id,
            import_collector,
            collection.projection(),
            CollectionScope::Full,
            import_timezone(collection.projection(), aggregation_timezone),
        )
        .map_err(|_| self.failure("refresh.import", "Import metadata is invalid."))?;
        let import_run_id = self
            .run_store
            .begin_import_run(import_spec, now_ms)
            .map_err(|_| self.failure("refresh.import", "Could not begin the import run."))?;

        let collection_outcome = collection.outcome();
        let records_seen = records_seen(collection);
        let records_rejected = clamp_count(collection.rejection_count());
        self.reconcile_collection(
            source_id,
            import_run_id,
            now_ms,
            aggregation_timezone,
            collection,
        )
        .map_err(|_| {
            self.import_failure(
                import_run_id,
                records_seen,
                records_rejected,
                false,
                "refresh.reconciliation",
                "Could not reconcile collected usage.",
            )
        })?;

        let outcome = RunOutcome::from_collection(collection_outcome);
        let finished_at_ms = self.clock.now_epoch_ms();

        self.run_store
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
                self.import_failure(
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
        })
    }

    fn reconcile_collection(
        &self,
        source_id: crate::application::reconciliation::SourceId,
        import_run_id: crate::application::reconciliation::ImportRunId,
        now_ms: i64,
        aggregation_timezone: &str,
        collection: &CollectionResult,
    ) -> Result<(), crate::application::ports::usage_store::UsageStoreError> {
        match collection.projection() {
            CollectionProjection::Daily => {
                let reconciliation = DailyReconciliationRequest::new(
                    source_id,
                    import_run_id,
                    CollectionScope::Full,
                    collection.outcome(),
                    now_ms,
                    collection.daily_candidates().to_vec(),
                );
                self.usage_store.reconcile_daily(reconciliation)?;
                let _ = self
                    .budget_evaluator
                    .evaluate_after_commit(aggregation_timezone, now_ms);
                Ok(())
            }
            CollectionProjection::Session => {
                let reconciliation = SessionReconciliationRequest::new(
                    source_id,
                    import_run_id,
                    CollectionScope::Full,
                    collection.outcome(),
                    now_ms,
                    collection.session_candidates().to_vec(),
                );
                self.usage_store
                    .reconcile_session(reconciliation)
                    .map(|_| ())
            }
        }
    }

    fn failed_result(&self, usage_changed: bool) -> ExecutionResult {
        ExecutionResult {
            outcome: RunOutcome::Failed,
            finished_at_ms: self.clock.now_epoch_ms(),
            usage_changed,
        }
    }

    fn failure(&self, code: &'static str, summary: &'static str) -> ExecutionFailure {
        ExecutionFailure {
            import_run_id: None,
            records_seen: 0,
            records_rejected: 0,
            finished_at_ms: self.clock.now_epoch_ms(),
            usage_changed: false,
            code,
            summary,
        }
    }

    fn import_failure(
        &self,
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
            finished_at_ms: self.clock.now_epoch_ms(),
            usage_changed,
            code,
            summary,
        }
    }
}

struct NeverCancelled;

impl CancellationSignal for NeverCancelled {
    fn is_cancelled(&self) -> bool {
        false
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunOutcome {
    Succeeded,
    Partial,
    Failed,
}

impl RunOutcome {
    fn from_collection(outcome: CollectionOutcome) -> Self {
        match outcome {
            CollectionOutcome::Partial => Self::Partial,
            CollectionOutcome::Complete | CollectionOutcome::Empty => Self::Succeeded,
        }
    }

    const fn status(self) -> RefreshStatus {
        match self {
            Self::Succeeded => RefreshStatus::Succeeded,
            Self::Partial => RefreshStatus::Partial,
            Self::Failed => RefreshStatus::Failed,
        }
    }

    const fn refresh_outcome(self) -> RefreshOutcome {
        match self {
            Self::Succeeded => RefreshOutcome::Succeeded,
            Self::Partial => RefreshOutcome::Partial,
            Self::Failed => RefreshOutcome::Failed,
        }
    }

    const fn import_outcome(self) -> ImportOutcome {
        match self {
            Self::Succeeded => ImportOutcome::Succeeded,
            Self::Partial => ImportOutcome::Partial,
            Self::Failed => ImportOutcome::Failed,
        }
    }

    const fn combine(self, next: Self) -> Self {
        match (self, next) {
            (Self::Failed, _) | (_, Self::Failed) => Self::Failed,
            (Self::Partial, _) | (_, Self::Partial) => Self::Partial,
            (Self::Succeeded, Self::Succeeded) => Self::Succeeded,
        }
    }
}

struct ExecutionResult {
    outcome: RunOutcome,
    finished_at_ms: i64,
    usage_changed: bool,
}

struct ExecutionFailure {
    import_run_id: Option<crate::application::reconciliation::ImportRunId>,
    records_seen: u32,
    records_rejected: u32,
    finished_at_ms: i64,
    usage_changed: bool,
    code: &'static str,
    summary: &'static str,
}

fn run_error(code: &'static str, summary: &'static str) -> Option<RunError> {
    RunError::new(code, summary).ok()
}

fn clamp_count(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

#[derive(Debug, Clone, Copy)]
struct RefreshTarget {
    source: SourceKey,
    projection: CollectionProjection,
}

const fn refresh_targets() -> [RefreshTarget; 6] {
    [
        RefreshTarget {
            source: SourceKey::ClaudeCode,
            projection: CollectionProjection::Daily,
        },
        RefreshTarget {
            source: SourceKey::ClaudeCode,
            projection: CollectionProjection::Session,
        },
        RefreshTarget {
            source: SourceKey::Codex,
            projection: CollectionProjection::Daily,
        },
        RefreshTarget {
            source: SourceKey::Codex,
            projection: CollectionProjection::Session,
        },
        RefreshTarget {
            source: SourceKey::OpenCode,
            projection: CollectionProjection::Daily,
        },
        RefreshTarget {
            source: SourceKey::OpenCode,
            projection: CollectionProjection::Session,
        },
    ]
}

const fn projection_label(projection: CollectionProjection) -> &'static str {
    match projection {
        CollectionProjection::Daily => "daily",
        CollectionProjection::Session => "session",
    }
}

fn import_timezone(projection: CollectionProjection, timezone: &str) -> Option<String> {
    match projection {
        CollectionProjection::Daily => Some(timezone.to_owned()),
        CollectionProjection::Session => None,
    }
}

fn records_seen(collection: &CollectionResult) -> u32 {
    let count = match collection.projection() {
        CollectionProjection::Daily => collection.daily_candidates().len(),
        CollectionProjection::Session => collection.session_candidates().len(),
    };
    clamp_count(count)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, AtomicI64, AtomicUsize, Ordering};
    use std::sync::{Arc, Condvar, Mutex};
    use std::thread;
    use std::time::Duration;

    use chrono::{TimeZone, Utc};

    use super::*;
    use crate::application::collection::{
        CandidateProvenance, CollectionMetadata, CollectionPeriod, CollectorDescriptor,
        CollectorFailure, CollectorFailureCode, DailyUsageCandidate, DetectionRequest,
        DetectionResult, ProcessSummary, RejectedRecord, SessionUsageCandidate,
    };
    use crate::application::ports::run_store::RunStoreError;
    use crate::application::ports::usage_store::UsageStoreError;
    use crate::application::reconciliation::{
        DailyReconciliationSummary, ImportRunId, RefreshRunId, SourceId,
    };
    use crate::domain::usage::{
        CostKind, CurrencyCode, DataQuality, TokenUsage, UsageCost, ValuedCostStatus,
    };

    struct FakeRunStore {
        refresh_outcomes: Mutex<Vec<RefreshOutcome>>,
        import_outcomes: Mutex<Vec<ImportOutcome>>,
        failure: Mutex<Option<RunStoreFailure>>,
        next_id: AtomicUsize,
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum RunStoreFailure {
        ResolveSource,
        BeginImport,
        CompleteImport,
        CompleteRefresh,
    }

    impl FakeRunStore {
        fn new() -> Self {
            Self {
                refresh_outcomes: Mutex::new(Vec::new()),
                import_outcomes: Mutex::new(Vec::new()),
                failure: Mutex::new(None),
                next_id: AtomicUsize::new(1),
            }
        }

        fn fail_once(&self, failure: RunStoreFailure) {
            *self.failure.lock().expect("lock") = Some(failure);
        }

        fn take_failure(&self, expected: RunStoreFailure) -> bool {
            let mut failure = self.failure.lock().expect("lock");
            if *failure == Some(expected) {
                *failure = None;
                true
            } else {
                false
            }
        }

        fn next(&self) -> i64 {
            self.next_id.fetch_add(1, Ordering::Relaxed) as i64
        }

        fn refresh_outcomes(&self) -> Vec<RefreshOutcome> {
            self.refresh_outcomes.lock().expect("lock").clone()
        }

        fn import_outcomes(&self) -> Vec<ImportOutcome> {
            self.import_outcomes.lock().expect("lock").clone()
        }
    }

    impl RunStore for FakeRunStore {
        fn resolve_source(
            &self,
            _source: SourceKey,
            _now_ms: i64,
        ) -> Result<SourceId, RunStoreError> {
            if self.take_failure(RunStoreFailure::ResolveSource) {
                return Err(RunStoreError::Backend);
            }
            Ok(SourceId::new(1))
        }

        fn begin_refresh_run(
            &self,
            _spec: RefreshRunSpec,
            _now_ms: i64,
        ) -> Result<RefreshRunId, RunStoreError> {
            Ok(RefreshRunId::new(self.next()))
        }

        fn complete_refresh_run(
            &self,
            _id: RefreshRunId,
            completion: RefreshRunCompletion,
        ) -> Result<(), RunStoreError> {
            if self.take_failure(RunStoreFailure::CompleteRefresh) {
                return Err(RunStoreError::Backend);
            }
            self.refresh_outcomes
                .lock()
                .expect("lock")
                .push(completion.outcome);
            Ok(())
        }

        fn begin_import_run(
            &self,
            _spec: ImportRunSpec,
            _now_ms: i64,
        ) -> Result<ImportRunId, RunStoreError> {
            if self.take_failure(RunStoreFailure::BeginImport) {
                return Err(RunStoreError::Backend);
            }
            Ok(ImportRunId::new(self.next()))
        }

        fn complete_import_run(
            &self,
            _id: ImportRunId,
            completion: ImportRunCompletion,
        ) -> Result<(), RunStoreError> {
            if self.take_failure(RunStoreFailure::CompleteImport) {
                return Err(RunStoreError::Backend);
            }
            self.import_outcomes
                .lock()
                .expect("lock")
                .push(completion.outcome);
            Ok(())
        }
    }

    struct FakeUsageStore {
        reconciled: Mutex<Vec<(CollectionProjection, CollectionOutcome)>>,
        fail: AtomicBool,
    }

    struct RecordingEventSink {
        events: Mutex<Vec<(RefreshStatus, bool)>>,
    }

    impl RecordingEventSink {
        fn new() -> Self {
            Self {
                events: Mutex::new(Vec::new()),
            }
        }

        fn events(&self) -> Vec<(RefreshStatus, bool)> {
            self.events.lock().expect("lock").clone()
        }
    }

    impl RefreshEventSink for RecordingEventSink {
        fn publish(&self, snapshot: RefreshSnapshot, usage_changed: bool) {
            self.events
                .lock()
                .expect("lock")
                .push((snapshot.status, usage_changed));
        }
    }

    struct RecordingBudgetEvaluator {
        calls: Mutex<Vec<(String, i64)>>,
        fail: AtomicBool,
    }

    impl RecordingBudgetEvaluator {
        fn new() -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                fail: AtomicBool::new(false),
            }
        }

        fn fail(&self) {
            self.fail.store(true, Ordering::Release);
        }

        fn calls(&self) -> Vec<(String, i64)> {
            self.calls.lock().expect("lock").clone()
        }
    }

    impl BudgetEvaluationRunner for RecordingBudgetEvaluator {
        fn evaluate_after_commit(
            &self,
            aggregation_timezone: &str,
            now_epoch_ms: i64,
        ) -> Result<(), BudgetEvaluationError> {
            self.calls
                .lock()
                .expect("lock")
                .push((aggregation_timezone.to_owned(), now_epoch_ms));
            if self.fail.load(Ordering::Acquire) {
                return Err(BudgetEvaluationError::StorageUnavailable);
            }
            Ok(())
        }
    }

    impl FakeUsageStore {
        fn new() -> Self {
            Self {
                reconciled: Mutex::new(Vec::new()),
                fail: AtomicBool::new(false),
            }
        }

        fn fail(&self) {
            self.fail.store(true, Ordering::Release);
        }

        fn reconciled_outcomes(&self) -> Vec<CollectionOutcome> {
            self.reconciled
                .lock()
                .expect("lock")
                .iter()
                .map(|(_, outcome)| *outcome)
                .collect()
        }

        fn reconciled_projections(&self) -> Vec<CollectionProjection> {
            self.reconciled
                .lock()
                .expect("lock")
                .iter()
                .map(|(projection, _)| *projection)
                .collect()
        }
    }

    impl UsageStore for FakeUsageStore {
        fn reconcile_daily(
            &self,
            request: DailyReconciliationRequest,
        ) -> Result<DailyReconciliationSummary, UsageStoreError> {
            if self.fail.swap(false, Ordering::AcqRel) {
                return Err(UsageStoreError::Backend);
            }
            self.reconciled
                .lock()
                .expect("lock")
                .push((CollectionProjection::Daily, request.outcome()));
            let observed = request
                .candidates()
                .iter()
                .map(|candidate| candidate.source_key.clone())
                .collect::<Vec<_>>();
            let upserted = u32::try_from(observed.len()).unwrap_or(u32::MAX);
            Ok(DailyReconciliationSummary::new(upserted, observed))
        }

        fn reconcile_session(
            &self,
            request: crate::application::reconciliation::SessionReconciliationRequest,
        ) -> Result<crate::application::reconciliation::SessionReconciliationSummary, UsageStoreError>
        {
            if self.fail.swap(false, Ordering::AcqRel) {
                return Err(UsageStoreError::Backend);
            }
            self.reconciled
                .lock()
                .expect("lock")
                .push((CollectionProjection::Session, request.outcome()));
            let observed = request
                .candidates()
                .iter()
                .map(|candidate| candidate.source_key.clone())
                .collect::<Vec<_>>();
            let upserted = u32::try_from(observed.len()).unwrap_or(u32::MAX);
            Ok(
                crate::application::reconciliation::SessionReconciliationSummary::new(
                    upserted, observed,
                ),
            )
        }
    }

    struct FakeClock {
        now_ms: i64,
    }

    impl Clock for FakeClock {
        fn now_epoch_ms(&self) -> i64 {
            self.now_ms
        }
    }

    struct AdvancingClock(AtomicI64);

    impl Clock for AdvancingClock {
        fn now_epoch_ms(&self) -> i64 {
            self.0.fetch_add(100, Ordering::Relaxed)
        }
    }

    fn timestamp(hour: u32) -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 6, 14, hour, 0, 0)
            .single()
            .expect("timestamp")
    }

    fn test_metadata(request: &CollectionRequest) -> CollectionMetadata {
        CollectionMetadata::new(
            request.collection_id().clone(),
            crate::application::collection::CollectorKey::new("fixture-collector")
                .expect("collector key"),
            "20.0.11".to_owned(),
            request.source(),
            request.scope().clone(),
            1,
            CollectionPeriod {
                started_at: timestamp(7),
                finished_at: timestamp(8),
            },
        )
        .expect("metadata")
    }

    fn tokens() -> TokenUsage {
        TokenUsage::new(Some(100), Some(0), Some(0), Some(0), 100).expect("tokens")
    }

    fn cost() -> UsageCost {
        UsageCost::Valued {
            amount_micros: 10_000,
            currency: CurrencyCode::new("USD").expect("currency"),
            kind: CostKind::CollectorCalculated,
            status: ValuedCostStatus::Estimated,
        }
    }

    fn provenance(request: &CollectionRequest) -> CandidateProvenance {
        CandidateProvenance {
            source: request.source(),
            collector: crate::application::collection::CollectorKey::new("fixture-collector")
                .expect("collector key"),
            collector_version: "20.0.11".to_owned(),
            profile_version: 1,
            collection_id: request.collection_id().clone(),
            observed_at: timestamp(8),
            data_quality: DataQuality::Complete,
            warnings: Vec::new(),
        }
    }

    fn daily_candidate(request: &CollectionRequest) -> DailyUsageCandidate {
        let aggregation_timezone = request.aggregation_timezone().unwrap_or("UTC").to_owned();
        DailyUsageCandidate {
            provenance: provenance(request),
            source_key: format!(
                "{}:daily:v1:{}:2026-06-13",
                request.source().as_str(),
                aggregation_timezone
            ),
            usage_date: chrono::NaiveDate::from_ymd_opt(2026, 6, 13).expect("date"),
            aggregation_timezone,
            tokens: tokens(),
            cost: cost(),
            model_breakdowns: Vec::new(),
        }
    }

    fn session_candidate(request: &CollectionRequest) -> SessionUsageCandidate {
        SessionUsageCandidate {
            provenance: provenance(request),
            source_key: format!("{}:session:v1:session-1", request.source().as_str()),
            source_session_id: "session-1".to_owned(),
            project_path: Some("/tmp/project".to_owned()),
            first_activity_at: Some(timestamp(7)),
            last_activity_at: Some(timestamp(8)),
            tokens: tokens(),
            cost: cost(),
            model_breakdowns: Vec::new(),
        }
    }

    fn rejected_record() -> RejectedRecord {
        RejectedRecord {
            code: "record.invalid".to_owned(),
            record_index: Some(0),
        }
    }

    fn collection_for_request(
        request: &CollectionRequest,
        rejections: Vec<RejectedRecord>,
    ) -> CollectionResult {
        match request.projection() {
            CollectionProjection::Daily => CollectionResult::daily(
                test_metadata(request),
                vec![daily_candidate(request)],
                rejections,
                Vec::new(),
                process_summary(),
            )
            .expect("collection result"),
            CollectionProjection::Session => CollectionResult::session(
                test_metadata(request),
                vec![session_candidate(request)],
                rejections,
                Vec::new(),
                process_summary(),
            )
            .expect("collection result"),
        }
    }

    fn empty_collection_for_request(request: &CollectionRequest) -> CollectionResult {
        match request.projection() {
            CollectionProjection::Daily => CollectionResult::daily(
                test_metadata(request),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                process_summary(),
            )
            .expect("collection result"),
            CollectionProjection::Session => CollectionResult::session(
                test_metadata(request),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                process_summary(),
            )
            .expect("collection result"),
        }
    }

    fn legacy_collection() -> CollectionResult {
        let request = CollectionRequest::daily(
            CollectionId::new("job-1").expect("collection id"),
            SourceKey::ClaudeCode,
            CollectionScope::Full,
            "UTC",
            timestamp(8),
        )
        .expect("request");
        let tokens = TokenUsage::new(Some(100), Some(0), Some(0), Some(0), 100).expect("tokens");
        let cost = UsageCost::Valued {
            amount_micros: 10_000,
            currency: CurrencyCode::new("USD").expect("currency"),
            kind: CostKind::CollectorCalculated,
            status: ValuedCostStatus::Estimated,
        };
        CollectionResult::daily(
            test_metadata(&request),
            vec![DailyUsageCandidate {
                provenance: provenance(&request),
                source_key: "claude-code:daily:v1:UTC:2026-06-13".to_owned(),
                usage_date: chrono::NaiveDate::from_ymd_opt(2026, 6, 13).expect("date"),
                aggregation_timezone: "UTC".to_owned(),
                tokens,
                cost,
                model_breakdowns: Vec::new(),
            }],
            Vec::new(),
            Vec::new(),
            process_summary(),
        )
        .expect("collection result")
    }

    fn expected_refresh_targets() -> Vec<(SourceKey, CollectionProjection)> {
        refresh_targets()
            .iter()
            .map(|target| (target.source, target.projection))
            .collect()
    }

    fn expected_refresh_projections() -> Vec<CollectionProjection> {
        refresh_targets()
            .iter()
            .map(|target| target.projection)
            .collect()
    }

    fn repeated_import_outcomes(outcome: ImportOutcome) -> Vec<ImportOutcome> {
        vec![outcome; refresh_targets().len()]
    }

    fn repeated_collection_outcomes(outcome: CollectionOutcome) -> Vec<CollectionOutcome> {
        vec![outcome; refresh_targets().len()]
    }

    fn repeated_refresh_outcomes(outcome: RefreshOutcome) -> Vec<RefreshOutcome> {
        vec![outcome]
    }

    #[allow(dead_code)]
    fn candidate() -> DailyUsageCandidate {
        let request = CollectionRequest::daily(
            CollectionId::new("job-1").expect("collection id"),
            SourceKey::ClaudeCode,
            CollectionScope::Full,
            "UTC",
            timestamp(8),
        )
        .expect("request");
        DailyUsageCandidate {
            provenance: provenance(&request),
            source_key: "claude-code:daily:v1:UTC:2026-06-13".to_owned(),
            usage_date: chrono::NaiveDate::from_ymd_opt(2026, 6, 13).expect("date"),
            aggregation_timezone: "UTC".to_owned(),
            tokens: tokens(),
            cost: cost(),
            model_breakdowns: Vec::new(),
        }
    }

    fn process_summary() -> ProcessSummary {
        ProcessSummary {
            runtime_ms: 1,
            stdout_bytes: 1,
            stderr_bytes: 0,
            exit_code: Some(0),
        }
    }

    struct ScriptedCollector {
        behavior: Box<
            dyn Fn(CollectionRequest) -> Result<CollectionResult, CollectorFailure> + Send + Sync,
        >,
        calls: AtomicUsize,
        requests: Mutex<Vec<(SourceKey, CollectionProjection)>>,
    }

    impl ScriptedCollector {
        fn new(
            behavior: impl Fn(CollectionRequest) -> Result<CollectionResult, CollectorFailure>
                + Send
                + Sync
                + 'static,
        ) -> Self {
            Self {
                behavior: Box::new(behavior),
                calls: AtomicUsize::new(0),
                requests: Mutex::new(Vec::new()),
            }
        }

        fn calls(&self) -> usize {
            self.calls.load(Ordering::Relaxed)
        }

        fn requests(&self) -> Vec<(SourceKey, CollectionProjection)> {
            self.requests.lock().expect("lock").clone()
        }
    }

    impl Collector for ScriptedCollector {
        fn describe(&self) -> Result<CollectorDescriptor, CollectorFailure> {
            unimplemented!("the coordinator does not describe the collector")
        }

        fn detect(
            &self,
            _request: DetectionRequest,
            _cancellation: &dyn CancellationSignal,
        ) -> Result<DetectionResult, CollectorFailure> {
            unimplemented!("the coordinator does not detect through the collector")
        }

        fn collect(
            &self,
            request: CollectionRequest,
            _cancellation: &dyn CancellationSignal,
        ) -> Result<CollectionResult, CollectorFailure> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            self.requests
                .lock()
                .expect("lock")
                .push((request.source(), request.projection()));
            (self.behavior)(request)
        }
    }

    fn coordinator_with(
        collector: Arc<dyn Collector>,
    ) -> (RefreshCoordinator, Arc<FakeRunStore>, Arc<FakeUsageStore>) {
        let run_store = Arc::new(FakeRunStore::new());
        let usage_store = Arc::new(FakeUsageStore::new());
        let clock = Arc::new(FakeClock { now_ms: 1_000 });
        let coordinator = RefreshCoordinator::new(
            collector,
            run_store.clone(),
            usage_store.clone(),
            clock,
            "0.1.0",
            "UTC",
        );

        (coordinator, run_store, usage_store)
    }

    fn await_terminal(coordinator: &RefreshCoordinator) -> RefreshSnapshot {
        for _ in 0..1_000 {
            let snapshot = coordinator.snapshot();
            if !snapshot.status.is_active() {
                return snapshot;
            }
            thread::sleep(Duration::from_millis(1));
        }
        panic!("refresh did not reach a terminal state");
    }

    #[test]
    fn complete_collection_reconciles_and_succeeds() {
        let collector = Arc::new(ScriptedCollector::new(|request| {
            Ok(collection_for_request(&request, Vec::new()))
        }));
        let (coordinator, run_store, usage_store) = coordinator_with(collector.clone());

        let submitted = coordinator.request_refresh(RefreshTrigger::Manual);
        assert_eq!(submitted.status, RefreshStatus::Running);
        let snapshot = await_terminal(&coordinator);

        assert_eq!(snapshot.status, RefreshStatus::Succeeded);
        assert_eq!(snapshot.last_successful_refresh_at_ms, Some(1_000));
        assert_eq!(collector.calls(), refresh_targets().len());
        assert_eq!(collector.requests(), expected_refresh_targets());
        assert_eq!(
            usage_store.reconciled_projections(),
            expected_refresh_projections()
        );
        assert_eq!(
            usage_store.reconciled_outcomes(),
            repeated_collection_outcomes(CollectionOutcome::Complete)
        );
        assert_eq!(
            run_store.import_outcomes(),
            repeated_import_outcomes(ImportOutcome::Succeeded)
        );
        assert_eq!(
            run_store.refresh_outcomes(),
            repeated_refresh_outcomes(RefreshOutcome::Succeeded)
        );
    }

    #[test]
    fn successful_refresh_records_completion_time() {
        let collector = Arc::new(ScriptedCollector::new(|request| {
            Ok(empty_collection_for_request(&request))
        }));
        let run_store = Arc::new(FakeRunStore::new());
        let usage_store = Arc::new(FakeUsageStore::new());
        let coordinator = RefreshCoordinator::new(
            collector,
            run_store,
            usage_store,
            Arc::new(AdvancingClock(AtomicI64::new(1_000))),
            "0.1.0",
            "UTC",
        );

        coordinator.request_refresh(RefreshTrigger::Manual);
        let snapshot = await_terminal(&coordinator);

        assert_eq!(snapshot.last_successful_refresh_at_ms, Some(1_600));
    }

    #[test]
    fn event_sink_observes_submission_and_committed_completion() {
        let collector = Arc::new(ScriptedCollector::new(|request| {
            Ok(collection_for_request(&request, Vec::new()))
        }));
        let run_store = Arc::new(FakeRunStore::new());
        let usage_store = Arc::new(FakeUsageStore::new());
        let events = Arc::new(RecordingEventSink::new());
        let coordinator = RefreshCoordinator::with_event_sink(
            collector,
            run_store,
            usage_store,
            Arc::new(FakeClock { now_ms: 1_000 }),
            events.clone(),
            "0.1.0",
            "UTC",
        );

        coordinator.request_refresh(RefreshTrigger::Manual);
        await_terminal(&coordinator);

        assert_eq!(
            events.events(),
            vec![
                (RefreshStatus::Running, false),
                (RefreshStatus::Succeeded, true),
            ]
        );
    }

    #[test]
    fn budget_evaluation_runs_after_daily_commit_without_failing_refresh() {
        let collector = Arc::new(ScriptedCollector::new(|request| {
            Ok(collection_for_request(&request, Vec::new()))
        }));
        let run_store = Arc::new(FakeRunStore::new());
        let usage_store = Arc::new(FakeUsageStore::new());
        let evaluator = Arc::new(RecordingBudgetEvaluator::new());
        evaluator.fail();
        let coordinator = RefreshCoordinator::with_event_sink_and_budget_evaluator(
            collector,
            run_store.clone(),
            usage_store.clone(),
            Arc::new(FakeClock { now_ms: 1_000 }),
            RefreshCoordinatorHooks::new(Arc::new(NoopRefreshEventSink), evaluator.clone()),
            "0.1.0",
            "Asia/Jakarta",
        );

        coordinator.request_refresh(RefreshTrigger::Manual);
        let snapshot = await_terminal(&coordinator);

        assert_eq!(snapshot.status, RefreshStatus::Succeeded);
        assert_eq!(
            usage_store.reconciled_projections(),
            expected_refresh_projections()
        );
        assert_eq!(
            run_store.refresh_outcomes(),
            repeated_refresh_outcomes(RefreshOutcome::Succeeded)
        );
        assert_eq!(
            evaluator.calls(),
            vec![
                ("Asia/Jakarta".to_owned(), 1_000),
                ("Asia/Jakarta".to_owned(), 1_000),
                ("Asia/Jakarta".to_owned(), 1_000),
            ]
        );
    }

    #[test]
    fn empty_collection_succeeds_with_no_records() {
        let collector = Arc::new(ScriptedCollector::new(|request| {
            Ok(empty_collection_for_request(&request))
        }));
        let (coordinator, run_store, usage_store) = coordinator_with(collector);

        let submitted = coordinator.request_refresh(RefreshTrigger::Launch);
        assert_eq!(submitted.status, RefreshStatus::Running);
        let snapshot = await_terminal(&coordinator);

        assert_eq!(snapshot.status, RefreshStatus::Succeeded);
        assert_eq!(
            usage_store.reconciled_outcomes(),
            repeated_collection_outcomes(CollectionOutcome::Empty)
        );
        assert_eq!(
            run_store.refresh_outcomes(),
            repeated_refresh_outcomes(RefreshOutcome::Succeeded)
        );
    }

    #[test]
    fn partial_collection_reports_partial_without_failing() {
        let collector = Arc::new(ScriptedCollector::new(|request| {
            Ok(collection_for_request(&request, vec![rejected_record()]))
        }));
        let (coordinator, run_store, usage_store) = coordinator_with(collector);

        let submitted = coordinator.request_refresh(RefreshTrigger::Scheduled);
        assert_eq!(submitted.status, RefreshStatus::Running);
        let snapshot = await_terminal(&coordinator);

        assert_eq!(snapshot.status, RefreshStatus::Partial);
        assert_eq!(
            usage_store.reconciled_outcomes(),
            repeated_collection_outcomes(CollectionOutcome::Partial)
        );
        assert_eq!(
            run_store.import_outcomes(),
            repeated_import_outcomes(ImportOutcome::Partial)
        );
        assert_eq!(
            run_store.refresh_outcomes(),
            repeated_refresh_outcomes(RefreshOutcome::Partial)
        );
    }

    #[test]
    fn failed_collection_records_failure_and_changes_no_facts() {
        let collector = Arc::new(ScriptedCollector::new(|request| {
            Err(CollectorFailure::new(
                CollectorFailureCode::SpawnFailed,
                Some(request.source()),
                Some(request.projection()),
            ))
        }));
        let (coordinator, run_store, usage_store) = coordinator_with(collector);

        let submitted = coordinator.request_refresh(RefreshTrigger::Manual);
        assert_eq!(submitted.status, RefreshStatus::Running);
        let snapshot = await_terminal(&coordinator);

        assert_eq!(snapshot.status, RefreshStatus::Failed);
        assert_eq!(snapshot.last_successful_refresh_at_ms, None);
        assert!(usage_store.reconciled_outcomes().is_empty());
        assert!(run_store.import_outcomes().is_empty());
        assert_eq!(run_store.refresh_outcomes(), vec![RefreshOutcome::Failed]);
    }

    #[test]
    fn source_resolution_failure_terminalizes_the_refresh_run() {
        let collector = Arc::new(ScriptedCollector::new(|request| {
            Ok(empty_collection_for_request(&request))
        }));
        let (coordinator, run_store, usage_store) = coordinator_with(collector);
        run_store.fail_once(RunStoreFailure::ResolveSource);

        coordinator.request_refresh(RefreshTrigger::Manual);
        let snapshot = await_terminal(&coordinator);

        assert_eq!(snapshot.status, RefreshStatus::Failed);
        assert_eq!(run_store.refresh_outcomes(), vec![RefreshOutcome::Failed]);
        assert!(run_store.import_outcomes().is_empty());
        assert!(usage_store.reconciled_outcomes().is_empty());
    }

    #[test]
    fn import_creation_failure_terminalizes_the_refresh_run() {
        let collector = Arc::new(ScriptedCollector::new(|request| {
            Ok(empty_collection_for_request(&request))
        }));
        let (coordinator, run_store, _usage_store) = coordinator_with(collector);
        run_store.fail_once(RunStoreFailure::BeginImport);

        coordinator.request_refresh(RefreshTrigger::Manual);
        let snapshot = await_terminal(&coordinator);

        assert_eq!(snapshot.status, RefreshStatus::Failed);
        assert_eq!(run_store.refresh_outcomes(), vec![RefreshOutcome::Failed]);
        assert!(run_store.import_outcomes().is_empty());
    }

    #[test]
    fn reconciliation_failure_terminalizes_import_and_refresh_runs() {
        let collector = Arc::new(ScriptedCollector::new(|request| {
            Ok(collection_for_request(&request, Vec::new()))
        }));
        let (coordinator, run_store, usage_store) = coordinator_with(collector);
        usage_store.fail();

        coordinator.request_refresh(RefreshTrigger::Manual);
        let snapshot = await_terminal(&coordinator);

        assert_eq!(snapshot.status, RefreshStatus::Failed);
        assert_eq!(run_store.import_outcomes(), vec![ImportOutcome::Failed]);
        assert_eq!(run_store.refresh_outcomes(), vec![RefreshOutcome::Failed]);
        assert!(usage_store.reconciled_outcomes().is_empty());
    }

    #[test]
    fn completion_failures_retry_terminal_cleanup() {
        let collector = Arc::new(ScriptedCollector::new(|request| {
            Ok(collection_for_request(&request, Vec::new()))
        }));
        let (coordinator, run_store, _usage_store) = coordinator_with(collector);
        run_store.fail_once(RunStoreFailure::CompleteImport);

        coordinator.request_refresh(RefreshTrigger::Manual);
        let snapshot = await_terminal(&coordinator);

        assert_eq!(snapshot.status, RefreshStatus::Failed);
        assert_eq!(run_store.import_outcomes(), vec![ImportOutcome::Failed]);
        assert_eq!(run_store.refresh_outcomes(), vec![RefreshOutcome::Failed]);

        let collector = Arc::new(ScriptedCollector::new(|request| {
            Ok(collection_for_request(&request, Vec::new()))
        }));
        let (coordinator, run_store, _usage_store) = coordinator_with(collector);
        run_store.fail_once(RunStoreFailure::CompleteRefresh);

        coordinator.request_refresh(RefreshTrigger::Manual);
        let snapshot = await_terminal(&coordinator);

        assert_eq!(snapshot.status, RefreshStatus::Failed);
        assert_eq!(
            run_store.import_outcomes(),
            repeated_import_outcomes(ImportOutcome::Succeeded)
        );
        assert_eq!(run_store.refresh_outcomes(), vec![RefreshOutcome::Failed]);
    }

    struct GatedCollector {
        calls: AtomicUsize,
        started: (Mutex<bool>, Condvar),
        release: (Mutex<bool>, Condvar),
    }

    impl GatedCollector {
        fn new() -> Self {
            Self {
                calls: AtomicUsize::new(0),
                started: (Mutex::new(false), Condvar::new()),
                release: (Mutex::new(false), Condvar::new()),
            }
        }

        fn wait_started(&self) {
            let (lock, condvar) = &self.started;
            let mut started = lock.lock().expect("lock");
            while !*started {
                started = condvar.wait(started).expect("wait started");
            }
        }

        fn release(&self) {
            let (lock, condvar) = &self.release;
            *lock.lock().expect("lock") = true;
            condvar.notify_all();
        }

        fn calls(&self) -> usize {
            self.calls.load(Ordering::Relaxed)
        }
    }

    impl Collector for GatedCollector {
        fn describe(&self) -> Result<CollectorDescriptor, CollectorFailure> {
            unimplemented!("not used")
        }

        fn detect(
            &self,
            _request: DetectionRequest,
            _cancellation: &dyn CancellationSignal,
        ) -> Result<DetectionResult, CollectorFailure> {
            unimplemented!("not used")
        }

        fn collect(
            &self,
            request: CollectionRequest,
            _cancellation: &dyn CancellationSignal,
        ) -> Result<CollectionResult, CollectorFailure> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            {
                let (lock, condvar) = &self.started;
                *lock.lock().expect("lock") = true;
                condvar.notify_all();
            }
            {
                let (lock, condvar) = &self.release;
                let mut released = lock.lock().expect("lock");
                while !*released {
                    released = condvar.wait(released).expect("wait release");
                }
            }
            Ok(empty_collection_for_request(&request))
        }
    }

    #[test]
    fn concurrent_requests_coalesce_into_one_run() {
        let collector = Arc::new(GatedCollector::new());
        let (coordinator, _run_store, _usage_store) = coordinator_with(collector.clone());
        let coordinator = Arc::new(coordinator);

        let submitted = coordinator.request_refresh(RefreshTrigger::Manual);
        assert_eq!(submitted.status, RefreshStatus::Running);

        collector.wait_started();
        let coalesced = coordinator.request_refresh(RefreshTrigger::Scheduled);
        assert_eq!(coalesced.status, RefreshStatus::Running);

        collector.release();
        assert_eq!(
            await_terminal(&coordinator).status,
            RefreshStatus::Succeeded
        );

        assert_eq!(collector.calls(), refresh_targets().len());
    }

    #[test]
    fn cancel_moves_an_active_run_toward_cancelling() {
        let collector = Arc::new(GatedCollector::new());
        let (coordinator, _run_store, _usage_store) = coordinator_with(collector.clone());
        let coordinator = Arc::new(coordinator);

        coordinator.request_refresh(RefreshTrigger::Manual);

        collector.wait_started();
        let cancelling = coordinator.cancel();
        assert_eq!(cancelling.status, RefreshStatus::Cancelling);

        collector.release();
        assert_eq!(
            await_terminal(&coordinator).status,
            RefreshStatus::Succeeded
        );
    }
}
