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

use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum BudgetEvaluationError {
    #[error("budget storage failed")]
    StorageUnavailable,
}
use crate::application::ports::clock::Clock;
use crate::application::ports::collector::Collector;
use crate::application::ports::run_store::RunStore;
use crate::application::ports::usage_store::UsageStore;
use crate::application::reconciliation::RefreshTrigger;

use super::execution::{execute_refresh, RefreshExecution};
use super::outcome::RunOutcome;
use super::request_plan::RefreshScopePolicy;
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

pub(crate) trait CommittedDailyUploadSink: Send + Sync {
    fn on_committed_daily_upload(
        &self,
        upload: crate::application::collect_sync::CommittedDailyUpload,
    );
}

pub(crate) struct NoopCommittedDailyUploadSink;

impl CommittedDailyUploadSink for NoopCommittedDailyUploadSink {
    fn on_committed_daily_upload(
        &self,
        _upload: crate::application::collect_sync::CommittedDailyUpload,
    ) {
    }
}

pub(crate) struct RefreshCoordinatorHooks {
    event_sink: Arc<dyn RefreshEventSink>,
    budget_evaluator: Arc<dyn BudgetEvaluationRunner>,
    committed_daily_upload_sink: Arc<Mutex<Arc<dyn CommittedDailyUploadSink>>>,
}

impl RefreshCoordinatorHooks {
    pub(crate) fn new(
        event_sink: Arc<dyn RefreshEventSink>,
        budget_evaluator: Arc<dyn BudgetEvaluationRunner>,
    ) -> Self {
        Self {
            event_sink,
            budget_evaluator,
            committed_daily_upload_sink: Arc::new(Mutex::new(Arc::new(
                NoopCommittedDailyUploadSink,
            ))),
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
    committed_daily_upload_sink: Arc<Mutex<Arc<dyn CommittedDailyUploadSink>>>,
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
            committed_daily_upload_sink: hooks.committed_daily_upload_sink,
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

    pub(crate) fn set_committed_daily_upload_sink(
        &self,
        sink: Arc<dyn CommittedDailyUploadSink>,
    ) {
        *self
            .committed_daily_upload_sink
            .lock()
            .expect("committed upload sink lock is poisoned") = sink;
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
        self.request_refresh_with_scope_policy(trigger, RefreshScopePolicy::CatchUp)
    }

    pub(crate) fn request_full_refresh(&self, trigger: RefreshTrigger) -> RefreshSnapshot {
        self.request_refresh_with_scope_policy(trigger, RefreshScopePolicy::Full)
    }

    pub(crate) fn request_freshness_refresh(&self, trigger: RefreshTrigger) -> RefreshSnapshot {
        self.request_refresh_with_scope_policy(trigger, RefreshScopePolicy::Freshness)
    }

    fn request_refresh_with_scope_policy(
        &self,
        trigger: RefreshTrigger,
        scope_policy: RefreshScopePolicy,
    ) -> RefreshSnapshot {
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
            .spawn(move || worker.finish_refresh(trigger, scope_policy, job_id, now_ms))
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

    fn finish_refresh(
        &self,
        trigger: RefreshTrigger,
        scope_policy: RefreshScopePolicy,
        job_id: String,
        started_at_ms: i64,
    ) {
        let aggregation_timezone = self.aggregation_timezone();
        let result = execute_refresh(
            RefreshExecution {
                collector: self.collector.as_ref(),
                run_store: self.run_store.as_ref(),
                usage_store: self.usage_store.as_ref(),
                budget_evaluator: self.budget_evaluator.as_ref(),
                clock: self.clock.as_ref(),
                app_version: &self.app_version,
                aggregation_timezone,
            },
            trigger,
            scope_policy,
            &job_id,
            started_at_ms,
        );
        let snapshot = {
            let mut state = self.lock_state();
            state.status = result.outcome.status();
            if matches!(result.outcome, RunOutcome::Succeeded) {
                state.last_successful_refresh_at_ms = Some(result.finished_at_ms);
            }
            state.snapshot()
        };
        // Cloud upload is best-effort and never changes refresh outcome.
        if !result.committed_daily_upload.is_empty() {
            let sink = self
                .committed_daily_upload_sink
                .lock()
                .expect("committed upload sink lock is poisoned")
                .clone();
            sink.on_committed_daily_upload(result.committed_daily_upload);
        }
        self.event_sink.publish(snapshot, result.usage_changed);
    }
}
