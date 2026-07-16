//! Collect-sync orchestration: baseline, scoped export, outbox drain, retries.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use chrono::NaiveDate;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::application::cloud_session::{CloudSession, SessionSnapshot};
use crate::application::collect_sync::{
    build_prepared_batches, map_exported_fact, BatchBuildLimits, BatchRequestMeta, UploadScope,
};
use crate::application::ports::clock::Clock;
use crate::application::ports::collect_sync_remote::{
    CollectSyncPlatform, CollectSyncRemote, CollectSyncRemoteError, PushDailyUsageRequest,
    UpsertSyncDeviceRequest,
};
use crate::application::ports::collect_sync_store::{
    BaselineStatus, CollectSyncAccountKey, CollectSyncStore, CollectSyncStoreError,
    CreateGenerationInput,
};
use crate::application::ports::daily_usage_export_store::{
    DailyUsageExportQuery, DailyUsageExportStore,
};

/// Secret-free upload status for IPC/Settings (chunk 04).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CollectSyncUiStatus {
    SignedOut,
    Idle,
    Syncing,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CollectSyncStatusSnapshot {
    pub status: CollectSyncUiStatus,
    pub last_accepted_at_ms: Option<i64>,
    pub last_error_code: Option<String>,
    pub last_error_message: Option<String>,
    pub last_error_retryable: Option<bool>,
}

pub(crate) trait CollectSyncStatusSink: Send + Sync {
    fn on_status_changed(&self, snapshot: CollectSyncStatusSnapshot);
}

pub(crate) struct NoopCollectSyncStatusSink;

impl CollectSyncStatusSink for NoopCollectSyncStatusSink {
    fn on_status_changed(&self, _snapshot: CollectSyncStatusSnapshot) {}
}

/// Successful daily targets committed by a refresh cycle.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct CommittedDailyUpload {
    pub targets: Vec<CommittedDailyTarget>,
    pub refresh_was_full: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CommittedDailyTarget {
    pub source_key: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    /// True when the collection scope was Full (all history for this source).
    pub full_history: bool,
}

impl CommittedDailyUpload {
    pub(crate) fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    pub(crate) fn into_upload_scope(self) -> Option<UploadScope> {
        if self.targets.is_empty() {
            return None;
        }
        if self.refresh_was_full
            || self.targets.iter().all(|target| target.full_history)
        {
            // Full-history successful targets: use Full when all sources succeeded
            // under full collection, otherwise wide incremental for the subset.
            if self.refresh_was_full && self.targets.iter().all(|target| target.full_history) {
                return Some(UploadScope::Full);
            }
        }

        let mut source_keys = std::collections::BTreeSet::new();
        let mut start = self.targets[0].start_date;
        let mut end = self.targets[0].end_date;
        for target in &self.targets {
            source_keys.insert(target.source_key.clone());
            start = start.min(target.start_date);
            end = end.max(target.end_date);
            if target.full_history {
                // Full history for a subset: open start bound.
                start = NaiveDate::from_ymd_opt(1970, 1, 1).unwrap_or(start);
            }
        }
        UploadScope::incremental(source_keys, start, end).ok()
    }
}

pub(crate) struct CollectSyncConfig {
    pub device_id: String,
    pub device_name: String,
    pub app_version: String,
    pub platform: CollectSyncPlatform,
    pub reporting_timezone: String,
}

pub(crate) struct CollectSync {
    session: Arc<CloudSession>,
    config: CollectSyncConfig,
    export_store: Arc<dyn DailyUsageExportStore>,
    collect_store: Arc<dyn CollectSyncStore>,
    remote: Arc<dyn CollectSyncRemote>,
    clock: Arc<dyn Clock>,
    status_sink: Arc<dyn CollectSyncStatusSink>,
    running: AtomicBool,
    cancel: AtomicBool,
    epoch: AtomicU64,
    active_user_id: Mutex<Option<String>>,
    next_retry_not_before_ms: Mutex<Option<i64>>,
}

impl CollectSync {
    pub(crate) fn new(
        session: Arc<CloudSession>,
        config: CollectSyncConfig,
        export_store: Arc<dyn DailyUsageExportStore>,
        collect_store: Arc<dyn CollectSyncStore>,
        remote: Arc<dyn CollectSyncRemote>,
        clock: Arc<dyn Clock>,
        status_sink: Arc<dyn CollectSyncStatusSink>,
    ) -> Arc<Self> {
        Arc::new(Self {
            session,
            config,
            export_store,
            collect_store,
            remote,
            clock,
            status_sink,
            running: AtomicBool::new(false),
            cancel: AtomicBool::new(false),
            epoch: AtomicU64::new(0),
            active_user_id: Mutex::new(None),
            next_retry_not_before_ms: Mutex::new(None),
        })
    }

    pub(crate) fn status_snapshot(&self) -> CollectSyncStatusSnapshot {
        let signed_in = matches!(
            self.session.snapshot(),
            Ok(SessionSnapshot::SignedIn { .. })
        );
        if !signed_in {
            return CollectSyncStatusSnapshot {
                status: CollectSyncUiStatus::SignedOut,
                last_accepted_at_ms: None,
                last_error_code: None,
                last_error_message: None,
                last_error_retryable: None,
            };
        }

        let account = match self.account_key() {
            Some(account) => account,
            None => {
                return CollectSyncStatusSnapshot {
                    status: CollectSyncUiStatus::SignedOut,
                    last_accepted_at_ms: None,
                    last_error_code: None,
                    last_error_message: None,
                    last_error_retryable: None,
                }
            }
        };

        let state = self.collect_store.load_state(&account).ok().flatten();
        let status = if self.running.load(Ordering::SeqCst) {
            CollectSyncUiStatus::Syncing
        } else if state
            .as_ref()
            .and_then(|value| value.last_error_code.as_ref())
            .is_some()
        {
            CollectSyncUiStatus::Error
        } else {
            CollectSyncUiStatus::Idle
        };

        CollectSyncStatusSnapshot {
            status,
            last_accepted_at_ms: state.as_ref().and_then(|value| value.last_accepted_at_ms),
            last_error_code: state.as_ref().and_then(|value| value.last_error_code.clone()),
            last_error_message: state
                .as_ref()
                .and_then(|value| value.last_error_message.clone()),
            last_error_retryable: state.as_ref().and_then(|value| value.last_error_retryable),
        }
    }

    pub(crate) fn on_signed_in(self: &Arc<Self>, user_id: &str) {
        {
            let mut guard = self.active_user_id.lock().expect("user lock");
            *guard = Some(user_id.to_owned());
        }
        self.cancel.store(false, Ordering::SeqCst);
        self.epoch.fetch_add(1, Ordering::SeqCst);
        *self.next_retry_not_before_ms.lock().expect("retry lock") = None;

        if let Some(account) = self.account_key() {
            let now = self.clock.now_epoch_ms();
            let _ = self.collect_store.ensure_state(&account, now);
            if let Ok(Some(state)) = self.collect_store.load_state(&account) {
                if matches!(
                    state.baseline_status,
                    BaselineStatus::None | BaselineStatus::InProgress
                ) {
                    let _ = self.collect_store.merge_pending_scope(
                        &account,
                        UploadScope::Full,
                        now,
                    );
                }
            }
        }
        self.kick();
    }

    pub(crate) fn on_signed_out(self: &Arc<Self>) {
        self.cancel.store(true, Ordering::SeqCst);
        self.epoch.fetch_add(1, Ordering::SeqCst);
        *self.active_user_id.lock().expect("user lock") = None;
        *self.next_retry_not_before_ms.lock().expect("retry lock") = None;
        self.publish_status();
    }

    pub(crate) fn on_committed_daily_upload(self: &Arc<Self>, upload: CommittedDailyUpload) {
        if upload.is_empty() {
            return;
        }
        let Some(scope) = upload.into_upload_scope() else {
            return;
        };
        let Some(account) = self.account_key() else {
            return;
        };
        let now = self.clock.now_epoch_ms();
        let _ = self.collect_store.merge_pending_scope(&account, scope, now);
        self.kick();
    }

    pub(crate) fn retry_now(self: &Arc<Self>) {
        *self.next_retry_not_before_ms.lock().expect("retry lock") = None;
        self.kick();
    }

    pub(crate) fn on_startup(self: &Arc<Self>) {
        if let Ok(SessionSnapshot::SignedIn { account }) = self.session.snapshot() {
            self.on_signed_in(&account.user_id);
        }
    }

    fn kick(self: &Arc<Self>) {
        if self
            .running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }
        self.cancel.store(false, Ordering::SeqCst);
        let this = Arc::clone(self);
        let epoch = this.epoch.load(Ordering::SeqCst);
        thread::Builder::new()
            .name("burnly-collect-sync".into())
            .spawn(move || {
                this.run_worker(epoch);
                this.running.store(false, Ordering::SeqCst);
                this.publish_status();
                // If work remains and not cancelled, schedule another pass.
                if !this.cancel.load(Ordering::SeqCst) && this.has_work() {
                    this.kick();
                }
            })
            .ok();
        self.publish_status();
    }

    fn has_work(&self) -> bool {
        let Some(account) = self.account_key() else {
            return false;
        };
        if self
            .collect_store
            .count_pending_batches(&account)
            .unwrap_or(0)
            > 0
        {
            return true;
        }
        matches!(
            self.collect_store.load_state(&account).ok().flatten(),
            Some(state) if state.pending_scope.is_some()
                || matches!(state.baseline_status, BaselineStatus::None | BaselineStatus::InProgress)
        )
    }

    fn run_worker(&self, epoch: u64) {
        self.publish_status();
        loop {
            if self.cancel.load(Ordering::SeqCst) || self.epoch.load(Ordering::SeqCst) != epoch {
                return;
            }
            let Some(account) = self.account_key_matching_epoch(epoch) else {
                return;
            };

            if let Some(not_before) = *self.next_retry_not_before_ms.lock().expect("retry lock") {
                if self.clock.now_epoch_ms() < not_before {
                    return;
                }
            }

            match self.step(&account, epoch) {
                StepResult::Continue => {}
                StepResult::Idle => return,
                StepResult::Backoff { until_ms } => {
                    *self.next_retry_not_before_ms.lock().expect("retry lock") = Some(until_ms);
                    return;
                }
                StepResult::Stop => return,
            }
        }
    }

    fn step(&self, account: &CollectSyncAccountKey, epoch: u64) -> StepResult {
        let now = self.clock.now_epoch_ms();
        let pending = self
            .collect_store
            .list_pending_batches(account)
            .unwrap_or_default();

        if let Some(batch) = pending.into_iter().next() {
            return self.send_batch(account, epoch, batch);
        }

        // Materialize next generation from pending scope / baseline need.
        let state = match self.collect_store.load_state(account) {
            Ok(Some(state)) => state,
            Ok(None) => {
                let _ = self.collect_store.ensure_state(account, now);
                return StepResult::Continue;
            }
            Err(_) => return StepResult::Idle,
        };

        let scope = match state.pending_scope.clone() {
            Some(scope) => scope,
            None if matches!(
                state.baseline_status,
                BaselineStatus::None | BaselineStatus::InProgress
            ) =>
            {
                UploadScope::Full
            }
            None => return StepResult::Idle,
        };

        if let Err(error) = self.ensure_device_registered(account, epoch, &state, now) {
            return self.map_remote_step_error(account, now, error);
        }

        match self.materialize_generation(account, scope, now) {
            Ok(0) => {
                // Empty export: clear pending and complete baseline if needed.
                let _ = self.collect_store.create_generation(CreateGenerationInput {
                    account: account.clone(),
                    generation_id: Uuid::new_v4().to_string(),
                    meta: self.batch_meta(UploadScope::Full),
                    prepared_batches: Vec::new(),
                    now_ms: now,
                    marks_baseline_in_progress: false,
                    clear_pending_scope: true,
                });
                if matches!(
                    state.baseline_status,
                    BaselineStatus::None | BaselineStatus::InProgress
                ) {
                    let _ = self.collect_store.mark_baseline_complete(account, now);
                }
                let _ = self
                    .collect_store
                    .record_attempt_result(account, now, None, None, None);
                StepResult::Continue
            }
            Ok(_) => StepResult::Continue,
            Err(StepError::Store) => StepResult::Idle,
            Err(StepError::Remote(error)) => self.map_remote_step_error(account, now, error),
            Err(StepError::Cancelled) => StepResult::Stop,
        }
    }

    fn materialize_generation(
        &self,
        account: &CollectSyncAccountKey,
        scope: UploadScope,
        now_ms: i64,
    ) -> Result<usize, StepError> {
        let query = match &scope {
            UploadScope::Full => DailyUsageExportQuery::full(self.config.reporting_timezone.clone()),
            UploadScope::Incremental {
                source_keys,
                start_date,
                end_date,
            } => DailyUsageExportQuery::incremental(
                self.config.reporting_timezone.clone(),
                source_keys.iter().cloned(),
                *start_date,
                *end_date,
            )
            .map_err(|_| StepError::Store)?,
        };

        let exported = self
            .export_store
            .export_daily_facts(&query)
            .map_err(|_| StepError::Store)?;
        let facts = exported
            .into_iter()
            .map(map_exported_fact)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| StepError::Store)?;

        let state = self
            .collect_store
            .load_state(account)
            .map_err(|_| StepError::Store)?
            .ok_or(StepError::Store)?;
        let meta = self.batch_meta(scope.clone());
        let prepared = build_prepared_batches(
            facts,
            &meta,
            BatchBuildLimits::backend_v1(),
            state.next_client_revision,
        )
        .map_err(|_| StepError::Store)?;
        let count = prepared.len();
        let marks_baseline = matches!(
            state.baseline_status,
            BaselineStatus::None | BaselineStatus::InProgress
        ) && matches!(scope, UploadScope::Full);

        self.collect_store
            .create_generation(CreateGenerationInput {
                account: account.clone(),
                generation_id: Uuid::new_v4().to_string(),
                meta,
                prepared_batches: prepared,
                now_ms,
                marks_baseline_in_progress: marks_baseline,
                clear_pending_scope: true,
            })
            .map_err(|error| match error {
                CollectSyncStoreError::PendingGenerationExists => StepError::Store,
                _ => StepError::Store,
            })?;
        Ok(count)
    }

    fn send_batch(
        &self,
        account: &CollectSyncAccountKey,
        epoch: u64,
        batch: crate::application::ports::collect_sync_store::OutboxBatch,
    ) -> StepResult {
        if self.epoch.load(Ordering::SeqCst) != epoch || self.cancel.load(Ordering::SeqCst) {
            return StepResult::Stop;
        }
        // Re-check account identity before network I/O.
        let Some(current) = self.account_key() else {
            return StepResult::Stop;
        };
        if current != *account {
            return StepResult::Stop;
        }

        let now = self.clock.now_epoch_ms();
        let state = self.collect_store.load_state(account).ok().flatten();
        if let Some(state) = state.as_ref() {
            if let Err(error) = self.ensure_device_registered(account, epoch, state, now) {
                return self.map_remote_step_error(account, now, error);
            }
        }

        match self.remote.push_daily_usage(PushDailyUsageRequest {
            request_body: batch.request_body.clone(),
            idempotency_key: batch.idempotency_key.clone(),
        }) {
            Ok(_result) => {
                let accepted_at = self.clock.now_epoch_ms();
                if self
                    .collect_store
                    .mark_batch_accepted(account, batch.id, accepted_at)
                    .is_err()
                {
                    return StepResult::Idle;
                }
                let remaining = self
                    .collect_store
                    .count_pending_batches(account)
                    .unwrap_or(1);
                if remaining == 0 {
                    if let Ok(Some(state)) = self.collect_store.load_state(account) {
                        if matches!(state.baseline_status, BaselineStatus::InProgress) {
                            let _ = self
                                .collect_store
                                .mark_baseline_complete(account, accepted_at);
                        }
                    }
                }
                let _ = self.collect_store.record_attempt_result(
                    account,
                    accepted_at,
                    None,
                    None,
                    None,
                );
                self.publish_status();
                StepResult::Continue
            }
            Err(CollectSyncRemoteError::DeviceNotFound { .. }) => {
                // One recovery path: re-register device then let next step retry same batch.
                if let Ok(Some(state)) = self.collect_store.load_state(account) {
                    let _ = self.collect_store.set_device_registration(
                        account,
                        "invalidate",
                        0,
                        now,
                    );
                    if let Err(error) =
                        self.force_device_register(account, epoch, &state, now)
                    {
                        return self.map_remote_step_error(account, now, error);
                    }
                }
                StepResult::Continue
            }
            Err(error) => self.map_remote_step_error(account, now, error),
        }
    }

    fn ensure_device_registered(
        &self,
        account: &CollectSyncAccountKey,
        epoch: u64,
        state: &crate::application::ports::collect_sync_store::CollectSyncState,
        now_ms: i64,
    ) -> Result<(), CollectSyncRemoteError> {
        let fingerprint = self.device_fingerprint();
        if state.device_metadata_fingerprint.as_deref() == Some(fingerprint.as_str())
            && state.device_registered_revision.is_some()
        {
            return Ok(());
        }
        self.force_device_register(account, epoch, state, now_ms)
    }

    fn force_device_register(
        &self,
        account: &CollectSyncAccountKey,
        epoch: u64,
        _state: &crate::application::ports::collect_sync_store::CollectSyncState,
        now_ms: i64,
    ) -> Result<(), CollectSyncRemoteError> {
        if self.epoch.load(Ordering::SeqCst) != epoch {
            return Err(CollectSyncRemoteError::Internal {
                message: "cancelled".into(),
            });
        }
        let snapshot = self.remote.upsert_device(UpsertSyncDeviceRequest {
            client_device_id: account.client_device_id.clone(),
            display_name: Some(self.config.device_name.clone()),
            platform: self.config.platform,
            app_version: self.config.app_version.clone(),
            reporting_timezone: self.config.reporting_timezone.clone(),
        })?;
        let fingerprint = self.device_fingerprint();
        let _ = self.collect_store.set_device_registration(
            account,
            &fingerprint,
            1,
            now_ms,
        );
        let _ = snapshot;
        Ok(())
    }

    fn device_fingerprint(&self) -> String {
        let raw = format!(
            "{}|{}|{}|{}",
            self.config.device_name,
            self.config.platform.as_str(),
            self.config.app_version,
            self.config.reporting_timezone
        );
        format!("{:x}", Sha256::digest(raw.as_bytes()))
    }

    fn batch_meta(&self, scope: UploadScope) -> BatchRequestMeta {
        BatchRequestMeta {
            client_device_id: self.config.device_id.clone(),
            app_version: self.config.app_version.clone(),
            reporting_timezone: self.config.reporting_timezone.clone(),
            scope,
        }
    }

    fn map_remote_step_error(
        &self,
        account: &CollectSyncAccountKey,
        now_ms: i64,
        error: CollectSyncRemoteError,
    ) -> StepResult {
        let (code, message, retryable, backoff_ms) = classify_remote_error(&error);
        let _ = self.collect_store.record_attempt_result(
            account,
            now_ms,
            Some(code),
            Some(&message),
            Some(retryable),
        );
        self.publish_status();
        if retryable {
            StepResult::Backoff {
                until_ms: now_ms.saturating_add(backoff_ms),
            }
        } else {
            StepResult::Idle
        }
    }

    fn account_key(&self) -> Option<CollectSyncAccountKey> {
        let SessionSnapshot::SignedIn { account } = self.session.snapshot().ok()? else {
            return None;
        };
        let device_id = self.config.device_id.trim();
        if device_id.is_empty() {
            return None;
        }
        Some(CollectSyncAccountKey {
            user_id: account.user_id,
            client_device_id: device_id.to_owned(),
        })
    }

    fn account_key_matching_epoch(&self, epoch: u64) -> Option<CollectSyncAccountKey> {
        if self.epoch.load(Ordering::SeqCst) != epoch {
            return None;
        }
        let key = self.account_key()?;
        let active = self.active_user_id.lock().expect("user lock").clone()?;
        if active != key.user_id {
            return None;
        }
        Some(key)
    }

    fn publish_status(&self) {
        self.status_sink
            .on_status_changed(self.status_snapshot());
    }
}

enum StepResult {
    Continue,
    Idle,
    Backoff { until_ms: i64 },
    Stop,
}

enum StepError {
    Store,
    Remote(CollectSyncRemoteError),
    Cancelled,
}

fn classify_remote_error(error: &CollectSyncRemoteError) -> (&'static str, String, bool, i64) {
    match error {
        CollectSyncRemoteError::Network { message } => {
            ("NETWORK", message.clone(), true, 5_000)
        }
        CollectSyncRemoteError::Timeout { message } => {
            ("TIMEOUT", message.clone(), true, 5_000)
        }
        CollectSyncRemoteError::RateLimited {
            message,
            retry_after_seconds,
            ..
        } => {
            let backoff = retry_after_seconds
                .map(|seconds| i64::try_from(seconds).unwrap_or(60).saturating_mul(1_000))
                .unwrap_or(15_000);
            ("RATE_LIMITED", message.clone(), true, backoff)
        }
        CollectSyncRemoteError::IdempotencyInProgress { message } => {
            ("IDEMPOTENCY_IN_PROGRESS", message.clone(), true, 2_000)
        }
        CollectSyncRemoteError::Unauthorized { message, code } => (
            "UNAUTHORIZED",
            format!("{} ({})", message, code.as_deref().unwrap_or("?")),
            false,
            0,
        ),
        CollectSyncRemoteError::Validation { message, .. } => {
            ("VALIDATION_FAILED", message.clone(), false, 0)
        }
        CollectSyncRemoteError::ContractUnsupported { message } => {
            ("SYNC_CONTRACT_UNSUPPORTED", message.clone(), false, 0)
        }
        CollectSyncRemoteError::DeviceNotFound { message } => {
            ("SYNC_DEVICE_NOT_FOUND", message.clone(), true, 1_000)
        }
        CollectSyncRemoteError::Conflict { message, .. } => {
            ("CONFLICT", message.clone(), false, 0)
        }
        CollectSyncRemoteError::PayloadTooLarge { message } => {
            ("PAYLOAD_TOO_LARGE", message.clone(), false, 0)
        }
        CollectSyncRemoteError::Forbidden { message, .. } => {
            ("FORBIDDEN", message.clone(), false, 0)
        }
        CollectSyncRemoteError::Problem {
            message, code, ..
        } => {
            let retryable = code.as_deref().is_none_or(|value| {
                !matches!(
                    value,
                    "VALIDATION_FAILED" | "SYNC_CONTRACT_UNSUPPORTED" | "FORBIDDEN"
                )
            });
            (
                "PROBLEM",
                message.clone(),
                retryable,
                if retryable { 5_000 } else { 0 },
            )
        }
        CollectSyncRemoteError::Decode { message } => ("DECODE", message.clone(), false, 0),
        CollectSyncRemoteError::Internal { message } => {
            ("INTERNAL", message.clone(), true, 5_000)
        }
    }
}

// Keep sleep helper available for future bounded waits without blocking refresh.
#[allow(dead_code)]
fn sleep_ms(ms: u64) {
    thread::sleep(Duration::from_millis(ms));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::cloud_session::{AccountSummary, CloudTokens};
    use crate::application::collect_sync::{ExportedDailyFact, ExportedDailyModel};
    use crate::application::ports::cloud_remote_logout::CloudRemoteLogout;
    use crate::application::ports::cloud_token_refresher::CloudTokenRefresher;
    use crate::application::ports::cloud_token_store::{
        CloudTokenStore, CloudTokenStoreError, StoredCloudSession,
    };
    use crate::application::ports::collect_sync_remote::{
        DailyUsagePushCounts, DailyUsagePushResult, SyncDeviceSnapshot,
    };
    use crate::application::ports::collect_sync_store::{
        CollectSyncState, CreateGenerationResult, OutboxBatch, OutboxBatchStatus,
    };
    use crate::application::ports::daily_usage_export_store::DailyUsageExportStoreError;
    use std::collections::HashMap;
    use std::sync::Mutex as StdMutex;

    struct FixedClock(StdMutex<i64>);
    impl Clock for FixedClock {
        fn now_epoch_ms(&self) -> i64 {
            *self.0.lock().expect("clock")
        }
    }

    struct MemoryTokenStore(StdMutex<Option<StoredCloudSession>>);
    impl CloudTokenStore for MemoryTokenStore {
        fn load(&self) -> Result<Option<StoredCloudSession>, CloudTokenStoreError> {
            Ok(self.0.lock().expect("store").clone())
        }
        fn save(&self, session: &StoredCloudSession) -> Result<(), CloudTokenStoreError> {
            *self.0.lock().expect("store") = Some(session.clone());
            Ok(())
        }
        fn clear(&self) -> Result<(), CloudTokenStoreError> {
            *self.0.lock().expect("store") = None;
            Ok(())
        }
    }

    struct NoopRefresher;
    impl CloudTokenRefresher for NoopRefresher {
        fn refresh(
            &self,
            _refresh_token: &str,
        ) -> Result<CloudTokens, crate::application::cloud_session::CloudSessionError> {
            Err(crate::application::cloud_session::CloudSessionError::NotSignedIn)
        }
    }

    struct NoopLogout;
    impl CloudRemoteLogout for NoopLogout {
        fn logout_remote(
            &self,
            _refresh_token: &str,
        ) -> Result<(), crate::application::cloud_session::CloudSessionError> {
            Ok(())
        }
    }

    fn signed_in_session(user_id: &str) -> Arc<CloudSession> {
        let store = Arc::new(MemoryTokenStore(StdMutex::new(None)));
        let session = Arc::new(CloudSession::new(
            store,
            Arc::new(NoopRefresher),
            Arc::new(NoopLogout),
            Arc::new(FixedClock(StdMutex::new(1_000))),
        ));
        session
            .apply_tokens(
                CloudTokens {
                    access_token: "access".into(),
                    refresh_token: "refresh".into(),
                    access_expires_at_ms: Some(9_999_999_999),
                },
                AccountSummary {
                    user_id: user_id.into(),
                    email: "u@example.com".into(),
                },
            )
            .expect("apply");
        session
    }

    struct MemoryCollectStore {
        states: StdMutex<HashMap<(String, String), CollectSyncState>>,
        batches: StdMutex<Vec<OutboxBatch>>,
        next_id: StdMutex<i64>,
    }

    impl MemoryCollectStore {
        fn new() -> Self {
            Self {
                states: StdMutex::new(HashMap::new()),
                batches: StdMutex::new(Vec::new()),
                next_id: StdMutex::new(1),
            }
        }
    }

    impl CollectSyncStore for MemoryCollectStore {
        fn load_state(
            &self,
            account: &CollectSyncAccountKey,
        ) -> Result<Option<CollectSyncState>, CollectSyncStoreError> {
            Ok(self
                .states
                .lock()
                .expect("states")
                .get(&(account.user_id.clone(), account.client_device_id.clone()))
                .cloned())
        }

        fn ensure_state(
            &self,
            account: &CollectSyncAccountKey,
            now_ms: i64,
        ) -> Result<CollectSyncState, CollectSyncStoreError> {
            let _ = now_ms;
            let mut states = self.states.lock().expect("states");
            let key = (account.user_id.clone(), account.client_device_id.clone());
            let state = states.entry(key).or_insert_with(|| CollectSyncState {
                account: account.clone(),
                next_client_revision: 1,
                baseline_status: BaselineStatus::None,
                pending_scope: None,
                active_generation_id: None,
                last_attempt_at_ms: None,
                last_accepted_at_ms: None,
                last_error_code: None,
                last_error_message: None,
                last_error_retryable: None,
                device_metadata_fingerprint: None,
                device_registered_revision: None,
            });
            state.account = account.clone();
            Ok(state.clone())
        }

        fn merge_pending_scope(
            &self,
            account: &CollectSyncAccountKey,
            scope: UploadScope,
            now_ms: i64,
        ) -> Result<UploadScope, CollectSyncStoreError> {
            let _ = self.ensure_state(account, now_ms)?;
            let mut states = self.states.lock().expect("states");
            let state = states
                .get_mut(&(account.user_id.clone(), account.client_device_id.clone()))
                .ok_or(CollectSyncStoreError::NotFound)?;
            let merged = crate::application::collect_sync::merge_upload_scopes(
                state.pending_scope.clone(),
                scope,
            );
            state.pending_scope = Some(merged.clone());
            Ok(merged)
        }

        fn create_generation(
            &self,
            input: CreateGenerationInput,
        ) -> Result<CreateGenerationResult, CollectSyncStoreError> {
            if self.count_pending_batches(&input.account)? > 0 {
                return Err(CollectSyncStoreError::PendingGenerationExists);
            }
            let _ = self.ensure_state(&input.account, input.now_ms)?;
            let mut states = self.states.lock().expect("states");
            let state = states
                .get_mut(&(
                    input.account.user_id.clone(),
                    input.account.client_device_id.clone(),
                ))
                .ok_or(CollectSyncStoreError::NotFound)?;
            let mut batches = self.batches.lock().expect("batches");
            let mut next_id = self.next_id.lock().expect("id");
            let mut created = Vec::new();
            for prepared in &input.prepared_batches {
                let batch = OutboxBatch {
                    id: *next_id,
                    account: input.account.clone(),
                    generation_id: input.generation_id.clone(),
                    batch_index: prepared.batch_index,
                    batch_count: prepared.batch_count,
                    client_revision: prepared.client_revision,
                    idempotency_key: prepared.idempotency_key.clone(),
                    request_body: prepared.request_body.clone(),
                    payload_hash: prepared.payload_hash.clone(),
                    window_scope: prepared.window_scope,
                    window_start: prepared.window_start.clone(),
                    window_end: prepared.window_end.clone(),
                    status: OutboxBatchStatus::Pending,
                    created_at_ms: input.now_ms,
                    accepted_at_ms: None,
                };
                *next_id += 1;
                created.push(batch.clone());
                batches.push(batch);
            }
            state.next_client_revision += i64::try_from(input.prepared_batches.len()).unwrap_or(0);
            if input.marks_baseline_in_progress {
                state.baseline_status = BaselineStatus::InProgress;
            }
            if input.clear_pending_scope {
                state.pending_scope = None;
            }
            if !created.is_empty() {
                state.active_generation_id = Some(input.generation_id);
            }
            Ok(CreateGenerationResult {
                state: state.clone(),
                batches: created,
            })
        }

        fn list_pending_batches(
            &self,
            account: &CollectSyncAccountKey,
        ) -> Result<Vec<OutboxBatch>, CollectSyncStoreError> {
            Ok(self
                .batches
                .lock()
                .expect("batches")
                .iter()
                .filter(|batch| {
                    batch.account == *account && batch.status == OutboxBatchStatus::Pending
                })
                .cloned()
                .collect())
        }

        fn mark_batch_accepted(
            &self,
            account: &CollectSyncAccountKey,
            batch_id: i64,
            accepted_at_ms: i64,
        ) -> Result<OutboxBatch, CollectSyncStoreError> {
            let mut batches = self.batches.lock().expect("batches");
            let batch = batches
                .iter_mut()
                .find(|batch| batch.id == batch_id && batch.account == *account)
                .ok_or(CollectSyncStoreError::NotFound)?;
            batch.status = OutboxBatchStatus::Accepted;
            batch.accepted_at_ms = Some(accepted_at_ms);
            let out = batch.clone();
            drop(batches);
            let mut states = self.states.lock().expect("states");
            if let Some(state) = states.get_mut(&(account.user_id.clone(), account.client_device_id.clone()))
            {
                state.last_accepted_at_ms = Some(accepted_at_ms);
            }
            Ok(out)
        }

        fn count_pending_batches(
            &self,
            account: &CollectSyncAccountKey,
        ) -> Result<u32, CollectSyncStoreError> {
            Ok(self.list_pending_batches(account)?.len() as u32)
        }

        fn record_attempt_result(
            &self,
            account: &CollectSyncAccountKey,
            now_ms: i64,
            error_code: Option<&str>,
            error_message: Option<&str>,
            retryable: Option<bool>,
        ) -> Result<(), CollectSyncStoreError> {
            let _ = self.ensure_state(account, now_ms)?;
            let mut states = self.states.lock().expect("states");
            let state = states
                .get_mut(&(account.user_id.clone(), account.client_device_id.clone()))
                .ok_or(CollectSyncStoreError::NotFound)?;
            state.last_attempt_at_ms = Some(now_ms);
            state.last_error_code = error_code.map(str::to_owned);
            state.last_error_message = error_message.map(str::to_owned);
            state.last_error_retryable = retryable;
            Ok(())
        }

        fn mark_baseline_complete(
            &self,
            account: &CollectSyncAccountKey,
            now_ms: i64,
        ) -> Result<(), CollectSyncStoreError> {
            let _ = self.ensure_state(account, now_ms)?;
            let mut states = self.states.lock().expect("states");
            let state = states
                .get_mut(&(account.user_id.clone(), account.client_device_id.clone()))
                .ok_or(CollectSyncStoreError::NotFound)?;
            state.baseline_status = BaselineStatus::Complete;
            Ok(())
        }

        fn set_device_registration(
            &self,
            account: &CollectSyncAccountKey,
            fingerprint: &str,
            registered_revision: i64,
            now_ms: i64,
        ) -> Result<(), CollectSyncStoreError> {
            let _ = self.ensure_state(account, now_ms)?;
            let mut states = self.states.lock().expect("states");
            let state = states
                .get_mut(&(account.user_id.clone(), account.client_device_id.clone()))
                .ok_or(CollectSyncStoreError::NotFound)?;
            state.device_metadata_fingerprint = Some(fingerprint.to_owned());
            state.device_registered_revision = Some(registered_revision);
            Ok(())
        }
    }

    struct EmptyExport;
    impl DailyUsageExportStore for EmptyExport {
        fn export_daily_facts(
            &self,
            _query: &DailyUsageExportQuery,
        ) -> Result<Vec<ExportedDailyFact>, DailyUsageExportStoreError> {
            Ok(Vec::new())
        }
    }

    struct OneFactExport;
    impl DailyUsageExportStore for OneFactExport {
        fn export_daily_facts(
            &self,
            _query: &DailyUsageExportQuery,
        ) -> Result<Vec<ExportedDailyFact>, DailyUsageExportStoreError> {
            Ok(vec![ExportedDailyFact {
                identity_key: "claude-code:daily:v1:UTC:2026-07-08".into(),
                identity_version: 1,
                source_key: "claude-code".into(),
                usage_date: "2026-07-08".into(),
                aggregation_timezone: "UTC".into(),
                input_tokens: Some(1),
                output_tokens: Some(1),
                cache_creation_tokens: Some(0),
                cache_read_tokens: Some(0),
                total_tokens: 2,
                unclassified_tokens: Some(0),
                cost_status: "unavailable".into(),
                cost_kind: "unknown".into(),
                cost_amount_micros: None,
                cost_currency: None,
                data_quality: "complete".into(),
                record_state: "active".into(),
                first_seen_at_ms: 1,
                last_seen_at_ms: 2,
                removed_at_ms: None,
                models: vec![ExportedDailyModel {
                    raw_model_id: Some("m".into()),
                    display_name: None,
                    provider_key: None,
                    input_tokens: Some(1),
                    output_tokens: Some(1),
                    cache_creation_tokens: Some(0),
                    cache_read_tokens: Some(0),
                    total_tokens: Some(2),
                    cost_status: "unavailable".into(),
                    cost_amount_micros: None,
                    cost_currency: None,
                }],
            }])
        }
    }

    struct ScriptedRemote {
        put_calls: StdMutex<u32>,
        push_calls: StdMutex<u32>,
        push_ok: bool,
        /// When true, first push returns device-not-found once.
        first_push_device_missing: StdMutex<bool>,
        last_push_keys: StdMutex<Vec<String>>,
        last_push_bodies: StdMutex<Vec<String>>,
    }

    impl ScriptedRemote {
        fn ok() -> Self {
            Self {
                put_calls: StdMutex::new(0),
                push_calls: StdMutex::new(0),
                push_ok: true,
                first_push_device_missing: StdMutex::new(false),
                last_push_keys: StdMutex::new(Vec::new()),
                last_push_bodies: StdMutex::new(Vec::new()),
            }
        }

        fn network_fail() -> Self {
            Self {
                put_calls: StdMutex::new(0),
                push_calls: StdMutex::new(0),
                push_ok: false,
                first_push_device_missing: StdMutex::new(false),
                last_push_keys: StdMutex::new(Vec::new()),
                last_push_bodies: StdMutex::new(Vec::new()),
            }
        }
    }

    impl CollectSyncRemote for ScriptedRemote {
        fn upsert_device(
            &self,
            request: UpsertSyncDeviceRequest,
        ) -> Result<SyncDeviceSnapshot, CollectSyncRemoteError> {
            *self.put_calls.lock().expect("put") += 1;
            Ok(SyncDeviceSnapshot {
                client_device_id: request.client_device_id,
                display_name: request.display_name,
                platform: request.platform.as_str().into(),
                app_version: request.app_version,
                reporting_timezone: request.reporting_timezone,
                last_sync_at: None,
                created_at: "t".into(),
                updated_at: "t".into(),
            })
        }

        fn push_daily_usage(
            &self,
            request: PushDailyUsageRequest,
        ) -> Result<DailyUsagePushResult, CollectSyncRemoteError> {
            *self.push_calls.lock().expect("push") += 1;
            self.last_push_keys
                .lock()
                .expect("keys")
                .push(request.idempotency_key.clone());
            self.last_push_bodies
                .lock()
                .expect("bodies")
                .push(request.request_body.clone());
            {
                let mut missing = self.first_push_device_missing.lock().expect("missing");
                if *missing {
                    *missing = false;
                    return Err(CollectSyncRemoteError::DeviceNotFound {
                        message: "missing".into(),
                    });
                }
            }
            if self.push_ok {
                Ok(DailyUsagePushResult {
                    client_device_id: "dev_1".into(),
                    accepted_at: "2026-07-09T12:00:00.000Z".into(),
                    client_revision: 1,
                    window_start: "2026-07-08".into(),
                    window_end: "2026-07-08".into(),
                    window_scope: crate::application::collect_sync::WireUploadScope::Full,
                    counts: DailyUsagePushCounts {
                        received: 1,
                        upserted: 1,
                        removed: 0,
                        unchanged: 0,
                        rejected: 0,
                    },
                })
            } else {
                Err(CollectSyncRemoteError::Network {
                    message: "down".into(),
                })
            }
        }
    }

    fn wait_idle(service: &CollectSync, max_iters: usize) {
        for _ in 0..max_iters {
            thread::sleep(Duration::from_millis(10));
            if !service.running.load(Ordering::SeqCst) {
                return;
            }
        }
    }

    fn account_key(user: &str) -> CollectSyncAccountKey {
        CollectSyncAccountKey {
            user_id: user.into(),
            client_device_id: "dev_1".into(),
        }
    }

    fn config() -> CollectSyncConfig {
        CollectSyncConfig {
            device_id: "dev_1".into(),
            device_name: "host".into(),
            app_version: "0.1.20".into(),
            platform: CollectSyncPlatform::Linux,
            reporting_timezone: "UTC".into(),
        }
    }

    #[test]
    fn sign_in_without_data_completes_baseline() {
        let session = signed_in_session("user-a");
        let store = Arc::new(MemoryCollectStore::new());
        let remote = Arc::new(ScriptedRemote::ok());
        let service = CollectSync::new(
            session,
            config(),
            Arc::new(EmptyExport),
            store.clone(),
            remote.clone(),
            Arc::new(FixedClock(StdMutex::new(1_000))),
            Arc::new(NoopCollectSyncStatusSink),
        );
        service.on_signed_in("user-a");
        wait_idle(&service, 50);
        let state = store
            .load_state(&account_key("user-a"))
            .expect("load")
            .expect("state");
        assert_eq!(state.baseline_status, BaselineStatus::Complete);
        assert_eq!(*remote.push_calls.lock().expect("push"), 0);
        assert!(*remote.put_calls.lock().expect("put") >= 1);
    }

    #[test]
    fn baseline_with_facts_puts_device_and_pushes() {
        let session = signed_in_session("user-a");
        let store = Arc::new(MemoryCollectStore::new());
        let remote = Arc::new(ScriptedRemote::ok());
        let service = CollectSync::new(
            session,
            config(),
            Arc::new(OneFactExport),
            store.clone(),
            remote.clone(),
            Arc::new(FixedClock(StdMutex::new(1_000))),
            Arc::new(NoopCollectSyncStatusSink),
        );
        service.on_signed_in("user-a");
        for _ in 0..100 {
            thread::sleep(Duration::from_millis(10));
            if !service.running.load(Ordering::SeqCst)
                && store.count_pending_batches(&account_key("user-a")).unwrap_or(1) == 0
            {
                break;
            }
        }
        assert!(*remote.put_calls.lock().expect("put") >= 1);
        assert!(*remote.push_calls.lock().expect("push") >= 1);
        let state = store
            .load_state(&account_key("user-a"))
            .expect("load")
            .expect("state");
        assert_eq!(state.baseline_status, BaselineStatus::Complete);
        assert!(state.last_accepted_at_ms.is_some());
    }

    #[test]
    fn signed_out_status_and_no_work() {
        let store = Arc::new(MemoryTokenStore(StdMutex::new(None)));
        let session = Arc::new(CloudSession::new(
            store,
            Arc::new(NoopRefresher),
            Arc::new(NoopLogout),
            Arc::new(FixedClock(StdMutex::new(1))),
        ));
        let remote = Arc::new(ScriptedRemote::ok());
        let service = CollectSync::new(
            session,
            config(),
            Arc::new(OneFactExport),
            Arc::new(MemoryCollectStore::new()),
            remote.clone(),
            Arc::new(FixedClock(StdMutex::new(1))),
            Arc::new(NoopCollectSyncStatusSink),
        );
        assert_eq!(
            service.status_snapshot().status,
            CollectSyncUiStatus::SignedOut
        );
        service.on_committed_daily_upload(CommittedDailyUpload {
            targets: vec![CommittedDailyTarget {
                source_key: "claude-code".into(),
                start_date: NaiveDate::from_ymd_opt(2026, 7, 8).expect("d"),
                end_date: NaiveDate::from_ymd_opt(2026, 7, 8).expect("d"),
                full_history: false,
            }],
            refresh_was_full: false,
        });
        wait_idle(&service, 20);
        assert_eq!(*remote.push_calls.lock().expect("push"), 0);
    }

    #[test]
    fn network_failure_preserves_pending_batch_for_retry_same_key() {
        let session = signed_in_session("user-a");
        let store = Arc::new(MemoryCollectStore::new());
        let remote = Arc::new(ScriptedRemote::network_fail());
        let service = CollectSync::new(
            session,
            config(),
            Arc::new(OneFactExport),
            store.clone(),
            remote.clone(),
            Arc::new(FixedClock(StdMutex::new(1_000))),
            Arc::new(NoopCollectSyncStatusSink),
        );
        service.on_signed_in("user-a");
        wait_idle(&service, 80);
        let pending = store
            .list_pending_batches(&account_key("user-a"))
            .expect("list");
        assert_eq!(pending.len(), 1);
        let original_key = pending[0].idempotency_key.clone();
        let original_body = pending[0].request_body.clone();
        assert!(*remote.push_calls.lock().expect("push") >= 1);

        // "Restart": new service over the same durable store resumes exact batch.
        let session2 = signed_in_session("user-a");
        let remote2 = Arc::new(ScriptedRemote::ok());
        let service2 = CollectSync::new(
            session2,
            config(),
            Arc::new(EmptyExport),
            store.clone(),
            remote2.clone(),
            Arc::new(FixedClock(StdMutex::new(2_000))),
            Arc::new(NoopCollectSyncStatusSink),
        );
        service2.on_signed_in("user-a");
        for _ in 0..100 {
            thread::sleep(Duration::from_millis(10));
            if store.count_pending_batches(&account_key("user-a")).unwrap_or(1) == 0 {
                break;
            }
        }
        let keys = remote2.last_push_keys.lock().expect("keys");
        let bodies = remote2.last_push_bodies.lock().expect("bodies");
        assert_eq!(keys.as_slice(), &[original_key]);
        assert_eq!(bodies.as_slice(), &[original_body]);
        assert_eq!(store.count_pending_batches(&account_key("user-a")).unwrap(), 0);
    }

    #[test]
    fn account_switch_does_not_drain_other_user_pending() {
        let store = Arc::new(MemoryCollectStore::new());
        // Seed user-a pending batch without network.
        {
            let session = signed_in_session("user-a");
            let remote = Arc::new(ScriptedRemote::network_fail());
            let service = CollectSync::new(
                session,
                config(),
                Arc::new(OneFactExport),
                store.clone(),
                remote,
                Arc::new(FixedClock(StdMutex::new(1_000))),
                Arc::new(NoopCollectSyncStatusSink),
            );
            service.on_signed_in("user-a");
            wait_idle(&service, 80);
            assert_eq!(store.count_pending_batches(&account_key("user-a")).unwrap(), 1);
        }

        let session_b = signed_in_session("user-b");
        let remote_b = Arc::new(ScriptedRemote::ok());
        let service_b = CollectSync::new(
            session_b,
            config(),
            Arc::new(EmptyExport),
            store.clone(),
            remote_b.clone(),
            Arc::new(FixedClock(StdMutex::new(3_000))),
            Arc::new(NoopCollectSyncStatusSink),
        );
        service_b.on_signed_in("user-b");
        wait_idle(&service_b, 50);
        // User-a pending must remain; user-b empty export should not push a-batch.
        assert_eq!(store.count_pending_batches(&account_key("user-a")).unwrap(), 1);
        assert_eq!(*remote_b.push_calls.lock().expect("push"), 0);
    }

    #[test]
    fn sign_out_stops_further_pushes() {
        let session = signed_in_session("user-a");
        let store = Arc::new(MemoryCollectStore::new());
        let remote = Arc::new(ScriptedRemote::network_fail());
        let service = CollectSync::new(
            session.clone(),
            config(),
            Arc::new(OneFactExport),
            store.clone(),
            remote.clone(),
            Arc::new(FixedClock(StdMutex::new(1_000))),
            Arc::new(NoopCollectSyncStatusSink),
        );
        service.on_signed_in("user-a");
        wait_idle(&service, 80);
        let pushes_before = *remote.push_calls.lock().expect("push");
        assert!(pushes_before >= 1);
        service.on_signed_out();
        let _ = session.clear_local();
        service.retry_now();
        wait_idle(&service, 30);
        assert_eq!(*remote.push_calls.lock().expect("push"), pushes_before);
        assert_eq!(
            service.status_snapshot().status,
            CollectSyncUiStatus::SignedOut
        );
    }

    #[test]
    fn device_not_found_triggers_put_then_reuses_same_push_key() {
        let session = signed_in_session("user-a");
        let store = Arc::new(MemoryCollectStore::new());
        let remote = Arc::new(ScriptedRemote {
            put_calls: StdMutex::new(0),
            push_calls: StdMutex::new(0),
            push_ok: true,
            first_push_device_missing: StdMutex::new(true),
            last_push_keys: StdMutex::new(Vec::new()),
            last_push_bodies: StdMutex::new(Vec::new()),
        });
        let service = CollectSync::new(
            session,
            config(),
            Arc::new(OneFactExport),
            store.clone(),
            remote.clone(),
            Arc::new(FixedClock(StdMutex::new(1_000))),
            Arc::new(NoopCollectSyncStatusSink),
        );
        service.on_signed_in("user-a");
        for _ in 0..120 {
            thread::sleep(Duration::from_millis(10));
            if store.count_pending_batches(&account_key("user-a")).unwrap_or(1) == 0 {
                break;
            }
        }
        let keys = remote.last_push_keys.lock().expect("keys");
        assert!(keys.len() >= 2, "expected device-missing retry push");
        assert_eq!(keys[0], keys[1], "same idempotency key on recovery push");
        assert!(*remote.put_calls.lock().expect("put") >= 2);
    }

    #[test]
    fn committed_partial_scope_is_incremental_not_full() {
        let upload = CommittedDailyUpload {
            targets: vec![
                CommittedDailyTarget {
                    source_key: "claude-code".into(),
                    start_date: NaiveDate::from_ymd_opt(2026, 7, 8).expect("d"),
                    end_date: NaiveDate::from_ymd_opt(2026, 7, 8).expect("d"),
                    full_history: false,
                },
                // Failed source omitted: only successful targets enter the upload.
            ],
            refresh_was_full: false,
        };
        let scope = upload.into_upload_scope().expect("scope");
        assert!(matches!(
            scope,
            UploadScope::Incremental {
                ref source_keys,
                ..
            } if source_keys.len() == 1 && source_keys.contains("claude-code")
        ));
    }
}
