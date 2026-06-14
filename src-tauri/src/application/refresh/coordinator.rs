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

use chrono::DateTime;

use crate::application::collection::{
    CollectionId, CollectionOutcome, CollectionProjection, CollectionRequest, CollectionResult,
    CollectionScope, CollectorFailure,
};
use crate::application::ports::clock::Clock;
use crate::application::ports::collector::{CancellationSignal, Collector};
use crate::application::ports::run_store::RunStore;
use crate::application::ports::usage_store::UsageStore;
use crate::application::reconciliation::{
    DailyReconciliationRequest, ImportCollector, ImportOutcome, ImportRunCompletion, ImportRunSpec,
    JobKey, RefreshOutcome, RefreshRunCompletion, RefreshRunSpec, RefreshTrigger, RunError,
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

pub(crate) struct RefreshCoordinator {
    collector: Arc<dyn Collector>,
    run_store: Arc<dyn RunStore>,
    usage_store: Arc<dyn UsageStore>,
    clock: Arc<dyn Clock>,
    app_version: String,
    aggregation_timezone: String,
    sequence: AtomicU64,
    state: Mutex<CoordinatorState>,
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
        Self {
            collector,
            run_store,
            usage_store,
            clock,
            app_version: app_version.into(),
            aggregation_timezone: aggregation_timezone.into(),
            sequence: AtomicU64::new(0),
            state: Mutex::new(CoordinatorState {
                status: RefreshStatus::Idle,
                job_id: None,
                trigger: None,
                last_successful_refresh_at_ms: None,
            }),
        }
    }

    pub(crate) fn snapshot(&self) -> RefreshSnapshot {
        self.lock_state().snapshot()
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

        {
            let mut state = self.lock_state();
            if state.status.is_active() {
                return state.snapshot();
            }
            state.status = RefreshStatus::Running;
            state.job_id = Some(job_id.clone());
            state.trigger = Some(trigger);
        }

        let outcome = self.execute(trigger, &job_id, now_ms);

        {
            let mut state = self.lock_state();
            state.status = outcome.status();
            if matches!(outcome, RunOutcome::Succeeded) {
                state.last_successful_refresh_at_ms = Some(now_ms);
            }
        }

        self.snapshot()
    }

    fn lock_state(&self) -> std::sync::MutexGuard<'_, CoordinatorState> {
        self.state.lock().expect("refresh state lock is poisoned")
    }

    fn next_job_id(&self, now_ms: i64) -> String {
        let sequence = self.sequence.fetch_add(1, Ordering::Relaxed);
        format!("refresh-{now_ms}-{sequence}")
    }

    fn execute(&self, trigger: RefreshTrigger, job_id: &str, now_ms: i64) -> RunOutcome {
        self.try_execute(trigger, job_id, now_ms)
            .unwrap_or(RunOutcome::Failed)
    }

    fn try_execute(
        &self,
        trigger: RefreshTrigger,
        job_id: &str,
        now_ms: i64,
    ) -> Result<RunOutcome, CoordinatorError> {
        let job_key = JobKey::new(job_id).map_err(|_| CoordinatorError)?;
        let spec = RefreshRunSpec::new(job_key, trigger, self.app_version.clone())
            .map_err(|_| CoordinatorError)?;
        let refresh_run_id = self
            .run_store
            .begin_refresh_run(spec, now_ms)
            .map_err(|_| CoordinatorError)?;

        let source_id = self
            .run_store
            .resolve_source(SourceKey::ClaudeCode, now_ms)
            .map_err(|_| CoordinatorError)?;

        let requested_at = DateTime::from_timestamp_millis(now_ms).ok_or(CoordinatorError)?;
        let request = CollectionRequest::daily(
            CollectionId::new(job_id).map_err(|_| CoordinatorError)?,
            SourceKey::ClaudeCode,
            CollectionScope::Full,
            self.aggregation_timezone.clone(),
            requested_at,
        )
        .map_err(|_| CoordinatorError)?;

        match self.collector.collect(request, &NeverCancelled) {
            Ok(collection) => self.persist(refresh_run_id, source_id, now_ms, &collection),
            Err(failure) => {
                self.run_store
                    .complete_refresh_run(
                        refresh_run_id,
                        RefreshRunCompletion {
                            outcome: RefreshOutcome::Failed,
                            finished_at_ms: self.clock.now_epoch_ms(),
                            error: collector_error(&failure),
                        },
                    )
                    .map_err(|_| CoordinatorError)?;
                Ok(RunOutcome::Failed)
            }
        }
    }

    fn persist(
        &self,
        refresh_run_id: crate::application::reconciliation::RefreshRunId,
        source_id: crate::application::reconciliation::SourceId,
        now_ms: i64,
        collection: &CollectionResult,
    ) -> Result<RunOutcome, CoordinatorError> {
        let metadata = collection.metadata();
        let import_collector = ImportCollector::new(
            metadata.collector().as_str(),
            metadata.collector_version(),
            metadata.profile_version(),
        )
        .map_err(|_| CoordinatorError)?;
        let import_spec = ImportRunSpec::new(
            refresh_run_id,
            source_id,
            import_collector,
            CollectionProjection::Daily,
            CollectionScope::Full,
            Some(self.aggregation_timezone.clone()),
        )
        .map_err(|_| CoordinatorError)?;
        let import_run_id = self
            .run_store
            .begin_import_run(import_spec, now_ms)
            .map_err(|_| CoordinatorError)?;

        let collection_outcome = collection.outcome();
        let reconciliation = DailyReconciliationRequest::new(
            source_id,
            import_run_id,
            CollectionScope::Full,
            collection_outcome,
            now_ms,
            collection.daily_candidates().to_vec(),
        );
        self.usage_store
            .reconcile_daily(reconciliation)
            .map_err(|_| CoordinatorError)?;

        let outcome = RunOutcome::from_collection(collection_outcome);
        let finished_at_ms = self.clock.now_epoch_ms();
        let records_seen = clamp_count(collection.daily_candidates().len());
        let records_rejected = clamp_count(collection.rejection_count());

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
            .map_err(|_| CoordinatorError)?;
        self.run_store
            .complete_refresh_run(
                refresh_run_id,
                RefreshRunCompletion {
                    outcome: outcome.refresh_outcome(),
                    finished_at_ms,
                    error: None,
                },
            )
            .map_err(|_| CoordinatorError)?;

        Ok(outcome)
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
}

struct CoordinatorError;

fn collector_error(failure: &CollectorFailure) -> Option<RunError> {
    RunError::new(failure.code.code(), failure.to_string()).ok()
}

fn clamp_count(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Condvar, Mutex};
    use std::thread;

    use chrono::{TimeZone, Utc};

    use super::*;
    use crate::application::collection::{
        CandidateProvenance, CollectionMetadata, CollectionPeriod, CollectorDescriptor,
        CollectorFailureCode, DailyUsageCandidate, DetectionRequest, DetectionResult,
        ProcessSummary, RejectedRecord,
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
        next_id: AtomicUsize,
    }

    impl FakeRunStore {
        fn new() -> Self {
            Self {
                refresh_outcomes: Mutex::new(Vec::new()),
                import_outcomes: Mutex::new(Vec::new()),
                next_id: AtomicUsize::new(1),
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
            Ok(ImportRunId::new(self.next()))
        }

        fn complete_import_run(
            &self,
            _id: ImportRunId,
            completion: ImportRunCompletion,
        ) -> Result<(), RunStoreError> {
            self.import_outcomes
                .lock()
                .expect("lock")
                .push(completion.outcome);
            Ok(())
        }
    }

    struct FakeUsageStore {
        reconciled: Mutex<Vec<CollectionOutcome>>,
    }

    impl FakeUsageStore {
        fn new() -> Self {
            Self {
                reconciled: Mutex::new(Vec::new()),
            }
        }

        fn reconciled_outcomes(&self) -> Vec<CollectionOutcome> {
            self.reconciled.lock().expect("lock").clone()
        }
    }

    impl UsageStore for FakeUsageStore {
        fn reconcile_daily(
            &self,
            request: DailyReconciliationRequest,
        ) -> Result<DailyReconciliationSummary, UsageStoreError> {
            self.reconciled
                .lock()
                .expect("lock")
                .push(request.outcome());
            let observed = request
                .candidates()
                .iter()
                .map(|candidate| candidate.source_key.clone())
                .collect::<Vec<_>>();
            let upserted = u32::try_from(observed.len()).unwrap_or(u32::MAX);
            Ok(DailyReconciliationSummary::new(upserted, observed))
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

    fn timestamp(hour: u32) -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 6, 14, hour, 0, 0)
            .single()
            .expect("timestamp")
    }

    fn test_metadata() -> CollectionMetadata {
        CollectionMetadata::new(
            CollectionId::new("job-1").expect("collection id"),
            crate::application::collection::CollectorKey::new("fixture-collector")
                .expect("collector key"),
            "20.0.11".to_owned(),
            SourceKey::ClaudeCode,
            CollectionScope::Full,
            1,
            CollectionPeriod {
                started_at: timestamp(7),
                finished_at: timestamp(8),
            },
        )
        .expect("metadata")
    }

    fn candidate() -> DailyUsageCandidate {
        let tokens = TokenUsage::new(Some(100), Some(0), Some(0), Some(0), 100).expect("tokens");
        let cost = UsageCost::Valued {
            amount_micros: 10_000,
            currency: CurrencyCode::new("USD").expect("currency"),
            kind: CostKind::CollectorCalculated,
            status: ValuedCostStatus::Estimated,
        };
        DailyUsageCandidate {
            provenance: CandidateProvenance {
                source: SourceKey::ClaudeCode,
                collector: crate::application::collection::CollectorKey::new("fixture-collector")
                    .expect("collector key"),
                collector_version: "20.0.11".to_owned(),
                profile_version: 1,
                collection_id: CollectionId::new("job-1").expect("collection id"),
                observed_at: timestamp(8),
                data_quality: DataQuality::Complete,
                warnings: Vec::new(),
            },
            source_key: "claude-code:daily:v1:UTC:2026-06-13".to_owned(),
            usage_date: chrono::NaiveDate::from_ymd_opt(2026, 6, 13).expect("date"),
            aggregation_timezone: "UTC".to_owned(),
            tokens,
            cost,
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

    fn collection(
        candidates: Vec<DailyUsageCandidate>,
        rejections: Vec<RejectedRecord>,
    ) -> CollectionResult {
        CollectionResult::daily(
            test_metadata(),
            candidates,
            rejections,
            Vec::new(),
            process_summary(),
        )
        .expect("collection result")
    }

    struct ScriptedCollector {
        behavior: Box<dyn Fn() -> Result<CollectionResult, CollectorFailure> + Send + Sync>,
        calls: AtomicUsize,
    }

    impl ScriptedCollector {
        fn new(
            behavior: impl Fn() -> Result<CollectionResult, CollectorFailure> + Send + Sync + 'static,
        ) -> Self {
            Self {
                behavior: Box::new(behavior),
                calls: AtomicUsize::new(0),
            }
        }

        fn calls(&self) -> usize {
            self.calls.load(Ordering::Relaxed)
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
            _request: CollectionRequest,
            _cancellation: &dyn CancellationSignal,
        ) -> Result<CollectionResult, CollectorFailure> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            (self.behavior)()
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

    #[test]
    fn complete_collection_reconciles_and_succeeds() {
        let collector = Arc::new(ScriptedCollector::new(|| {
            Ok(collection(vec![candidate()], Vec::new()))
        }));
        let (coordinator, run_store, usage_store) = coordinator_with(collector.clone());

        let snapshot = coordinator.request_refresh(RefreshTrigger::Manual);

        assert_eq!(snapshot.status, RefreshStatus::Succeeded);
        assert_eq!(snapshot.last_successful_refresh_at_ms, Some(1_000));
        assert_eq!(collector.calls(), 1);
        assert_eq!(
            usage_store.reconciled_outcomes(),
            vec![CollectionOutcome::Complete]
        );
        assert_eq!(run_store.import_outcomes(), vec![ImportOutcome::Succeeded]);
        assert_eq!(
            run_store.refresh_outcomes(),
            vec![RefreshOutcome::Succeeded]
        );
    }

    #[test]
    fn empty_collection_succeeds_with_no_records() {
        let collector = Arc::new(ScriptedCollector::new(|| {
            Ok(collection(Vec::new(), Vec::new()))
        }));
        let (coordinator, run_store, usage_store) = coordinator_with(collector);

        let snapshot = coordinator.request_refresh(RefreshTrigger::Launch);

        assert_eq!(snapshot.status, RefreshStatus::Succeeded);
        assert_eq!(
            usage_store.reconciled_outcomes(),
            vec![CollectionOutcome::Empty]
        );
        assert_eq!(
            run_store.refresh_outcomes(),
            vec![RefreshOutcome::Succeeded]
        );
    }

    #[test]
    fn partial_collection_reports_partial_without_failing() {
        let collector = Arc::new(ScriptedCollector::new(|| {
            Ok(collection(
                vec![candidate()],
                vec![RejectedRecord {
                    code: "record.invalid".to_owned(),
                    record_index: Some(0),
                }],
            ))
        }));
        let (coordinator, run_store, usage_store) = coordinator_with(collector);

        let snapshot = coordinator.request_refresh(RefreshTrigger::Scheduled);

        assert_eq!(snapshot.status, RefreshStatus::Partial);
        assert_eq!(
            usage_store.reconciled_outcomes(),
            vec![CollectionOutcome::Partial]
        );
        assert_eq!(run_store.import_outcomes(), vec![ImportOutcome::Partial]);
        assert_eq!(run_store.refresh_outcomes(), vec![RefreshOutcome::Partial]);
    }

    #[test]
    fn failed_collection_records_failure_and_changes_no_facts() {
        let collector = Arc::new(ScriptedCollector::new(|| {
            Err(CollectorFailure::new(
                CollectorFailureCode::SpawnFailed,
                Some(SourceKey::ClaudeCode),
                Some(CollectionProjection::Daily),
            ))
        }));
        let (coordinator, run_store, usage_store) = coordinator_with(collector);

        let snapshot = coordinator.request_refresh(RefreshTrigger::Manual);

        assert_eq!(snapshot.status, RefreshStatus::Failed);
        assert_eq!(snapshot.last_successful_refresh_at_ms, None);
        assert!(usage_store.reconciled_outcomes().is_empty());
        assert!(run_store.import_outcomes().is_empty());
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
            _request: CollectionRequest,
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
            Ok(collection(Vec::new(), Vec::new()))
        }
    }

    #[test]
    fn concurrent_requests_coalesce_into_one_run() {
        let collector = Arc::new(GatedCollector::new());
        let (coordinator, _run_store, _usage_store) = coordinator_with(collector.clone());
        let coordinator = Arc::new(coordinator);

        let worker = coordinator.clone();
        let handle = thread::spawn(move || worker.request_refresh(RefreshTrigger::Manual));

        collector.wait_started();
        let coalesced = coordinator.request_refresh(RefreshTrigger::Scheduled);
        assert_eq!(coalesced.status, RefreshStatus::Running);

        collector.release();
        handle.join().expect("join worker");

        assert_eq!(collector.calls(), 1);
    }

    #[test]
    fn cancel_moves_an_active_run_toward_cancelling() {
        let collector = Arc::new(GatedCollector::new());
        let (coordinator, _run_store, _usage_store) = coordinator_with(collector.clone());
        let coordinator = Arc::new(coordinator);

        let worker = coordinator.clone();
        let handle = thread::spawn(move || worker.request_refresh(RefreshTrigger::Manual));

        collector.wait_started();
        let cancelling = coordinator.cancel();
        assert_eq!(cancelling.status, RefreshStatus::Cancelling);

        collector.release();
        handle.join().expect("join worker");
    }
}
