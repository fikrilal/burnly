use std::sync::atomic::{AtomicBool, AtomicI64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use chrono::{TimeZone, Utc};

use super::coordinator::{
    BudgetEvaluationError, BudgetEvaluationRunner, CommittedDailyUploadSink, RefreshCoordinator,
    RefreshCoordinatorHooks, RefreshEventSink,
};
use super::state::{RefreshSnapshot, RefreshStatus};
use super::target::refresh_targets;
use crate::application::collect_sync::{CommittedDailyUpload, UploadScope};
use crate::application::collection::{
    CandidateProvenance, CollectionId, CollectionMetadata, CollectionOutcome, CollectionPeriod,
    CollectionProjection, CollectionRequest, CollectionResult, CollectionScope,
    CollectorDescriptor, CollectorFailure, CollectorFailureCode, DailyUsageCandidate,
    DetectionRequest, DetectionResult, ProcessSummary, RejectedRecord, SessionUsageCandidate,
};
use crate::application::diagnostics::{DiagnosticArea, DiagnosticEvent, DiagnosticSeverity};
use crate::application::ports::clock::Clock;
use crate::application::ports::collector::{CancellationSignal, Collector};
use crate::application::ports::diagnostic_recorder::DiagnosticRecorder;
use crate::application::ports::run_store::{RunStore, RunStoreError};
use crate::application::ports::usage_store::{UsageStore, UsageStoreError};
use crate::application::reconciliation::{
    DailyReconciliationRequest, DailyReconciliationSummary, ImportOutcome, ImportRunCompletion,
    ImportRunId, ImportRunLookup, ImportRunSpec, RefreshOutcome, RefreshRunCompletion,
    RefreshRunId, RefreshRunSpec, RefreshTrigger, SourceId, SuccessfulImportState,
};
use crate::domain::source::SourceKey;
use crate::domain::usage::{
    CostKind, CurrencyCode, DataQuality, TokenUsage, UsageCost, ValuedCostStatus,
};

#[path = "test_support.rs"]
mod test_support;

use test_support::*;

struct FakeRunStore {
    refresh_outcomes: Mutex<Vec<RefreshOutcome>>,
    import_outcomes: Mutex<Vec<ImportOutcome>>,
    import_scopes: Mutex<Vec<CollectionScope>>,
    latest_imports: Mutex<Vec<SuccessfulImportState>>,
    failure: Mutex<Option<RunStoreFailure>>,
    next_id: AtomicUsize,
}

#[derive(Default)]
struct RecordingUploadSink {
    uploads: Mutex<Vec<CommittedDailyUpload>>,
}

impl CommittedDailyUploadSink for RecordingUploadSink {
    fn on_committed_daily_upload(&self, upload: CommittedDailyUpload) {
        self.uploads.lock().expect("lock").push(upload);
    }
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
            import_scopes: Mutex::new(Vec::new()),
            latest_imports: Mutex::new(Vec::new()),
            failure: Mutex::new(None),
            next_id: AtomicUsize::new(1),
        }
    }

    fn seed_successful_import(&self, state: SuccessfulImportState) {
        self.latest_imports.lock().expect("lock").push(state);
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

    fn import_scopes(&self) -> Vec<CollectionScope> {
        self.import_scopes.lock().expect("lock").clone()
    }
}

impl RunStore for FakeRunStore {
    fn resolve_source(&self, _source: SourceKey, _now_ms: i64) -> Result<SourceId, RunStoreError> {
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
        spec: ImportRunSpec,
        _now_ms: i64,
    ) -> Result<ImportRunId, RunStoreError> {
        if self.take_failure(RunStoreFailure::BeginImport) {
            return Err(RunStoreError::Backend);
        }
        self.import_scopes
            .lock()
            .expect("lock")
            .push(spec.scope().clone());
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

    fn latest_successful_import(
        &self,
        lookup: ImportRunLookup,
    ) -> Result<Option<SuccessfulImportState>, RunStoreError> {
        Ok(self
            .latest_imports
            .lock()
            .expect("lock")
            .iter()
            .find(|state| {
                state.source() == lookup.source() && state.projection() == lookup.projection()
            })
            .cloned())
    }
}

struct FakeUsageStore {
    reconciled: Mutex<Vec<(CollectionProjection, CollectionOutcome, CollectionScope)>>,
    fail: AtomicBool,
}

struct RecordingEventSink {
    events: Mutex<Vec<(RefreshStatus, bool)>>,
}

struct NoopTestEventSink;

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

impl RefreshEventSink for NoopTestEventSink {
    fn publish(&self, _snapshot: RefreshSnapshot, _usage_changed: bool) {}
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
            .map(|(_, outcome, _)| *outcome)
            .collect()
    }

    fn reconciled_projections(&self) -> Vec<CollectionProjection> {
        self.reconciled
            .lock()
            .expect("lock")
            .iter()
            .map(|(projection, _, _)| *projection)
            .collect()
    }

    fn reconciled_scopes(&self) -> Vec<CollectionScope> {
        self.reconciled
            .lock()
            .expect("lock")
            .iter()
            .map(|(_, _, scope)| scope.clone())
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
        self.reconciled.lock().expect("lock").push((
            CollectionProjection::Daily,
            request.outcome(),
            request.scope().clone(),
        ));
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
        self.reconciled.lock().expect("lock").push((
            CollectionProjection::Session,
            request.outcome(),
            request.scope().clone(),
        ));
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
    behavior:
        Box<dyn Fn(CollectionRequest) -> Result<CollectionResult, CollectorFailure> + Send + Sync>,
    calls: AtomicUsize,
    requests: Mutex<Vec<(SourceKey, CollectionProjection)>>,
    scopes: Mutex<Vec<CollectionScope>>,
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
            scopes: Mutex::new(Vec::new()),
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::Relaxed)
    }

    fn requests(&self) -> Vec<(SourceKey, CollectionProjection)> {
        self.requests.lock().expect("lock").clone()
    }

    fn scopes(&self) -> Vec<CollectionScope> {
        self.scopes.lock().expect("lock").clone()
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
        self.scopes
            .lock()
            .expect("lock")
            .push(request.scope().clone());
        (self.behavior)(request)
    }
}

#[test]
fn complete_collection_reconciles_and_succeeds() {
    let collector = successful_collector();
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
fn missing_baseline_uses_full_scope_for_collector_import_and_reconciliation() {
    let collector = successful_collector();
    let (coordinator, run_store, usage_store) = coordinator_with(collector.clone());

    coordinator.request_refresh(RefreshTrigger::Manual);
    let snapshot = await_terminal(&coordinator);

    assert_eq!(snapshot.status, RefreshStatus::Succeeded);
    assert_eq!(collector.scopes(), repeated_scope(CollectionScope::Full));
    assert_eq!(
        run_store.import_scopes(),
        repeated_scope(CollectionScope::Full)
    );
    assert_eq!(
        usage_store.reconciled_scopes(),
        repeated_scope(CollectionScope::Full)
    );
}

#[test]
fn manual_refresh_uses_incremental_catch_up_after_baseline() {
    let collector = successful_collector();
    let run_store = Arc::new(FakeRunStore::new());
    let usage_store = Arc::new(FakeUsageStore::new());
    let previous_scope =
        CollectionScope::incremental(date(2026, 6, 20), date(2026, 6, 20)).expect("scope");
    seed_successful_imports(&run_store, previous_scope);
    let expected_scope =
        CollectionScope::incremental(date(2026, 6, 18), date(2026, 6, 28)).expect("scope");
    let coordinator = RefreshCoordinator::new(
        collector.clone(),
        run_store.clone(),
        usage_store.clone(),
        Arc::new(FakeClock {
            now_ms: Utc
                .with_ymd_and_hms(2026, 6, 28, 12, 0, 0)
                .single()
                .expect("timestamp")
                .timestamp_millis(),
        }),
        "0.1.0",
        "UTC",
    );

    coordinator.request_refresh(RefreshTrigger::Manual);
    let snapshot = await_terminal(&coordinator);

    assert_eq!(snapshot.status, RefreshStatus::Succeeded);
    assert_eq!(collector.scopes(), repeated_scope(expected_scope.clone()));
    assert_eq!(
        run_store.import_scopes(),
        repeated_scope(expected_scope.clone())
    );
    assert_eq!(
        usage_store.reconciled_scopes(),
        repeated_scope(expected_scope)
    );
}

#[test]
fn freshness_refresh_uses_today_only_after_baseline() {
    let collector = successful_collector();
    let run_store = Arc::new(FakeRunStore::new());
    let usage_store = Arc::new(FakeUsageStore::new());
    let previous_scope =
        CollectionScope::incremental(date(2026, 6, 20), date(2026, 6, 20)).expect("scope");
    seed_successful_imports(&run_store, previous_scope);
    let expected_scope =
        CollectionScope::incremental(date(2026, 6, 28), date(2026, 6, 28)).expect("scope");
    let coordinator = RefreshCoordinator::new(
        collector.clone(),
        run_store.clone(),
        usage_store.clone(),
        Arc::new(FakeClock {
            now_ms: Utc
                .with_ymd_and_hms(2026, 6, 28, 12, 0, 0)
                .single()
                .expect("timestamp")
                .timestamp_millis(),
        }),
        "0.1.0",
        "UTC",
    );

    coordinator.request_freshness_refresh(RefreshTrigger::Manual);
    let snapshot = await_terminal(&coordinator);

    assert_eq!(snapshot.status, RefreshStatus::Succeeded);
    assert_eq!(collector.scopes(), repeated_scope(expected_scope.clone()));
    assert_eq!(
        run_store.import_scopes(),
        repeated_scope(expected_scope.clone())
    );
    assert_eq!(
        usage_store.reconciled_scopes(),
        repeated_scope(expected_scope)
    );
}

#[test]
fn freshness_refresh_without_baseline_uses_full_scope() {
    let collector = successful_collector();
    let (coordinator, run_store, usage_store) = coordinator_with(collector.clone());

    coordinator.request_freshness_refresh(RefreshTrigger::Manual);
    let snapshot = await_terminal(&coordinator);

    assert_eq!(snapshot.status, RefreshStatus::Succeeded);
    assert_eq!(collector.scopes(), repeated_scope(CollectionScope::Full));
    assert_eq!(
        run_store.import_scopes(),
        repeated_scope(CollectionScope::Full)
    );
    assert_eq!(
        usage_store.reconciled_scopes(),
        repeated_scope(CollectionScope::Full)
    );
}

#[test]
fn successful_refresh_records_completion_time() {
    let collector = empty_collector();
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

    // AdvancingClock ticks 100 per read; the commit timestamp tracks the
    // number of refresh targets.
    assert_eq!(
        snapshot.last_successful_refresh_at_ms,
        Some(1_000 + refresh_targets().len() as i64 * 100)
    );
}

#[test]
fn event_sink_observes_submission_and_committed_completion() {
    let collector = successful_collector();
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
    let collector = successful_collector();
    let run_store = Arc::new(FakeRunStore::new());
    let usage_store = Arc::new(FakeUsageStore::new());
    let evaluator = Arc::new(RecordingBudgetEvaluator::new());
    evaluator.fail();
    let coordinator = RefreshCoordinator::with_event_sink_and_budget_evaluator(
        collector,
        run_store.clone(),
        usage_store.clone(),
        Arc::new(FakeClock { now_ms: 1_000 }),
        RefreshCoordinatorHooks::new(Arc::new(NoopTestEventSink), evaluator.clone()),
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
    // The budget evaluator runs once per daily commit.
    let daily_targets = refresh_targets()
        .iter()
        .filter(|target| target.projection == CollectionProjection::Daily)
        .count();
    assert_eq!(
        evaluator.calls(),
        vec![("Asia/Jakarta".to_owned(), 1_000); daily_targets]
    );
}

#[test]
fn empty_collection_succeeds_with_no_records() {
    let collector = empty_collector();
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
fn collector_failure_for_one_target_keeps_later_targets_and_marks_partial() {
    let collector = Arc::new(ScriptedCollector::new(|request| {
        if request.source() == SourceKey::Pi && request.projection() == CollectionProjection::Daily
        {
            return Err(CollectorFailure::new(
                CollectorFailureCode::IncompatibleEnvelope,
                Some(request.source()),
                Some(request.projection()),
            ));
        }
        Ok(empty_collection_for_request(&request))
    }));
    let (coordinator, run_store, usage_store) = coordinator_with(collector.clone());
    let upload_sink = Arc::new(RecordingUploadSink::default());
    coordinator.set_committed_daily_upload_sink(upload_sink.clone());

    coordinator.request_full_refresh(RefreshTrigger::Manual);
    let snapshot = await_terminal(&coordinator);

    assert_eq!(snapshot.status, RefreshStatus::Partial);
    assert_eq!(collector.calls(), refresh_targets().len());
    assert_eq!(run_store.refresh_outcomes(), vec![RefreshOutcome::Partial]);
    assert_eq!(
        run_store.import_outcomes(),
        vec![ImportOutcome::Succeeded; refresh_targets().len() - 1]
    );
    assert_eq!(
        usage_store.reconciled_outcomes(),
        vec![CollectionOutcome::Empty; refresh_targets().len() - 1]
    );
    let uploads = upload_sink.uploads.lock().expect("lock");
    assert_eq!(uploads.len(), 1);
    assert!(!uploads[0].full_refresh_complete);
    assert!(matches!(
        uploads[0].clone().into_upload_scope().expect("scope"),
        UploadScope::Incremental { ref source_keys, .. }
            if !source_keys.contains(SourceKey::Pi.as_str())
                && source_keys.len() == 8
    ));
}

#[test]
fn collector_hard_fail_records_diagnostic_with_source_projection_and_failure_code() {
    let collector = Arc::new(ScriptedCollector::new(|request| {
        if request.source() == SourceKey::ClaudeCode
            && request.projection() == CollectionProjection::Session
        {
            return Err(CollectorFailure::new(
                CollectorFailureCode::IncompatibleEnvelope,
                Some(request.source()),
                Some(request.projection()),
            ));
        }
        Ok(empty_collection_for_request(&request))
    }));
    let (coordinator, _run_store, _usage_store) = coordinator_with(collector);
    let diagnostics = Arc::new(RecordingDiagnosticRecorder::default());
    coordinator.set_diagnostic_recorder(diagnostics.clone());

    coordinator.request_full_refresh(RefreshTrigger::Manual);
    let snapshot = await_terminal(&coordinator);

    assert_eq!(snapshot.status, RefreshStatus::Partial);
    let events = diagnostics.events();
    assert_eq!(events.len(), 1);
    let event = &events[0];
    assert_eq!(event.area, DiagnosticArea::Collector);
    assert_eq!(event.severity, DiagnosticSeverity::Warning);
    assert_eq!(event.code.as_str(), "collection.target_failed");
    assert_eq!(
        event.summary.as_str(),
        "Collection failed for one refresh target."
    );
    let context = event.context.as_ref().expect("context").as_str();
    assert!(context.contains(r#""source":"claude-code""#));
    assert!(context.contains(r#""projection":"session""#));
    assert!(context.contains(r#""failureCode":"collector.incompatible_envelope""#));
    assert!(!context.contains("path"));
    assert!(!context.contains("stdout"));
}

#[derive(Default)]
struct RecordingDiagnosticRecorder {
    events: Mutex<Vec<DiagnosticEvent>>,
}

impl RecordingDiagnosticRecorder {
    fn events(&self) -> Vec<DiagnosticEvent> {
        self.events.lock().expect("lock").clone()
    }
}

impl DiagnosticRecorder for RecordingDiagnosticRecorder {
    fn record(&self, event: DiagnosticEvent) {
        self.events.lock().expect("lock").push(event);
    }
}

#[test]
fn source_resolution_failure_terminalizes_the_refresh_run() {
    let collector = empty_collector();
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
    let collector = empty_collector();
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
    let collector = successful_collector();
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
    let collector = successful_collector();
    let (coordinator, run_store, _usage_store) = coordinator_with(collector);
    run_store.fail_once(RunStoreFailure::CompleteImport);

    coordinator.request_refresh(RefreshTrigger::Manual);
    let snapshot = await_terminal(&coordinator);

    assert_eq!(snapshot.status, RefreshStatus::Failed);
    assert_eq!(run_store.import_outcomes(), vec![ImportOutcome::Failed]);
    assert_eq!(run_store.refresh_outcomes(), vec![RefreshOutcome::Failed]);

    let collector = successful_collector();
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
