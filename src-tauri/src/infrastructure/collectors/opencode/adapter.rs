//! Native OpenCode collector orchestration.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::UNIX_EPOCH;

use chrono::{NaiveDate, NaiveTime, TimeZone, Utc};
use chrono_tz::Tz;

use crate::application::collection::{
    CollectionProjection, CollectionRequest, CollectionResult, CollectionScope,
    CollectorDescriptor, CollectorFailure, CollectorFailureCode, CollectorIntegrity,
    DailyUsageCandidate, DetectionRequest, DetectionResult, RejectedRecord, SessionUsageCandidate,
};
use crate::application::cost::BurnlyCostCalculator;
use crate::application::diagnostics::DiagnosticSeverity;
use crate::application::ports::collector::{CancellationSignal, Collector};
use crate::application::ports::diagnostic_recorder::DiagnosticRecorder;
use crate::application::ports::opencode_usage_ledger::{
    OpenCodeActivityBounds, OpenCodeExactOrigin, OpenCodeExactUsage, OpenCodeLedgerReconcileResult,
    OpenCodeLedgerRecord, OpenCodeReconciliationState, OpenCodeRecoveryDisposition,
    OpenCodeRedundancyProof, OpenCodeSessionCheckpoint, OpenCodeSessionLedgerSnapshot,
    OpenCodeTokenVector, OpenCodeUsageLedger, OpenCodeUsageLedgerError,
};
use crate::domain::source::SourceKey;

use super::super::support::{
    available_detection, cancelled_detection, collection_metadata, daily_session_projections,
    detection_issue, empty_collection_result, invalid_configuration_detection, not_found_detection,
    path_is_missing, record_collector_diagnostic, request_failure, single_source_descriptor,
    unsupported_detection, validate_source, validation_failure_preserving_all_rejected,
    CollectorDiagnosticCounter, CollectorIdentity, LocalCollectionRun,
};
use super::{
    default_opencode_database, open_failure_code, source_cost_usd_to_micros, OpenCodeGeneration,
    OpenCodeMappingContext, OpenCodeMappingError, OpenCodeMessageUsage, OpenCodePageSize,
    OpenCodeSchemaInspection, OpenCodeSchemaReason, OpenCodeSessionHeader, OpenCodeStore,
    OpenCodeStoreError, OpenCodeTokenCounters, ScopedDailyFold, ScopedSessionFold,
};

const COLLECTOR_VERSION: &str = "local";
const PROFILE_VERSION: u16 = 2;
const SESSION_PAGE_SIZE: usize = 100;
const MESSAGE_PAGE_SIZE: usize = 500;
const LIVE_WRITE_STABILITY_AGE_MS: i64 = 5 * 60 * 1_000;
const IDENTITY: CollectorIdentity = CollectorIdentity {
    key: "opencode",
    display_name: "OpenCode",
    runtime_version: COLLECTOR_VERSION,
    adapter_version: 1,
    source: SourceKey::OpenCode,
    profile_version: PROFILE_VERSION,
};

#[derive(Clone)]
pub(crate) struct OpenCodeCollector {
    source_database_path: Option<PathBuf>,
    ledger: Arc<dyn OpenCodeUsageLedger>,
    diagnostics: Option<Arc<dyn DiagnosticRecorder>>,
    calculator: BurnlyCostCalculator,
    session_page_size: OpenCodePageSize,
    message_page_size: OpenCodePageSize,
}

impl OpenCodeCollector {
    pub(crate) fn from_default_location(ledger: Arc<dyn OpenCodeUsageLedger>) -> Self {
        Self::new(default_opencode_database(), ledger)
    }

    pub(crate) fn from_database_path(
        path: impl Into<PathBuf>,
        ledger: Arc<dyn OpenCodeUsageLedger>,
    ) -> Self {
        Self::new(Some(path.into()), ledger)
    }

    fn new(source_database_path: Option<PathBuf>, ledger: Arc<dyn OpenCodeUsageLedger>) -> Self {
        Self {
            source_database_path,
            ledger,
            diagnostics: None,
            calculator: BurnlyCostCalculator::new(),
            session_page_size: OpenCodePageSize::new(SESSION_PAGE_SIZE)
                .expect("OpenCode session page size is bounded"),
            message_page_size: OpenCodePageSize::new(MESSAGE_PAGE_SIZE)
                .expect("OpenCode message page size is bounded"),
        }
    }

    pub(crate) fn with_diagnostic_recorder(
        mut self,
        diagnostics: Arc<dyn DiagnosticRecorder>,
    ) -> Self {
        self.diagnostics = Some(diagnostics);
        self
    }

    #[cfg(test)]
    fn with_page_sizes(mut self, sessions: usize, messages: usize) -> Self {
        self.session_page_size = OpenCodePageSize::new(sessions).expect("session page size");
        self.message_page_size = OpenCodePageSize::new(messages).expect("message page size");
        self
    }
}

impl Collector for OpenCodeCollector {
    fn describe(&self) -> Result<CollectorDescriptor, CollectorFailure> {
        single_source_descriptor(
            IDENTITY,
            daily_session_projections(),
            CollectorIntegrity::UnverifiedDevelopment,
        )
    }

    fn detect(
        &self,
        request: DetectionRequest,
        cancellation: &dyn CancellationSignal,
    ) -> Result<DetectionResult, CollectorFailure> {
        if cancellation.is_cancelled() {
            return Ok(cancelled_detection(&request));
        }
        if request.source != SourceKey::OpenCode {
            return Ok(unsupported_detection(
                &request,
                detection_issue("opencode.unsupported_source", "Source is not OpenCode."),
            ));
        }
        let Some(path) = self.source_database_path.as_deref() else {
            return Ok(missing_detection(&request));
        };
        if path_is_missing(path) {
            return Ok(missing_detection(&request));
        }

        let result = OpenCodeStore::open_read_only(path).and_then(|mut store| {
            let snapshot = store.begin_snapshot()?;
            snapshot.read_sessions_page(None, self.session_page_size)
        });
        match result {
            Ok(sessions) => Ok(available_detection(
                &request,
                SourceKey::OpenCode,
                daily_session_projections(),
                !sessions.is_empty(),
            )),
            Err(_) => Ok(invalid_configuration_detection(
                &request,
                SourceKey::OpenCode,
                daily_session_projections(),
                detection_issue(
                    "opencode.database_incompatible",
                    "OpenCode usage database is not readable by Burnly.",
                ),
            )),
        }
    }

    fn collect(
        &self,
        request: CollectionRequest,
        cancellation: &dyn CancellationSignal,
    ) -> Result<CollectionResult, CollectorFailure> {
        let run = LocalCollectionRun::start();
        validate_source(&request, SourceKey::OpenCode)?;
        ensure_not_cancelled(&request, cancellation)?;
        let Some(path) = self.source_database_path.as_deref() else {
            return empty_collection_result(IDENTITY, &request, &run);
        };
        if path_is_missing(path) {
            return empty_collection_result(IDENTITY, &request, &run);
        }

        let mut store = match OpenCodeStore::open_read_only(path) {
            Ok(store) => store,
            Err(error) => {
                let code = match &error {
                    OpenCodeStoreError::Open(_) => open_failure_code(path),
                    _ => error.source_failure_code(),
                };
                self.record_failure(&request, code, CollectionStats::default());
                return Err(request_failure(&request, code));
            }
        };
        let inspection = store.capabilities();
        let mut stats = CollectionStats::default();
        let has_incomplete_residue = inspection.ignored_reason().is_some();
        if let Some(reason) = inspection.ignored_reason() {
            self.record_ignored_generation(&request, inspection, reason);
        }
        if has_incomplete_residue {
            self.prove_redundancy(
                &mut store,
                path,
                &request,
                &mut stats,
                ProofMoment::BeforeRead,
            )?;
        }

        let observed_at = Utc::now();
        let timezone = match projection_timezone(&request) {
            Ok(timezone) => timezone,
            Err(failure) => {
                if failure.code == CollectorFailureCode::IncompatibleEnvelope {
                    self.record_failure(&request, failure.code, stats);
                }
                return Err(failure);
            }
        };
        let context = OpenCodeMappingContext::new(
            COLLECTOR_VERSION.to_owned(),
            request.collection_id().clone(),
            observed_at,
        )
        .map_err(|_| request_failure(&request, CollectorFailureCode::Internal))?;
        let mut projection = match request.projection() {
            CollectionProjection::Daily => {
                ProjectionFold::Daily(ScopedDailyFold::from_timezone(timezone, request.scope()))
            }
            CollectionProjection::Session => ProjectionFold::Session(ScopedSessionFold::new()),
        };
        self.reconcile_all_sessions(
            &mut store,
            observed_at.timestamp_millis(),
            cancellation,
            &request,
            &mut projection,
            &mut stats,
        )
        .inspect_err(|failure| self.record_failure(&request, failure.code, stats))?;
        ensure_not_cancelled(&request, cancellation)?;
        // Re-prove redundancy after every collection read: a unique row added
        // to the incomplete generation mid-collection must not be silently
        // ignored, so the proof must hold at the end of collection, not only
        // at the start. A matching high-water mark skips the second id scan.
        if has_incomplete_residue {
            self.prove_redundancy(
                &mut store,
                path,
                &request,
                &mut stats,
                ProofMoment::AfterRead,
            )?;
        }

        if stats.counter_regressions > 0 {
            self.record_counter_regression(&request, stats);
        }
        if stats.non_usage_error_rows > 0 {
            self.record_non_usage_errors(&request, stats);
        }
        let metadata = collection_metadata(IDENTITY, &request, run.started_at(), observed_at)?;
        let rejections = partial_rejections(stats);
        let finished = projection.finish(&context, &self.calculator).map_err(|_| {
            self.record_failure(&request, CollectorFailureCode::IncompatibleEnvelope, stats);
            request_failure(&request, CollectorFailureCode::IncompatibleEnvelope)
        })?;
        let result = match finished {
            FinishedProjection::Daily(candidates) => CollectionResult::daily(
                metadata,
                candidates,
                rejections,
                Vec::new(),
                run.process_summary(),
            ),
            FinishedProjection::Session(candidates) => CollectionResult::session(
                metadata,
                candidates,
                rejections,
                Vec::new(),
                run.process_summary(),
            ),
        };
        let result = result.map_err(|error| {
            let failure = validation_failure_preserving_all_rejected(&request, error);
            self.record_failure(&request, failure.code, stats);
            failure
        })?;
        self.record_collection_stats(&request, stats);
        Ok(result)
    }
}

impl OpenCodeCollector {
    /// Re-proves that every residual row of the ignored generation is
    /// redundant with the selected generation. A stored proof skips the id
    /// scan. A failure is recorded as `collector.incompatible_envelope`
    /// because ignoring a non-redundant generation would silently understate
    /// usage. Full scope does not reuse the proof at the start of a collect.
    fn prove_redundancy(
        &self,
        store: &mut OpenCodeStore,
        path: &Path,
        request: &CollectionRequest,
        stats: &mut CollectionStats,
        moment: ProofMoment,
    ) -> Result<(), CollectorFailure> {
        let fingerprint = source_fingerprint(store, path);
        let stored = self.ledger.read_redundancy_proof().map_err(|_| {
            self.record_failure(request, CollectorFailureCode::Internal, *stats);
            internal(request)
        })?;
        let full = matches!(request.scope(), CollectionScope::Full);
        let proved = match moment {
            ProofMoment::BeforeRead if !full => stored
                .as_ref()
                .zip(fingerprint.as_ref())
                .is_some_and(|(proof, fingerprint)| proof_matches(proof, fingerprint)),
            ProofMoment::AfterRead => stored
                .as_ref()
                .zip(fingerprint.as_ref())
                .is_some_and(|(proof, fingerprint)| proof_matches_high_water(proof, fingerprint)),
            ProofMoment::BeforeRead => false,
        };
        if proved {
            return Ok(());
        }
        self.scan_redundancy(store, request, stats, fingerprint)
    }

    fn scan_redundancy(
        &self,
        store: &mut OpenCodeStore,
        request: &CollectionRequest,
        stats: &mut CollectionStats,
        fingerprint: Option<SourceFingerprint>,
    ) -> Result<(), CollectorFailure> {
        stats.redundancy_scans = stats.redundancy_scans.saturating_add(1);
        // `redundancy_exceeded` returns true when residue is NOT redundant.
        // A query failure fails closed: ignoring an unproved generation would
        // silently drop usage.
        let redundant = store
            .begin_snapshot()
            .ok()
            .and_then(|snapshot| snapshot.redundancy_exceeded().ok())
            .map(|exceeded| !exceeded)
            .unwrap_or(false);
        if let Some(fingerprint) = fingerprint.as_ref() {
            let proof = fingerprint.proof(redundant);
            if self.ledger.write_redundancy_proof(&proof).is_err() && redundant {
                self.record_failure(request, CollectorFailureCode::Internal, *stats);
                return Err(internal(request));
            }
        }
        if redundant {
            Ok(())
        } else {
            self.record_failure(request, CollectorFailureCode::IncompatibleEnvelope, *stats);
            Err(incompatible(request))
        }
    }

    fn reconcile_all_sessions(
        &self,
        store: &mut OpenCodeStore,
        observed_at_ms: i64,
        cancellation: &dyn CancellationSignal,
        request: &CollectionRequest,
        projection: &mut ProjectionFold,
        stats: &mut CollectionStats,
    ) -> Result<(), CollectorFailure> {
        // `collect` already rejected a missing or invalid timezone. A second
        // parse here keeps the activity window next to the session loop
        // without another argument.
        let timezone = projection_timezone(request)?;
        let window = activity_window(request.scope(), timezone);
        let mut after_session_id = None::<String>;
        let incremental = !matches!(request.scope(), CollectionScope::Full);

        loop {
            ensure_not_cancelled(request, cancellation)?;
            let batch = {
                let snapshot = store.begin_snapshot().map_err(|_| incompatible(request))?;
                let sessions = snapshot
                    .read_sessions_page(after_session_id.as_deref(), self.session_page_size)
                    .map_err(|_| incompatible(request))?;
                if sessions.is_empty() {
                    Vec::new()
                } else {
                    let mut work = Vec::with_capacity(sessions.len());
                    for session in sessions {
                        ensure_not_cancelled(request, cancellation)?;
                        let cumulative_cost_micros =
                            source_cost_usd_to_micros(Some(session.cost_usd))
                                .map_err(|_| incompatible(request))?;
                        let checkpoint = self
                            .ledger
                            .read_checkpoint(&session.id)
                            .map_err(|_| internal(request))?;
                        let stable_deferred_retry = checkpoint.as_ref().is_some_and(|checkpoint| {
                            checkpoint.reconciliation_state
                                == OpenCodeReconciliationState::DeferredLiveWrite
                                && checkpoint_observation_matches_header(
                                    checkpoint,
                                    &session,
                                    cumulative_cost_micros,
                                )
                                && observed_at_ms.saturating_sub(session.updated_at_ms)
                                    >= LIVE_WRITE_STABILITY_AGE_MS
                        });
                        let header_matches = incremental
                            && checkpoint.as_ref().is_some_and(|checkpoint| {
                                checkpoint_matches_header(
                                    checkpoint,
                                    &session,
                                    cumulative_cost_micros,
                                )
                            });
                        if header_matches {
                            let checkpoint = checkpoint.expect("matching checkpoint exists");
                            work.push(SessionWork::Cached {
                                session_id: session.id,
                                checkpoint,
                            });
                            continue;
                        }

                        let after_message_id = if incremental {
                            checkpoint
                                .as_ref()
                                .and_then(|checkpoint| checkpoint.last_source_message_id.as_deref())
                        } else {
                            None
                        };
                        // Full scope replaces the deferred set from its scan.
                        // Incremental scope point-reads only ids still open.
                        let stored_deferred = if incremental {
                            self.ledger
                                .read_deferred_message_ids(&session.id)
                                .map_err(|_| internal(request))?
                        } else {
                            Vec::new()
                        };
                        let mut read = self.read_messages_after(
                            &snapshot,
                            &session.id,
                            after_message_id,
                            cancellation,
                            request,
                        )?;
                        if incremental {
                            self.retry_deferred_messages(
                                &snapshot,
                                &session.id,
                                &stored_deferred,
                                &mut read,
                                request,
                            )?;
                        }
                        stats.message_pages = stats.message_pages.saturating_add(read.pages);
                        stats.messages_read =
                            stats.messages_read.saturating_add(read.messages_read);
                        stats.non_usage_error_rows = stats
                            .non_usage_error_rows
                            .saturating_add(read.non_usage_error_rows);
                        let recovery_disposition = match (read.deferred > 0, stable_deferred_retry)
                        {
                            (true, true) => OpenCodeRecoveryDisposition::StableIncomplete,
                            (true, false) => OpenCodeRecoveryDisposition::DeferredLiveWrite,
                            (false, _) => OpenCodeRecoveryDisposition::Ready,
                        };
                        let deferred_live_rows = u64::from(
                            recovery_disposition == OpenCodeRecoveryDisposition::DeferredLiveWrite,
                        )
                        .saturating_mul(read.deferred);
                        stats.deferred_live_rows =
                            stats.deferred_live_rows.saturating_add(deferred_live_rows);
                        // A suffix read must not rebuild the session. Rebuild
                        // deletes ledger rows this incremental pass did not
                        // read. Keep the stored facts and let a full collect
                        // repair the older correction.
                        if incremental
                            && checkpoint.as_ref().is_some_and(|checkpoint| {
                                header_regresses(checkpoint, &session, cumulative_cost_micros)
                            })
                        {
                            let checkpoint = checkpoint.expect("regressed checkpoint exists");
                            work.push(SessionWork::Cached {
                                session_id: session.id,
                                checkpoint,
                            });
                            continue;
                        }
                        work.push(SessionWork::Reconcile {
                            snapshot: session_snapshot(
                                session,
                                read.messages,
                                recovery_disposition,
                                observed_at_ms,
                            )
                            .map_err(|()| incompatible(request))?,
                            next_cursor: read.next_cursor,
                            deferred_ids: read.deferred_ids,
                        });
                    }
                    work
                }
            };

            if batch.is_empty() {
                break;
            }
            stats.session_pages = stats.session_pages.saturating_add(1);
            stats.sessions_processed = stats
                .sessions_processed
                .saturating_add(u64::try_from(batch.len()).unwrap_or(u64::MAX));
            for work in batch {
                ensure_not_cancelled(request, cancellation)?;
                match work {
                    SessionWork::Cached {
                        session_id,
                        checkpoint,
                    } => {
                        let bounds = self
                            .ledger
                            .session_activity_bounds(&session_id)
                            .map_err(|_| internal(request))?;
                        match cached_read(bounds, window) {
                            CachedRead::Skip => {
                                stats.sessions_skipped = stats.sessions_skipped.saturating_add(1);
                            }
                            CachedRead::Fold(filter) => {
                                self.fold_cached(projection, &checkpoint, filter, request)?;
                            }
                        }
                        after_session_id = Some(session_id);
                    }
                    SessionWork::Reconcile {
                        snapshot,
                        next_cursor,
                        deferred_ids,
                    } => {
                        let result = self
                            .ledger
                            .reconcile_session(&snapshot)
                            .map_err(|_| internal(request))?;
                        self.ledger
                            .store_message_progress(
                                &snapshot.session_id,
                                next_cursor.as_deref(),
                                &deferred_ids,
                            )
                            .map_err(|_| internal(request))?;
                        stats.include(&result);
                        projection
                            .add_reconciled(&result)
                            .map_err(|_| incompatible(request))?;
                        after_session_id = Some(snapshot.session_id);
                    }
                }
            }
        }
        Ok(())
    }

    fn fold_cached(
        &self,
        projection: &mut ProjectionFold,
        checkpoint: &OpenCodeSessionCheckpoint,
        window: Option<(i64, i64)>,
        request: &CollectionRequest,
    ) -> Result<(), CollectorFailure> {
        projection
            .begin_cached(checkpoint)
            .map_err(|_| incompatible(request))?;
        // Session rows replace lifetime totals. A windowed ledger read would
        // store the catch-up slice. Daily rows stay inside the request window.
        let activity_window = match projection {
            ProjectionFold::Session(_) => None,
            ProjectionFold::Daily(_) => window,
        };
        self.ledger
            .fold_scoped_records(&checkpoint.session_id, activity_window, &mut |record| {
                projection
                    .add_cached_record(checkpoint, &record)
                    .map_err(|_| OpenCodeUsageLedgerError::IncompatibleSnapshot)
            })
            .map_err(|error| match error {
                OpenCodeUsageLedgerError::Storage => internal(request),
                OpenCodeUsageLedgerError::IncompatibleSnapshot => incompatible(request),
            })?;
        projection.end_cached().map_err(|_| incompatible(request))?;
        Ok(())
    }

    fn read_messages_after(
        &self,
        snapshot: &super::OpenCodeReadSnapshot<'_>,
        session_id: &str,
        after: Option<&str>,
        cancellation: &dyn CancellationSignal,
        request: &CollectionRequest,
    ) -> Result<MessageRead, CollectorFailure> {
        let mut page_cursor = after.map(str::to_owned);
        let mut next_cursor = after.map(str::to_owned);
        let mut messages = Vec::new();
        let mut deferred_ids = Vec::new();
        let mut seen_ids = HashSet::new();
        let mut deferred = 0_u64;
        let mut messages_read = 0_u64;
        let mut non_usage_error_rows = 0_u64;
        let mut pages = 0_u64;
        loop {
            ensure_not_cancelled(request, cancellation)?;
            let page = snapshot
                .read_messages_page(session_id, page_cursor.as_deref(), self.message_page_size)
                .map_err(|_| incompatible(request))?;
            if !page.has_rows() {
                break;
            }
            pages = pages.saturating_add(1);
            non_usage_error_rows = non_usage_error_rows.saturating_add(page.non_usage_error_rows);
            // High water includes deferred and non-usage rows. Stopping at
            // the last completed id makes the next pass re-read the tail.
            next_cursor.clone_from(&page.last_row_id);
            page_cursor = page.last_row_id;
            for message in page.messages {
                let incomplete_live_row = message.generation == OpenCodeGeneration::V2
                    && message.completed_at_ms.is_none();
                seen_ids.insert(message.id.clone());
                if incomplete_live_row {
                    deferred_ids.push(message.id);
                    deferred = deferred.saturating_add(1);
                } else {
                    messages_read = messages_read.saturating_add(1);
                    messages.push(message);
                }
            }
        }
        Ok(MessageRead {
            messages,
            deferred_ids,
            seen_ids,
            deferred,
            messages_read,
            non_usage_error_rows,
            pages,
            next_cursor,
        })
    }

    fn retry_deferred_messages(
        &self,
        snapshot: &super::OpenCodeReadSnapshot<'_>,
        session_id: &str,
        stored_ids: &[String],
        read: &mut MessageRead,
        request: &CollectionRequest,
    ) -> Result<(), CollectorFailure> {
        let mut pending = Vec::new();
        for message_id in stored_ids {
            // A page that still overlaps the old cursor already classified
            // this id. Point-reading it again repeats the JSON row.
            if read.seen_ids.insert(message_id.clone()) {
                pending.push(message_id.as_str());
            }
        }
        if pending.is_empty() {
            return Ok(());
        }
        let found = snapshot
            .read_messages_by_id(session_id, &pending)
            .map_err(|_| incompatible(request))?;
        for message in found.into_iter().flatten() {
            let still_incomplete =
                message.generation == OpenCodeGeneration::V2 && message.completed_at_ms.is_none();
            if still_incomplete {
                read.deferred_ids.push(message.id);
                read.deferred = read.deferred.saturating_add(1);
            } else {
                read.messages_read = read.messages_read.saturating_add(1);
                read.messages.push(message);
            }
        }
        Ok(())
    }

    fn record_failure(
        &self,
        request: &CollectionRequest,
        code: CollectorFailureCode,
        stats: CollectionStats,
    ) {
        record_collector_diagnostic(
            self.diagnostics.as_deref(),
            request,
            DiagnosticSeverity::Warning,
            "opencode.collection_failed",
            "OpenCode collection failed.",
            Some(code),
            &stats.counters(),
        );
    }

    fn record_ignored_generation(
        &self,
        request: &CollectionRequest,
        inspection: OpenCodeSchemaInspection,
        reason: OpenCodeSchemaReason,
    ) {
        let Some(recorder) = self.diagnostics.as_deref() else {
            return;
        };
        let Ok(code) = crate::application::diagnostics::DiagnosticCode::new(
            "opencode.incomplete_generation_ignored",
        ) else {
            return;
        };
        let Ok(summary) = crate::application::diagnostics::DiagnosticSummary::new(
            "An incomplete OpenCode schema generation was ignored.",
        ) else {
            return;
        };
        let Ok(context) = crate::application::diagnostics::DiagnosticContext::new(
            ignored_generation_context(request, inspection, reason),
        ) else {
            return;
        };
        let Ok(event) = crate::application::diagnostics::DiagnosticEvent::new(
            crate::application::diagnostics::DiagnosticArea::Collector,
            DiagnosticSeverity::Info,
            code,
            summary,
            Some(context),
            Utc::now().timestamp_millis(),
        ) else {
            return;
        };
        recorder.record(event);
    }

    fn record_counter_regression(&self, request: &CollectionRequest, stats: CollectionStats) {
        record_collector_diagnostic(
            self.diagnostics.as_deref(),
            request,
            DiagnosticSeverity::Warning,
            "opencode.session_counter_regressed",
            "OpenCode cumulative usage counters regressed.",
            None,
            &stats.counters(),
        );
    }

    fn record_non_usage_errors(&self, request: &CollectionRequest, stats: CollectionStats) {
        record_collector_diagnostic(
            self.diagnostics.as_deref(),
            request,
            DiagnosticSeverity::Info,
            "opencode.non_usage_error_rows_skipped",
            "OpenCode assistant error rows without usage were reconciled from session counters.",
            None,
            &stats.counters(),
        );
    }

    fn record_collection_stats(&self, request: &CollectionRequest, stats: CollectionStats) {
        record_collector_diagnostic(
            self.diagnostics.as_deref(),
            request,
            DiagnosticSeverity::Info,
            "opencode.collection_stats",
            "OpenCode collection finished.",
            None,
            &stats.scope_counters(),
        );
    }
}

enum SessionWork {
    Cached {
        session_id: String,
        checkpoint: OpenCodeSessionCheckpoint,
    },
    Reconcile {
        snapshot: OpenCodeSessionLedgerSnapshot,
        next_cursor: Option<String>,
        deferred_ids: Vec<String>,
    },
}

enum ProofMoment {
    BeforeRead,
    AfterRead,
}

#[derive(Clone, Copy)]
enum ActivityWindow {
    Range(i64, i64),
    Unbounded,
}

enum CachedRead {
    Skip,
    Fold(Option<(i64, i64)>),
}

struct MessageRead {
    messages: Vec<OpenCodeMessageUsage>,
    deferred_ids: Vec<String>,
    seen_ids: HashSet<String>,
    deferred: u64,
    messages_read: u64,
    non_usage_error_rows: u64,
    pages: u64,
    next_cursor: Option<String>,
}

struct SourceFingerprint {
    file_length: u64,
    file_mtime_ms: i64,
    page_count: u64,
    ignored_max_rowid: Option<i64>,
    ignored_row_count: u64,
    generations: &'static str,
}

impl SourceFingerprint {
    fn proof(&self, proved_redundant: bool) -> OpenCodeRedundancyProof {
        OpenCodeRedundancyProof {
            file_length: self.file_length,
            file_mtime_ms: self.file_mtime_ms,
            page_count: self.page_count,
            ignored_max_rowid: self.ignored_max_rowid,
            ignored_row_count: self.ignored_row_count,
            generations: self.generations.to_owned(),
            proved_redundant,
        }
    }
}

enum ProjectionFold {
    Daily(ScopedDailyFold),
    Session(ScopedSessionFold),
}

impl ProjectionFold {
    fn begin_cached(
        &mut self,
        checkpoint: &OpenCodeSessionCheckpoint,
    ) -> Result<(), OpenCodeMappingError> {
        match self {
            Self::Daily(_) => Ok(()),
            Self::Session(fold) => fold.begin_session(checkpoint),
        }
    }

    fn add_cached_record(
        &mut self,
        checkpoint: &OpenCodeSessionCheckpoint,
        record: &OpenCodeLedgerRecord,
    ) -> Result<(), OpenCodeMappingError> {
        match self {
            Self::Daily(fold) => fold.add_cached_record(checkpoint, record),
            Self::Session(fold) => fold.add_record(record),
        }
    }

    fn end_cached(&mut self) -> Result<(), OpenCodeMappingError> {
        match self {
            Self::Daily(_) => Ok(()),
            Self::Session(fold) => fold.finish_open(),
        }
    }

    fn add_reconciled(
        &mut self,
        reconciled: &OpenCodeLedgerReconcileResult,
    ) -> Result<(), OpenCodeMappingError> {
        match self {
            Self::Daily(fold) => fold.add_reconciled(reconciled),
            Self::Session(fold) => fold.add_reconciled(reconciled),
        }
    }

    fn finish(
        self,
        context: &OpenCodeMappingContext,
        calculator: &BurnlyCostCalculator,
    ) -> Result<FinishedProjection, OpenCodeMappingError> {
        match self {
            Self::Daily(fold) => Ok(FinishedProjection::Daily(fold.finish(context, calculator)?)),
            Self::Session(fold) => Ok(FinishedProjection::Session(
                fold.finish(context, calculator)?,
            )),
        }
    }
}

enum FinishedProjection {
    Daily(Vec<DailyUsageCandidate>),
    Session(Vec<SessionUsageCandidate>),
}

#[derive(Debug, Clone, Copy, Default)]
struct CollectionStats {
    session_pages: u64,
    message_pages: u64,
    sessions_processed: u64,
    sessions_skipped: u64,
    messages_read: u64,
    deferred_live_rows: u64,
    non_usage_error_rows: u64,
    exact_records_accepted: u64,
    recovery_segments_created: u64,
    late_exact_reclassified: u64,
    late_exact_ignored: u64,
    counter_regressions: u64,
    redundancy_scans: u64,
}

impl CollectionStats {
    fn include(&mut self, result: &OpenCodeLedgerReconcileResult) {
        self.exact_records_accepted = self
            .exact_records_accepted
            .saturating_add(u64::from(result.exact_records_accepted));
        self.recovery_segments_created = self
            .recovery_segments_created
            .saturating_add(u64::from(result.recovery_segments_created));
        self.late_exact_reclassified = self
            .late_exact_reclassified
            .saturating_add(u64::from(result.late_exact_reclassified));
        self.late_exact_ignored = self
            .late_exact_ignored
            .saturating_add(u64::from(result.late_exact_ignored));
        self.counter_regressions = self
            .counter_regressions
            .saturating_add(u64::from(result.counter_regressions));
    }

    fn counters(self) -> [CollectorDiagnosticCounter; 11] {
        [
            CollectorDiagnosticCounter::new("sessionPages", self.session_pages),
            CollectorDiagnosticCounter::new("messagePages", self.message_pages),
            CollectorDiagnosticCounter::new("sessionsProcessed", self.sessions_processed),
            CollectorDiagnosticCounter::new("messagesRead", self.messages_read),
            CollectorDiagnosticCounter::new("deferredLiveRows", self.deferred_live_rows),
            CollectorDiagnosticCounter::new("nonUsageErrorRows", self.non_usage_error_rows),
            CollectorDiagnosticCounter::new("exactRecordsAccepted", self.exact_records_accepted),
            CollectorDiagnosticCounter::new(
                "recoverySegmentsCreated",
                self.recovery_segments_created,
            ),
            CollectorDiagnosticCounter::new("lateExactReclassified", self.late_exact_reclassified),
            CollectorDiagnosticCounter::new("lateExactIgnored", self.late_exact_ignored),
            CollectorDiagnosticCounter::new("counterRegressions", self.counter_regressions),
        ]
    }

    fn scope_counters(self) -> [CollectorDiagnosticCounter; 5] {
        [
            CollectorDiagnosticCounter::new("sessionsProcessed", self.sessions_processed),
            CollectorDiagnosticCounter::new("sessionsSkipped", self.sessions_skipped),
            CollectorDiagnosticCounter::new("messagesRead", self.messages_read),
            CollectorDiagnosticCounter::new("messagePages", self.message_pages),
            CollectorDiagnosticCounter::new("redundancyScans", self.redundancy_scans),
        ]
    }
}

fn projection_timezone(request: &CollectionRequest) -> Result<Tz, CollectorFailure> {
    match request.projection() {
        CollectionProjection::Session => Ok(chrono_tz::UTC),
        CollectionProjection::Daily => {
            let name = request.aggregation_timezone().ok_or_else(|| {
                request_failure(request, CollectorFailureCode::ScopeNotRepresentable)
            })?;
            name.parse::<Tz>().map_err(|_| incompatible(request))
        }
    }
}

fn activity_window(scope: &CollectionScope, timezone: Tz) -> ActivityWindow {
    let CollectionScope::Incremental(scope) = scope else {
        return ActivityWindow::Unbounded;
    };
    let start = local_midnight_ms(timezone, scope.start_date());
    let end = scope
        .end_date()
        .succ_opt()
        .and_then(|next| local_midnight_ms(timezone, next))
        .map(|end_exclusive| end_exclusive.saturating_sub(1));
    match (start, end) {
        (Some(start), Some(end)) if start <= end => ActivityWindow::Range(start, end),
        _ => ActivityWindow::Unbounded,
    }
}

fn local_midnight_ms(timezone: Tz, date: NaiveDate) -> Option<i64> {
    timezone
        .from_local_datetime(&date.and_time(NaiveTime::MIN))
        .earliest()
        .map(|value| value.timestamp_millis())
}

fn cached_read(bounds: Option<OpenCodeActivityBounds>, window: ActivityWindow) -> CachedRead {
    match window {
        ActivityWindow::Unbounded => CachedRead::Fold(None),
        ActivityWindow::Range(start, end) => match bounds {
            Some(bounds) if bounds.latest_ms < start || bounds.earliest_ms > end => {
                CachedRead::Skip
            }
            None => CachedRead::Skip,
            Some(_) => CachedRead::Fold(Some((start, end))),
        },
    }
}

fn source_fingerprint(store: &mut OpenCodeStore, path: &Path) -> Option<SourceFingerprint> {
    let inspection = store.capabilities();
    let generations = inspection.redundancy_generations()?;
    let table = inspection.ignored_detail_table()?;
    let metadata = std::fs::metadata(path).ok()?;
    let file_mtime_ms = i64::try_from(
        metadata
            .modified()
            .ok()?
            .duration_since(UNIX_EPOCH)
            .ok()?
            .as_millis(),
    )
    .ok()?;
    let snapshot = store.begin_snapshot().ok()?;
    let page_count = snapshot.page_count().ok()?;
    let (ignored_max_rowid, ignored_row_count) = snapshot.detail_high_water(table).ok()?;
    Some(SourceFingerprint {
        file_length: metadata.len(),
        file_mtime_ms,
        page_count,
        ignored_max_rowid,
        ignored_row_count,
        generations,
    })
}

fn proof_matches(proof: &OpenCodeRedundancyProof, fingerprint: &SourceFingerprint) -> bool {
    proof.proved_redundant
        && proof.file_length == fingerprint.file_length
        && proof.file_mtime_ms == fingerprint.file_mtime_ms
        && proof.page_count == fingerprint.page_count
        && proof.ignored_max_rowid == fingerprint.ignored_max_rowid
        && proof.ignored_row_count == fingerprint.ignored_row_count
        && proof.generations == fingerprint.generations
}

fn proof_matches_high_water(
    proof: &OpenCodeRedundancyProof,
    fingerprint: &SourceFingerprint,
) -> bool {
    proof.proved_redundant
        && proof.ignored_max_rowid == fingerprint.ignored_max_rowid
        && proof.ignored_row_count == fingerprint.ignored_row_count
        && proof.generations == fingerprint.generations
}

fn header_regresses(
    checkpoint: &OpenCodeSessionCheckpoint,
    header: &OpenCodeSessionHeader,
    cumulative_cost_micros: Option<u64>,
) -> bool {
    // Accepted ledger cost can sit one micro above the source per priced row.
    // Only a drop below the cost last observed on the header is a shrink.
    token_vector(header.tokens)
        .checked_sub(checkpoint.accepted_tokens)
        .is_none()
        || matches!(
            (cumulative_cost_micros, checkpoint.observed_source_cost_micros),
            (Some(current), Some(observed)) if current < observed
        )
}

fn session_snapshot(
    header: OpenCodeSessionHeader,
    messages: Vec<OpenCodeMessageUsage>,
    recovery_disposition: OpenCodeRecoveryDisposition,
    observed_at_ms: i64,
) -> Result<OpenCodeSessionLedgerSnapshot, ()> {
    let exact_usage = messages
        .into_iter()
        .map(exact_usage)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(OpenCodeSessionLedgerSnapshot {
        session_id: header.id,
        source_updated_at_ms: header.updated_at_ms,
        recovery_activity_at_ms: Some(header.idle_at_ms.unwrap_or(header.updated_at_ms)),
        cumulative_tokens: token_vector(header.tokens),
        cumulative_cost_micros: source_cost_usd_to_micros(Some(header.cost_usd)).map_err(|_| ())?,
        exact_usage,
        recovery_disposition,
        observed_at_ms,
    })
}

fn exact_usage(message: OpenCodeMessageUsage) -> Result<OpenCodeExactUsage, ()> {
    if message
        .completed_at_ms
        .is_some_and(|completed| completed < message.created_at_ms)
    {
        return Err(());
    }
    Ok(OpenCodeExactUsage {
        message_id: message.id,
        activity_at_ms: message.completed_at_ms.unwrap_or(message.created_at_ms),
        provider_id: message.provider_id,
        raw_model_id: message.model_id,
        tokens: token_vector(message.tokens),
        cost_micros: source_cost_usd_to_micros(message.cost_usd).map_err(|_| ())?,
        origin: match message.generation {
            OpenCodeGeneration::V1 => OpenCodeExactOrigin::V1Message,
            OpenCodeGeneration::V2 => OpenCodeExactOrigin::V2Message,
        },
    })
}

const fn token_vector(tokens: OpenCodeTokenCounters) -> OpenCodeTokenVector {
    OpenCodeTokenVector {
        input: tokens.input,
        output: tokens.output,
        reasoning: tokens.reasoning,
        cache_read: tokens.cache_read,
        cache_write: tokens.cache_write,
    }
}

fn checkpoint_matches_header(
    checkpoint: &crate::application::ports::opencode_usage_ledger::OpenCodeSessionCheckpoint,
    header: &OpenCodeSessionHeader,
    cumulative_cost_micros: Option<u64>,
) -> bool {
    checkpoint.reconciliation_state != OpenCodeReconciliationState::DeferredLiveWrite
        && checkpoint_observation_matches_header(checkpoint, header, cumulative_cost_micros)
}

fn checkpoint_observation_matches_header(
    checkpoint: &crate::application::ports::opencode_usage_ledger::OpenCodeSessionCheckpoint,
    header: &OpenCodeSessionHeader,
    cumulative_cost_micros: Option<u64>,
) -> bool {
    checkpoint.source_updated_at_ms == header.updated_at_ms
        && checkpoint.observed_source_tokens == token_vector(header.tokens)
        && checkpoint.observed_source_cost_micros == cumulative_cost_micros
}

fn partial_rejections(stats: CollectionStats) -> Vec<RejectedRecord> {
    let mut rejections = Vec::with_capacity(2);
    if stats.deferred_live_rows > 0 {
        rejections.push(RejectedRecord {
            code: "opencode.live_write_deferred".to_owned(),
            record_index: None,
        });
    }
    if stats.counter_regressions > 0 {
        rejections.push(RejectedRecord {
            code: "opencode.session_counter_regressed".to_owned(),
            record_index: None,
        });
    }
    rejections
}

fn missing_detection(request: &DetectionRequest) -> DetectionResult {
    not_found_detection(
        request,
        SourceKey::OpenCode,
        daily_session_projections(),
        detection_issue(
            "opencode.database_missing",
            "OpenCode usage database was not found.",
        ),
    )
}

fn ensure_not_cancelled(
    request: &CollectionRequest,
    cancellation: &dyn CancellationSignal,
) -> Result<(), CollectorFailure> {
    if cancellation.is_cancelled() {
        Err(request_failure(request, CollectorFailureCode::Cancelled))
    } else {
        Ok(())
    }
}

fn incompatible(request: &CollectionRequest) -> CollectorFailure {
    request_failure(request, CollectorFailureCode::IncompatibleEnvelope)
}

fn internal(request: &CollectionRequest) -> CollectorFailure {
    request_failure(request, CollectorFailureCode::Internal)
}

fn ignored_generation_context(
    request: &CollectionRequest,
    inspection: OpenCodeSchemaInspection,
    reason: OpenCodeSchemaReason,
) -> String {
    let selected = if inspection.has_v1() && inspection.has_v2() {
        "combined"
    } else if inspection.has_v1() {
        "v1"
    } else {
        "v2"
    };
    serde_json::json!({
        "source": SourceKey::OpenCode.as_str(),
        "projection": match request.projection() {
            CollectionProjection::Daily => "daily",
            CollectionProjection::Session => "session",
        },
        "selectedGenerations": selected,
        "ignoredGeneration": match inspection.ignored_generation() {
            Some(OpenCodeGeneration::V1) => "v1",
            Some(OpenCodeGeneration::V2) => "v2",
            None => "none",
        },
        "reason": schema_reason_name(reason),
    })
    .to_string()
}

const fn schema_reason_name(reason: OpenCodeSchemaReason) -> &'static str {
    match reason {
        OpenCodeSchemaReason::MissingSessionTable => "missing_session_table",
        OpenCodeSchemaReason::MissingDetailTable => "missing_detail_table",
        OpenCodeSchemaReason::MissingRequiredColumn => "missing_required_column",
        OpenCodeSchemaReason::SchemaQueryFailed => "schema_query_failed",
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    use chrono::Utc;
    use rusqlite::{params, Connection};
    use serde_json::json;
    use tempfile::TempDir;

    use super::*;
    use crate::application::collection::{
        CollectionId, CollectionOutcome, CollectionScope, DetectionState,
    };
    use crate::infrastructure::collectors::support::{
        daily_request, detection_request, session_request, NeverCancelled, RecordingDiagnostics,
    };
    use crate::infrastructure::database::{Database, SqliteOpenCodeUsageLedgerStore};

    #[test]
    fn describes_native_profile_and_detects_missing_incompatible_and_available_sources() {
        let fixture = Fixture::new(false, true);
        let descriptor = fixture.collector().describe().expect("descriptor");
        assert_eq!(descriptor.collector.as_str(), "opencode");
        assert_eq!(descriptor.profiles.len(), 1);
        assert_eq!(descriptor.profiles[0].source, SourceKey::OpenCode);
        assert_eq!(descriptor.profiles[0].profile_version, 2);
        assert_eq!(
            descriptor.profiles[0].supported_projections,
            daily_session_projections()
        );

        let request = detection_request(SourceKey::OpenCode, Utc::now());
        let available = fixture
            .collector()
            .detect(request.clone(), &NeverCancelled)
            .expect("available detection");
        assert_eq!(available.state, DetectionState::AvailableNoData);

        let missing = OpenCodeCollector::from_database_path(
            fixture.directory.path().join("missing.db"),
            fixture.ledger(),
        )
        .detect(request.clone(), &NeverCancelled)
        .expect("missing detection");
        assert_eq!(missing.state, DetectionState::NotFound);
        assert_eq!(missing.issues[0].code, "opencode.database_missing");

        let incompatible_path = fixture.directory.path().join("incompatible.db");
        Connection::open(&incompatible_path)
            .expect("incompatible database")
            .execute("CREATE TABLE unrelated (id TEXT)", [])
            .expect("unrelated schema");
        let incompatible =
            OpenCodeCollector::from_database_path(incompatible_path, fixture.ledger())
                .detect(request, &NeverCancelled)
                .expect("incompatible detection");
        assert_eq!(incompatible.state, DetectionState::InvalidConfiguration);
        assert_eq!(
            incompatible.issues[0].code,
            "opencode.database_incompatible"
        );
        assert!(!incompatible.issues[0]
            .message
            .contains(fixture.directory.path().to_string_lossy().as_ref()));
    }

    #[test]
    #[ignore = "set BURNLY_OPENCODE_EVIDENCE_LEDGER to run against local OpenCode data"]
    fn runtime_evidence_collects_default_location_without_sensitive_output() {
        let ledger_path = std::env::var_os("BURNLY_OPENCODE_EVIDENCE_LEDGER")
            .map(PathBuf::from)
            .expect("BURNLY_OPENCODE_EVIDENCE_LEDGER must name a disposable database");
        let mut ledger_database = Database::open(&ledger_path).expect("open evidence ledger");
        ledger_database
            .migrate_to_latest()
            .expect("migrate evidence ledger");
        let diagnostics = Arc::new(RecordingDiagnostics::default());
        let collector = OpenCodeCollector::from_default_location(Arc::new(
            SqliteOpenCodeUsageLedgerStore::new(ledger_database),
        ))
        .with_diagnostic_recorder(diagnostics.clone());

        let detection = collector
            .detect(
                detection_request(SourceKey::OpenCode, Utc::now()),
                &NeverCancelled,
            )
            .expect("default-location detection");
        assert!(matches!(
            detection.state,
            DetectionState::Available | DetectionState::AvailableNoData
        ));

        let initial_daily = collector
            .collect(
                daily_request(
                    "runtime-evidence-daily-initial",
                    SourceKey::OpenCode,
                    CollectionScope::Full,
                    "Asia/Jakarta",
                    Utc::now(),
                ),
                &NeverCancelled,
            )
            .expect("full daily collection");
        let sessions = collector
            .collect(
                session_request("runtime-evidence-session", SourceKey::OpenCode, Utc::now()),
                &NeverCancelled,
            )
            .expect("full session collection");
        let daily = collector
            .collect(
                daily_request(
                    "runtime-evidence-daily-stable",
                    SourceKey::OpenCode,
                    CollectionScope::Full,
                    "Asia/Jakarta",
                    Utc::now(),
                ),
                &NeverCancelled,
            )
            .expect("stable full daily collection");

        assert!(matches!(
            initial_daily.outcome(),
            CollectionOutcome::Complete | CollectionOutcome::Partial
        ));
        eprintln!(
            "opencode_runtime_probe_outcomes initial={:?}/{} sessions={:?}/{} stable={:?}/{} diagnostics={:?}",
            initial_daily.outcome(),
            initial_daily.rejection_count(),
            sessions.outcome(),
            sessions.rejection_count(),
            daily.outcome(),
            daily.rejection_count(),
            diagnostics
                .events()
                .iter()
                .map(|event| event.code.as_str())
                .collect::<Vec<_>>()
        );
        assert_eq!(daily.outcome(), CollectionOutcome::Complete);
        assert_eq!(sessions.outcome(), CollectionOutcome::Complete);
        let daily_totals = aggregate_tokens(
            daily
                .daily_candidates()
                .iter()
                .map(|candidate| &candidate.tokens),
        );
        let session_totals = aggregate_tokens(
            sessions
                .session_candidates()
                .iter()
                .map(|candidate| &candidate.tokens),
        );
        assert_eq!(daily_totals, session_totals);
        assert!(daily.daily_candidates().iter().all(|candidate| {
            candidate
                .model_breakdowns
                .iter()
                .map(|model| model.tokens.total_tokens())
                .sum::<u64>()
                == candidate.tokens.total_tokens()
        }));
        assert!(sessions.session_candidates().iter().all(|candidate| {
            candidate
                .model_breakdowns
                .iter()
                .map(|model| model.tokens.total_tokens())
                .sum::<u64>()
                == candidate.tokens.total_tokens()
        }));

        let jakarta_today = Utc::now()
            .with_timezone(&chrono_tz::Asia::Jakarta)
            .date_naive();
        let today_totals = aggregate_tokens(
            daily
                .daily_candidates()
                .iter()
                .filter(|candidate| candidate.usage_date == jakarta_today)
                .map(|candidate| &candidate.tokens),
        );
        let model_rows = daily
            .daily_candidates()
            .iter()
            .map(|candidate| candidate.model_breakdowns.len())
            .sum::<usize>();

        println!(
            "opencode_runtime_evidence=v1 detection=available initial_rejections={} days={} sessions={} model_rows={} input={} output={} cache_write={} cache_read={} reasoning={} total={} today={} today_input={} today_output={} today_cache_write={} today_cache_read={} today_reasoning={} today_total={}",
            initial_daily.rejection_count(),
            daily.daily_candidates().len(),
            sessions.session_candidates().len(),
            model_rows,
            daily_totals.input,
            daily_totals.output,
            daily_totals.cache_write,
            daily_totals.cache_read,
            daily_totals.reasoning,
            daily_totals.total,
            jakarta_today,
            today_totals.input,
            today_totals.output,
            today_totals.cache_write,
            today_totals.cache_read,
            today_totals.reasoning,
            today_totals.total,
        );
    }

    #[test]
    fn exhausts_bounded_session_and_message_pages_and_maps_daily_and_sessions() {
        let fixture = Fixture::new(false, true);
        let connection = fixture.source();
        for (session_index, session_id) in ["session-a", "session-b"].iter().enumerate() {
            insert_v2_session(
                &connection,
                session_id,
                Counters::new(6, 6, 9, 12, 15),
                1.5,
                1_000 + i64::try_from(session_index).expect("index") * 100,
            );
            for message_index in 0..3 {
                insert_v2_message(
                    &connection,
                    &format!("{session_id}-message-{message_index}"),
                    session_id,
                    i64::from(message_index + 1),
                    1_100 + i64::from(message_index),
                    true,
                );
            }
        }
        drop(connection);
        let collector = fixture.collector().with_page_sizes(1, 1);

        let sessions = collector
            .collect(
                session_request("sessions", SourceKey::OpenCode, Utc::now()),
                &NeverCancelled,
            )
            .expect("session collection");
        assert_eq!(sessions.outcome(), CollectionOutcome::Complete);
        assert_eq!(sessions.session_candidates().len(), 2);
        assert!(sessions
            .session_candidates()
            .iter()
            .all(|candidate| candidate.tokens.total_tokens() == 48));

        let daily = collector
            .collect(
                daily_request(
                    "daily",
                    SourceKey::OpenCode,
                    CollectionScope::Full,
                    "UTC",
                    Utc::now(),
                ),
                &NeverCancelled,
            )
            .expect("daily collection");
        assert_eq!(daily.outcome(), CollectionOutcome::Complete);
        assert_eq!(daily.daily_candidates().len(), 1);
        assert_eq!(daily.daily_candidates()[0].tokens.total_tokens(), 96);
        assert_eq!(daily.metadata().collector().as_str(), "opencode");
        assert_eq!(daily.metadata().profile_version(), 2);
    }

    #[test]
    fn collects_v1_only_usage_with_native_identity() {
        let fixture = Fixture::new(true, false);
        let connection = fixture.source();
        insert_v1_session(&connection, "session-v1", Counters::message(5), 0.25, 1_000);
        insert_v1_message(&connection, "message-v1", "session-v1", 5, 1_100);
        drop(connection);

        let result = fixture
            .collector()
            .collect(
                session_request("v1-only", SourceKey::OpenCode, Utc::now()),
                &NeverCancelled,
            )
            .expect("V1 collection");
        let candidate = &result.session_candidates()[0];

        assert_eq!(result.outcome(), CollectionOutcome::Complete);
        assert_eq!(candidate.tokens.total_tokens(), 19);
        assert_eq!(
            candidate.model_breakdowns[0].raw_model_id,
            "provider-v1/model"
        );
    }

    #[test]
    fn incompatible_collection_records_redacted_failure() {
        let fixture = Fixture::new(false, true);
        let incompatible_path = fixture.directory.path().join("invalid-source.db");
        Connection::open(&incompatible_path)
            .expect("incompatible database")
            .execute("CREATE TABLE unrelated (id TEXT)", [])
            .expect("unrelated schema");
        let diagnostics = Arc::new(RecordingDiagnostics::default());
        let collector = OpenCodeCollector::from_database_path(incompatible_path, fixture.ledger())
            .with_diagnostic_recorder(diagnostics.clone());

        let failure = collector
            .collect(
                session_request("invalid", SourceKey::OpenCode, Utc::now()),
                &NeverCancelled,
            )
            .expect_err("incompatible source");
        assert_eq!(failure.code, CollectorFailureCode::IncompatibleEnvelope);
        let events = diagnostics.events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].code.as_str(), "opencode.collection_failed");
        let context = events[0].context.as_ref().expect("context").as_str();
        assert!(context.contains(r#""failureCode":"collector.incompatible_envelope""#));
        assert!(!context.contains("invalid-source.db"));
        assert!(!context.contains(fixture.directory.path().to_string_lossy().as_ref()));
    }

    #[test]
    fn missing_database_keeps_source_not_found_classification() {
        let fixture = Fixture::new(true, false);
        let missing_path = fixture.directory.path().join("missing.db");
        let diagnostics = Arc::new(RecordingDiagnostics::default());
        let collector = OpenCodeCollector::from_database_path(missing_path, fixture.ledger())
            .with_diagnostic_recorder(diagnostics.clone());

        let result = collector
            .collect(
                session_request("missing", SourceKey::OpenCode, Utc::now()),
                &NeverCancelled,
            )
            .expect("missing source is an empty collection");
        assert_eq!(result.outcome(), CollectionOutcome::Empty);
        assert!(result.session_candidates().is_empty());
        assert!(diagnostics.events().is_empty());
    }

    #[test]
    fn mixed_generation_collects_v1_and_ignores_residual_session_message() {
        let fixture = Fixture::new(true, true);
        let connection = fixture.source();
        insert_v1_session(
            &connection,
            "session-v1-active",
            Counters::new(11, 4, 6, 8, 10),
            0.5,
            1_000,
        );
        insert_v1_message(&connection, "message-v1", "session-v1-active", 5, 1_100);
        insert_v1_message(&connection, "message-v1b", "session-v1-active", 6, 1_150);
        // The residual V2 row reuses a stable message ID present in V1, so it
        // is redundant with the selected generation and can be ignored.
        insert_v2_message(
            &connection,
            "message-v1",
            "session-v1-active",
            9,
            1_200,
            true,
        );
        drop(connection);
        // Remove the V2 session table to reproduce the reported production
        // shape: complete V1 plus a residual `session_message`.
        fixture
            .source()
            .execute("DROP TABLE session_v2", [])
            .expect("drop residual V2 session table");
        let diagnostics = Arc::new(RecordingDiagnostics::default());
        let collector = fixture
            .collector()
            .with_diagnostic_recorder(diagnostics.clone());

        let sessions = collector
            .collect(
                session_request("mixed-session", SourceKey::OpenCode, Utc::now()),
                &NeverCancelled,
            )
            .expect("session collection");
        assert_eq!(sessions.outcome(), CollectionOutcome::Complete);
        assert_eq!(sessions.session_candidates().len(), 1);
        let candidate = &sessions.session_candidates()[0];
        assert_eq!(candidate.tokens.total_tokens(), 39);
        assert_eq!(
            candidate.model_breakdowns[0].raw_model_id,
            "provider-v1/model"
        );
        assert!(candidate
            .model_breakdowns
            .iter()
            .all(|model| model.raw_model_id != "provider-v2/model"));

        let daily = collector
            .collect(
                daily_request(
                    "mixed-daily",
                    SourceKey::OpenCode,
                    CollectionScope::Full,
                    "UTC",
                    Utc::now(),
                ),
                &NeverCancelled,
            )
            .expect("daily collection");
        assert_eq!(daily.outcome(), CollectionOutcome::Complete);
        assert_eq!(daily.daily_candidates().len(), 1);
        assert_eq!(daily.daily_candidates()[0].tokens.total_tokens(), 39);
        assert!(daily.daily_candidates()[0]
            .model_breakdowns
            .iter()
            .all(|model| model.raw_model_id != "provider-v2/model"));

        let events = diagnostics.events();
        let ignored: Vec<_> = events
            .iter()
            .filter(|event| event.code.as_str() == "opencode.incomplete_generation_ignored")
            .collect();
        assert_eq!(ignored.len(), 2);
        assert!(ignored
            .iter()
            .all(|event| event.severity == DiagnosticSeverity::Info));
        for event in ignored {
            let context = event.context.as_ref().expect("context").as_str();
            assert!(context.contains(r#""ignoredGeneration":"v2""#));
            assert!(context.contains(r#""reason":"missing_session_table""#));
            assert!(context.contains(r#""selectedGenerations":"v1""#));
            assert!(!context.contains("session-v1-active"));
            assert!(!context.contains("message-v1"));
            assert!(!context.contains(fixture.directory.path().to_string_lossy().as_ref()));
        }
    }

    #[test]
    fn uncovered_residual_generation_fails_closed() {
        let fixture = Fixture::new(true, true);
        let connection = fixture.source();
        insert_v1_session(
            &connection,
            "session-v1-active",
            Counters::message(5),
            0.25,
            1_000,
        );
        insert_v1_message(&connection, "message-v1", "session-v1-active", 5, 1_100);
        insert_v2_message(
            &connection,
            "residual-v2a",
            "session-v1-active",
            9,
            1_200,
            true,
        );
        insert_v2_message(
            &connection,
            "residual-v2b",
            "session-v1-active",
            10,
            1_250,
            true,
        );
        drop(connection);
        fixture
            .source()
            .execute("DROP TABLE session_v2", [])
            .expect("drop residual V2 session table");
        let collector = fixture.collector();

        let failure = collector
            .collect(
                session_request("uncovered", SourceKey::OpenCode, Utc::now()),
                &NeverCancelled,
            )
            .expect_err("residual rows exceed the selected generation");

        assert_eq!(failure.code, CollectorFailureCode::IncompatibleEnvelope);
    }

    #[test]
    fn mixed_generation_repeated_refresh_is_idempotent() {
        let fixture = Fixture::new(true, true);
        let connection = fixture.source();
        insert_v1_session(
            &connection,
            "session-v1-active",
            Counters::message(5),
            0.25,
            1_000,
        );
        insert_v1_message(&connection, "message-v1", "session-v1-active", 5, 1_100);
        insert_v2_message(
            &connection,
            "message-v1",
            "session-v1-active",
            9,
            1_200,
            true,
        );
        drop(connection);
        fixture
            .source()
            .execute("DROP TABLE session_v2", [])
            .expect("drop residual V2 session table");
        let collector = fixture.collector();

        let first = collector
            .collect(
                session_request("mixed-first", SourceKey::OpenCode, Utc::now()),
                &NeverCancelled,
            )
            .expect("first collection");
        let second = collector
            .collect(
                session_request("mixed-second", SourceKey::OpenCode, Utc::now()),
                &NeverCancelled,
            )
            .expect("second collection");

        let first = &first.session_candidates()[0];
        let second = &second.session_candidates()[0];
        assert_eq!(first.tokens, second.tokens);
        assert_eq!(first.cost, second.cost);
        assert_eq!(first.model_breakdowns, second.model_breakdowns);
    }

    #[test]
    fn no_complete_generation_fails_as_incompatible_envelope() {
        let fixture = Fixture::new(true, false);
        let incompatible_path = fixture.directory.path().join("partial.db");
        {
            let connection = Connection::open(&incompatible_path).expect("partial database");
            connection
                .execute(
                    "CREATE TABLE session (
                        id TEXT, cost REAL, tokens_input INTEGER, tokens_output INTEGER,
                        tokens_reasoning INTEGER, tokens_cache_read INTEGER,
                        tokens_cache_write INTEGER, time_created INTEGER, time_updated INTEGER
                    )",
                    [],
                )
                .expect("incomplete V1 session table");
            connection
                .execute(
                    "CREATE TABLE session_message (
                        id TEXT, session_id TEXT, type TEXT, seq INTEGER,
                        time_created INTEGER, time_updated INTEGER, data TEXT
                    )",
                    [],
                )
                .expect("residual V2 detail table");
        }
        let collector = OpenCodeCollector::from_database_path(incompatible_path, fixture.ledger());

        let failure = collector
            .collect(
                session_request("no-complete", SourceKey::OpenCode, Utc::now()),
                &NeverCancelled,
            )
            .expect_err("no complete generation");

        assert_eq!(failure.code, CollectorFailureCode::IncompatibleEnvelope);
    }

    #[test]
    fn combined_database_prefers_v2_overlap_and_keeps_v1_only_detail() {
        let fixture = Fixture::new(true, true);
        let connection = fixture.source();
        insert_v1_session(
            &connection,
            "session-shared",
            Counters::new(11, 4, 6, 8, 10),
            0.75,
            1_000,
        );
        insert_v2_session(
            &connection,
            "session-shared",
            Counters::new(11, 4, 6, 8, 10),
            0.75,
            1_000,
        );
        insert_v1_message(&connection, "message-overlap", "session-shared", 3, 1_100);
        insert_v2_message(
            &connection,
            "message-overlap",
            "session-shared",
            7,
            1_100,
            true,
        );
        insert_v1_message(&connection, "message-v1-only", "session-shared", 4, 1_200);
        drop(connection);

        let result = fixture
            .collector()
            .collect(
                session_request("combined", SourceKey::OpenCode, Utc::now()),
                &NeverCancelled,
            )
            .expect("combined collection");
        let candidate = &result.session_candidates()[0];

        assert_eq!(candidate.tokens.total_tokens(), 39);
        assert_eq!(candidate.model_breakdowns.len(), 2);
        assert!(candidate
            .model_breakdowns
            .iter()
            .any(|model| model.raw_model_id == "provider-v2/model"));
        assert!(candidate
            .model_breakdowns
            .iter()
            .any(|model| model.raw_model_id == "provider-v1/model"));
    }

    #[test]
    fn unchanged_header_reuses_ledger_after_source_detail_compacts() {
        let fixture = Fixture::new(false, true);
        let connection = fixture.source();
        insert_v2_session(
            &connection,
            "session-stable",
            Counters::message(9),
            0.5,
            1_000,
        );
        insert_v2_message(
            &connection,
            "message-stable",
            "session-stable",
            9,
            1_100,
            true,
        );
        drop(connection);
        let collector = fixture.collector();

        let first = collector
            .collect(
                session_request("stable-full", SourceKey::OpenCode, Utc::now()),
                &NeverCancelled,
            )
            .expect("first collection");
        fixture
            .source()
            .execute("DELETE FROM session_message", [])
            .expect("compact detail");
        let second = collector
            .collect(
                incremental_session_request("stable-incremental"),
                &NeverCancelled,
            )
            .expect("cached collection");

        let first = &first.session_candidates()[0];
        let second = &second.session_candidates()[0];
        assert_eq!(first.source_key, second.source_key);
        assert_eq!(first.tokens, second.tokens);
        assert_eq!(first.cost, second.cost);
        assert_eq!(first.model_breakdowns, second.model_breakdowns);
    }

    #[test]
    fn incomplete_v2_response_cannot_establish_complete_result_and_recovers_on_retry() {
        let fixture = Fixture::new(false, true);
        let connection = fixture.source();
        insert_v2_session(
            &connection,
            "session-live",
            Counters::message(8),
            0.5,
            1_000,
        );
        insert_v2_message(&connection, "message-live", "session-live", 8, 1_100, false);
        drop(connection);
        let collector = fixture.collector();

        let failure = collector
            .collect(
                session_request("live", SourceKey::OpenCode, Utc::now()),
                &NeverCancelled,
            )
            .expect_err("all usage is still live");
        assert_eq!(failure.code, CollectorFailureCode::AllRecordsRejected);

        let payload = v2_payload(1_100, 8, true).to_string();
        fixture
            .source()
            .execute(
                "UPDATE session_message SET data = ?1 WHERE id = 'message-live'",
                [payload],
            )
            .expect("complete response");
        let completed = collector
            .collect(
                session_request("live-retry", SourceKey::OpenCode, Utc::now()),
                &NeverCancelled,
            )
            .expect("completed retry");
        assert_eq!(completed.outcome(), CollectionOutcome::Complete);
        assert_eq!(completed.session_candidates()[0].tokens.total_tokens(), 22);
    }

    #[test]
    fn completed_v2_error_envelope_recovers_session_counters_without_exact_usage() {
        let fixture = Fixture::new(false, true);
        let connection = fixture.source();
        insert_v2_session(
            &connection,
            "session-error",
            Counters::message(8),
            0.5,
            1_000,
        );
        let payload = json!({
            "model": {"providerID": "provider-v2", "id": "model"},
            "time": {"created": 1_100, "completed": 1_101},
            "finish": "error",
            "error": {"name": "ProviderError"}
        })
        .to_string();
        connection
            .execute(
                "INSERT INTO session_message (
                    id, session_id, type, seq, time_created, time_updated, data
                 ) VALUES ('message-error', 'session-error', 'assistant', 1, 1_100, 1_101, ?1)",
                [payload],
            )
            .expect("V2 error message");
        drop(connection);

        let result = fixture
            .collector()
            .collect(
                session_request("error-envelope", SourceKey::OpenCode, Utc::now()),
                &NeverCancelled,
            )
            .expect("error envelope collection");

        assert_eq!(result.outcome(), CollectionOutcome::Complete);
        assert_eq!(result.session_candidates()[0].tokens.total_tokens(), 22);
        assert_eq!(
            result.session_candidates()[0].provenance.data_quality,
            crate::domain::usage::DataQuality::Partial
        );
    }

    #[test]
    fn unchanged_stale_incomplete_rows_recover_only_after_a_deferred_observation() {
        let stale = Fixture::new(false, true);
        let connection = stale.source();
        insert_v2_session(
            &connection,
            "session-stale-incomplete",
            Counters::message(8),
            0.5,
            1_000,
        );
        insert_v2_message(
            &connection,
            "message-stale-incomplete",
            "session-stale-incomplete",
            8,
            1_100,
            false,
        );
        drop(connection);
        let collector = stale.collector();

        let first = collector.collect(
            session_request("stale-first", SourceKey::OpenCode, Utc::now()),
            &NeverCancelled,
        );
        assert_eq!(
            first.expect_err("first observation stays deferred").code,
            CollectorFailureCode::AllRecordsRejected
        );
        let recovered = collector
            .collect(
                session_request("stale-second", SourceKey::OpenCode, Utc::now()),
                &NeverCancelled,
            )
            .expect("stable retry recovers cumulative usage");
        assert_eq!(recovered.outcome(), CollectionOutcome::Complete);
        assert_eq!(recovered.session_candidates()[0].tokens.total_tokens(), 22);
        assert_eq!(
            recovered.session_candidates()[0].provenance.data_quality,
            crate::domain::usage::DataQuality::Partial
        );
        let repeated = collector
            .collect(
                session_request("stale-third", SourceKey::OpenCode, Utc::now()),
                &NeverCancelled,
            )
            .expect("stable incomplete rows stay reconciled");
        assert_eq!(repeated.outcome(), CollectionOutcome::Complete);
        assert_eq!(
            repeated.session_candidates()[0].tokens,
            recovered.session_candidates()[0].tokens
        );

        let recent = Fixture::new(false, true);
        let connection = recent.source();
        let now_ms = Utc::now().timestamp_millis();
        insert_v2_session(
            &connection,
            "session-recent-incomplete",
            Counters::message(8),
            0.5,
            now_ms,
        );
        insert_v2_message(
            &connection,
            "message-recent-incomplete",
            "session-recent-incomplete",
            8,
            now_ms,
            false,
        );
        drop(connection);
        let collector = recent.collector();
        for collection_id in ["recent-first", "recent-second"] {
            assert_eq!(
                collector
                    .collect(
                        session_request(collection_id, SourceKey::OpenCode, Utc::now()),
                        &NeverCancelled,
                    )
                    .expect_err("recent response remains deferred")
                    .code,
                CollectorFailureCode::AllRecordsRejected
            );
        }
    }

    #[test]
    fn cancellation_between_pages_fails_safely_and_retry_is_complete() {
        let fixture = Fixture::new(false, true);
        let connection = fixture.source();
        insert_v2_session(
            &connection,
            "session-cancel",
            Counters::new(6, 6, 9, 12, 15),
            1.5,
            1_000,
        );
        for index in 0..3 {
            insert_v2_message(
                &connection,
                &format!("message-{index}"),
                "session-cancel",
                i64::from(index + 1),
                1_100 + i64::from(index),
                true,
            );
        }
        drop(connection);
        let collector = fixture.collector().with_page_sizes(1, 1);
        let request = || session_request("cancel", SourceKey::OpenCode, Utc::now());

        let failure = collector
            .collect(request(), &CancelAfter::new(5))
            .expect_err("cancelled page scan");
        assert_eq!(failure.code, CollectorFailureCode::Cancelled);
        let retry = collector
            .collect(request(), &NeverCancelled)
            .expect("retry");
        assert_eq!(retry.outcome(), CollectionOutcome::Complete);
        assert_eq!(retry.session_candidates()[0].tokens.total_tokens(), 48);
    }

    #[test]
    fn counter_regression_is_partial_and_records_only_bounded_counters() {
        let fixture = Fixture::new(false, true);
        let diagnostics = Arc::new(RecordingDiagnostics::default());
        let collector = fixture
            .collector()
            .with_diagnostic_recorder(diagnostics.clone());
        let connection = fixture.source();
        insert_v2_session(
            &connection,
            "session-regressed",
            Counters::message(9),
            0.5,
            1_000,
        );
        insert_v2_message(
            &connection,
            "message-regressed",
            "session-regressed",
            9,
            1_100,
            true,
        );
        drop(connection);
        collector
            .collect(
                session_request("before", SourceKey::OpenCode, Utc::now()),
                &NeverCancelled,
            )
            .expect("initial collection");

        let connection = fixture.source();
        update_v2_session(
            &connection,
            "session-regressed",
            Counters::message(4),
            0.5,
            2_000,
        );
        let payload = v2_payload(1_100, 4, true).to_string();
        connection
            .execute(
                "UPDATE session_message SET data = ?1 WHERE id = 'message-regressed'",
                [payload],
            )
            .expect("regress detail");
        drop(connection);

        let regressed = collector
            .collect(
                session_request("after", SourceKey::OpenCode, Utc::now()),
                &NeverCancelled,
            )
            .expect("partial regression collection");
        assert_eq!(regressed.outcome(), CollectionOutcome::Partial);
        let events = diagnostics.events();
        let regression = events
            .iter()
            .find(|event| event.code.as_str() == "opencode.session_counter_regressed")
            .expect("regression diagnostic");
        let context = regression.context.as_ref().expect("context").as_str();
        assert!(context.contains(r#""counterRegressions":1"#));
        for event in &events {
            let context = event.context.as_ref().expect("context").as_str();
            assert!(!context.contains("session-regressed"));
            assert!(!context.contains(fixture.directory.path().to_string_lossy().as_ref()));
        }
    }

    #[test]
    fn incremental_deferred_live_row_does_not_reread_the_completed_tail() {
        let fixture = Fixture::new(false, true);
        let updated_at = Utc::now().timestamp_millis();
        let connection = fixture.source();
        insert_v2_session(
            &connection,
            "session-hole",
            Counters::new(11, 4, 6, 8, 10),
            1.0,
            updated_at,
        );
        insert_v2_message(&connection, "message-1", "session-hole", 5, 1_100, true);
        insert_v2_message(&connection, "message-2", "session-hole", 4, 1_200, false);
        insert_v2_message(&connection, "message-3", "session-hole", 6, 1_300, true);
        drop(connection);
        let diagnostics = Arc::new(RecordingDiagnostics::default());
        let collector = fixture
            .collector()
            .with_diagnostic_recorder(diagnostics.clone());

        let first = collector
            .collect(incremental_session_request("hole-first"), &NeverCancelled)
            .expect("first incremental collection");
        assert_eq!(first.outcome(), CollectionOutcome::Partial);
        assert_eq!(first.session_candidates()[0].tokens.total_tokens(), 39);
        assert_eq!(latest_stat(&diagnostics, "messagesRead"), 2);
        assert!(latest_stat(&diagnostics, "messagePages") >= 1);
        let ledger = Connection::open(&fixture.ledger_path).expect("ledger reader");
        assert_eq!(
            checkpoint_cursor(&ledger, "session-hole").as_deref(),
            Some("message-3")
        );
        assert_eq!(
            deferred_message_ids(&ledger, "session-hole"),
            vec!["message-2".to_owned()]
        );
        let reconciled_at = checkpoint_reconciled_at(&ledger, "session-hole");
        drop(ledger);
        while Utc::now().timestamp_millis() <= reconciled_at {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }

        let second = collector
            .collect(incremental_session_request("hole-second"), &NeverCancelled)
            .expect("unchanged incremental collection");
        assert_eq!(second.outcome(), CollectionOutcome::Partial);
        assert_eq!(
            second.session_candidates()[0].tokens,
            first.session_candidates()[0].tokens
        );
        assert_eq!(latest_stat(&diagnostics, "messagesRead"), 0);
        assert_eq!(latest_stat(&diagnostics, "messagePages"), 0);
        let ledger = Connection::open(&fixture.ledger_path).expect("ledger reader");
        assert_eq!(
            checkpoint_cursor(&ledger, "session-hole").as_deref(),
            Some("message-3")
        );
        assert_eq!(
            deferred_message_ids(&ledger, "session-hole"),
            vec!["message-2".to_owned()]
        );
        assert!(checkpoint_reconciled_at(&ledger, "session-hole") > reconciled_at);
        drop(ledger);

        let connection = fixture.source();
        update_v2_session(
            &connection,
            "session-hole",
            Counters::new(20, 6, 9, 12, 15),
            1.5,
            Utc::now().timestamp_millis(),
        );
        let payload = v2_payload(1_200, 9, true).to_string();
        connection
            .execute(
                "UPDATE session_message SET data = ?1 WHERE id = 'message-2'",
                [payload],
            )
            .expect("complete deferred message");
        drop(connection);
        let third = collector
            .collect(incremental_session_request("hole-third"), &NeverCancelled)
            .expect("completed deferred message");
        assert_eq!(third.outcome(), CollectionOutcome::Complete);
        assert_eq!(latest_stat(&diagnostics, "messagesRead"), 1);
        assert_eq!(latest_stat(&diagnostics, "messagePages"), 0);
        assert_eq!(third.session_candidates()[0].tokens.total_tokens(), 62);
        assert_eq!(model_input(&third), 20);
        let ledger = Connection::open(&fixture.ledger_path).expect("ledger reader");
        assert_eq!(
            checkpoint_cursor(&ledger, "session-hole").as_deref(),
            Some("message-3")
        );
        assert!(deferred_message_ids(&ledger, "session-hole").is_empty());
    }

    #[test]
    fn incremental_collect_reads_only_new_messages_and_matches_full_totals() {
        let fixture = Fixture::new(true, false);
        let connection = fixture.source();
        insert_v1_session(
            &connection,
            "session-append",
            Counters::message(5),
            0.25,
            1_000,
        );
        insert_v1_message(&connection, "message-1", "session-append", 5, 1_100);
        drop(connection);
        let diagnostics = Arc::new(RecordingDiagnostics::default());
        let (collector, ledger) = fixture.counting_collector(diagnostics.clone());

        let first = collector
            .collect(incremental_session_request("append-first"), &NeverCancelled)
            .expect("first incremental collection");
        assert_eq!(first.session_candidates()[0].tokens.total_tokens(), 19);
        assert_eq!(latest_stat(&diagnostics, "messagesRead"), 1);
        assert_eq!(ledger.session_record_reads(), 0);

        let second = collector
            .collect(
                incremental_session_request("append-second"),
                &NeverCancelled,
            )
            .expect("unchanged incremental collection");
        assert_eq!(
            second.session_candidates()[0].tokens,
            first.session_candidates()[0].tokens
        );
        assert_eq!(latest_stat(&diagnostics, "messagesRead"), 0);
        assert_eq!(latest_stat(&diagnostics, "messagePages"), 0);
        assert_eq!(ledger.session_record_reads(), 0);
        assert!(ledger.scoped_folds() >= 1);
        assert_stats_are_redacted(&diagnostics, &fixture, &["session-append", "message-1"]);

        let connection = fixture.source();
        update_v1_session(
            &connection,
            "session-append",
            Counters::new(11, 4, 6, 8, 10),
            0.50,
            2_000,
        );
        insert_v1_message(&connection, "message-2", "session-append", 6, 1_200);
        drop(connection);
        let third = collector
            .collect(incremental_session_request("append-third"), &NeverCancelled)
            .expect("appended incremental collection");
        assert_eq!(latest_stat(&diagnostics, "messagesRead"), 1);
        assert_eq!(third.session_candidates()[0].tokens.total_tokens(), 39);

        let full = fixture
            .isolated_collector("append-full.sqlite3")
            .collect(
                session_request("append-full", SourceKey::OpenCode, Utc::now()),
                &NeverCancelled,
            )
            .expect("full session collection");
        assert_eq!(
            third.session_candidates()[0].tokens,
            full.session_candidates()[0].tokens
        );
        assert_eq!(
            third.session_candidates()[0].model_breakdowns,
            full.session_candidates()[0].model_breakdowns
        );

        let (start, end) = wide_dates();
        let daily = collector
            .collect(
                incremental_daily_request("append-daily", "UTC", start, end),
                &NeverCancelled,
            )
            .expect("incremental daily collection");
        let full_daily = fixture
            .isolated_collector("append-daily-full.sqlite3")
            .collect(
                daily_request(
                    "append-daily-full",
                    SourceKey::OpenCode,
                    CollectionScope::Full,
                    "UTC",
                    Utc::now(),
                ),
                &NeverCancelled,
            )
            .expect("full daily collection");
        assert_eq!(
            daily.daily_candidates()[0].tokens,
            full_daily.daily_candidates()[0].tokens
        );
    }

    #[test]
    fn incremental_keeps_older_correction_until_full_reread() {
        let fixture = Fixture::new(true, false);
        let connection = fixture.source();
        insert_v1_session(
            &connection,
            "session-correct",
            Counters::message(5),
            0.25,
            1_000,
        );
        insert_v1_message(&connection, "message-1", "session-correct", 5, 1_100);
        drop(connection);
        let diagnostics = Arc::new(RecordingDiagnostics::default());
        let collector = fixture
            .collector()
            .with_diagnostic_recorder(diagnostics.clone());

        collector
            .collect(
                incremental_session_request("correct-first"),
                &NeverCancelled,
            )
            .expect("initial incremental collection");

        let connection = fixture.source();
        update_v1_session(
            &connection,
            "session-correct",
            Counters::message(3),
            0.25,
            2_000,
        );
        update_v1_message(&connection, "message-1", 3, 1_100);
        drop(connection);
        let corrected = collector
            .collect(
                incremental_session_request("correct-second"),
                &NeverCancelled,
            )
            .expect("incremental collection keeps the stored correction boundary");
        assert_eq!(latest_stat(&diagnostics, "messagesRead"), 0);
        assert_eq!(model_input(&corrected), 5);

        let repaired = collector
            .collect(
                session_request("correct-full", SourceKey::OpenCode, Utc::now()),
                &NeverCancelled,
            )
            .expect("full collection repairs the older message");
        assert!(latest_stat(&diagnostics, "messagesRead") >= 1);
        assert_eq!(model_input(&repaired), 3);
        assert_eq!(repaired.session_candidates()[0].tokens.total_tokens(), 17);
    }

    #[test]
    fn incremental_token_growth_reconciles_when_source_cost_trails_accepted_cost() {
        let fixture = Fixture::new(true, false);
        let connection = fixture.source();
        insert_v1_session(
            &connection,
            "session-rounding",
            Counters::new(2, 4, 6, 8, 10),
            0.000101,
            1_000,
        );
        insert_v1_message_cost(
            &connection,
            "message-1",
            "session-rounding",
            1,
            1_100,
            0.0000505,
        );
        insert_v1_message_cost(
            &connection,
            "message-2",
            "session-rounding",
            1,
            1_200,
            0.0000505,
        );
        drop(connection);
        let diagnostics = Arc::new(RecordingDiagnostics::default());
        let collector = fixture
            .collector()
            .with_diagnostic_recorder(diagnostics.clone());

        let first = collector
            .collect(
                incremental_session_request("rounding-first"),
                &NeverCancelled,
            )
            .expect("priced rows round one micro above the header");
        assert_eq!(first.session_candidates()[0].tokens.total_tokens(), 30);
        let ledger = Connection::open(&fixture.ledger_path).expect("ledger reader");
        assert_eq!(
            checkpoint_costs(&ledger, "session-rounding"),
            (Some(102), Some(101))
        );
        assert_eq!(
            checkpoint_cursor(&ledger, "session-rounding").as_deref(),
            Some("message-2")
        );
        drop(ledger);

        let connection = fixture.source();
        update_v1_session(
            &connection,
            "session-rounding",
            Counters::new(3, 6, 9, 12, 15),
            0.000101,
            2_000,
        );
        insert_v1_message_cost(&connection, "message-3", "session-rounding", 1, 1_300, 0.0);
        drop(connection);
        let second = collector
            .collect(
                incremental_session_request("rounding-second"),
                &NeverCancelled,
            )
            .expect("token growth reconciles inside the observed source cost");
        assert_eq!(second.session_candidates()[0].tokens.total_tokens(), 45);
        assert_eq!(latest_stat(&diagnostics, "messagesRead"), 1);
        let ledger = Connection::open(&fixture.ledger_path).expect("ledger reader");
        assert_eq!(
            checkpoint_cursor(&ledger, "session-rounding").as_deref(),
            Some("message-3")
        );
        assert_eq!(
            checkpoint_costs(&ledger, "session-rounding"),
            (Some(102), Some(101))
        );
    }

    #[test]
    fn unchanged_deferred_header_point_reads_a_completed_message() {
        let fixture = Fixture::new(false, true);
        let updated_at = Utc::now().timestamp_millis();
        let connection = fixture.source();
        insert_v2_session(
            &connection,
            "session-point",
            Counters::new(15, 6, 9, 12, 15),
            1.5,
            updated_at,
        );
        insert_v2_message(&connection, "message-1", "session-point", 5, 1_100, true);
        insert_v2_message(&connection, "message-2", "session-point", 4, 1_200, false);
        insert_v2_message(&connection, "message-3", "session-point", 6, 1_300, true);
        drop(connection);
        let diagnostics = Arc::new(RecordingDiagnostics::default());
        let collector = fixture
            .collector()
            .with_diagnostic_recorder(diagnostics.clone());

        let first = collector
            .collect(incremental_session_request("point-first"), &NeverCancelled)
            .expect("deferred live write");
        assert_eq!(first.outcome(), CollectionOutcome::Partial);
        let ledger = Connection::open(&fixture.ledger_path).expect("ledger reader");
        assert_eq!(
            checkpoint_cursor(&ledger, "session-point").as_deref(),
            Some("message-3")
        );
        assert_eq!(
            deferred_message_ids(&ledger, "session-point"),
            vec!["message-2".to_owned()]
        );
        drop(ledger);

        let connection = fixture.source();
        let payload = v2_payload(1_200, 4, true).to_string();
        connection
            .execute(
                "UPDATE session_message SET data = ?1 WHERE id = 'message-2'",
                [payload],
            )
            .expect("complete deferred message without a header change");
        drop(connection);
        let second = collector
            .collect(incremental_session_request("point-second"), &NeverCancelled)
            .expect("point-read completed deferred message");
        assert_eq!(second.outcome(), CollectionOutcome::Complete);
        assert_eq!(latest_stat(&diagnostics, "messagesRead"), 1);
        assert_eq!(latest_stat(&diagnostics, "messagePages"), 0);
        assert_eq!(model_input(&second), 15);
        let ledger = Connection::open(&fixture.ledger_path).expect("ledger reader");
        assert_eq!(
            checkpoint_cursor(&ledger, "session-point").as_deref(),
            Some("message-3")
        );
        assert!(deferred_message_ids(&ledger, "session-point").is_empty());
    }

    #[test]
    fn failed_reconcile_leaves_the_cursor_and_deferred_set() {
        let fixture = Fixture::new(false, true);
        let updated_at = Utc::now().timestamp_millis();
        let connection = fixture.source();
        insert_v2_session(
            &connection,
            "session-fail",
            Counters::new(11, 4, 6, 8, 10),
            1.0,
            updated_at,
        );
        insert_v2_message(&connection, "message-1", "session-fail", 5, 1_100, true);
        insert_v2_message(&connection, "message-2", "session-fail", 4, 1_200, false);
        insert_v2_message(&connection, "message-3", "session-fail", 6, 1_300, true);
        drop(connection);
        let gate = Arc::new(ReconcileGate::new(SqliteOpenCodeUsageLedgerStore::new(
            Database::open(&fixture.ledger_path).expect("ledger connection"),
        )));
        let collector = OpenCodeCollector::from_database_path(
            &fixture.source_path,
            Arc::clone(&gate) as Arc<dyn OpenCodeUsageLedger>,
        );

        collector
            .collect(incremental_session_request("fail-first"), &NeverCancelled)
            .expect("store the cursor before reconcile fails");
        let ledger = Connection::open(&fixture.ledger_path).expect("ledger reader");
        assert_eq!(
            checkpoint_cursor(&ledger, "session-fail").as_deref(),
            Some("message-3")
        );
        assert_eq!(
            deferred_message_ids(&ledger, "session-fail"),
            vec!["message-2".to_owned()]
        );
        drop(ledger);

        gate.fail_next();
        let failure = collector
            .collect(incremental_session_request("fail-second"), &NeverCancelled)
            .expect_err("reconcile failure");
        assert_eq!(failure.code, CollectorFailureCode::Internal);
        let ledger = Connection::open(&fixture.ledger_path).expect("ledger reader");
        assert_eq!(
            checkpoint_cursor(&ledger, "session-fail").as_deref(),
            Some("message-3")
        );
        assert_eq!(
            deferred_message_ids(&ledger, "session-fail"),
            vec!["message-2".to_owned()]
        );
    }

    #[test]
    fn incremental_daily_excludes_activity_outside_the_local_day() {
        let in_scope_ms = chrono::DateTime::parse_from_rfc3339("2026-01-01T16:30:00Z")
            .expect("in-scope timestamp")
            .timestamp_millis();
        let out_of_scope_ms = chrono::DateTime::parse_from_rfc3339("2026-01-01T17:30:00Z")
            .expect("out-of-scope timestamp")
            .timestamp_millis();
        let fixture = Fixture::new(true, false);
        let connection = fixture.source();
        insert_v1_session(
            &connection,
            "session-timezone",
            Counters::new(12, 4, 6, 8, 10),
            0.50,
            out_of_scope_ms,
        );
        insert_v1_message(
            &connection,
            "message-in",
            "session-timezone",
            5,
            in_scope_ms,
        );
        insert_v1_message(
            &connection,
            "message-out",
            "session-timezone",
            7,
            out_of_scope_ms,
        );
        drop(connection);
        let diagnostics = Arc::new(RecordingDiagnostics::default());
        let (collector, ledger) = fixture.counting_collector(diagnostics.clone());
        let day = chrono::NaiveDate::from_ymd_opt(2026, 1, 1).expect("scope day");

        let first = collector
            .collect(
                incremental_daily_request("timezone-first", "Asia/Jakarta", day, day),
                &NeverCancelled,
            )
            .expect("first scoped daily collection");
        assert_eq!(first.daily_candidates().len(), 1);
        assert_eq!(first.daily_candidates()[0].tokens.total_tokens(), 19);
        assert_eq!(latest_stat(&diagnostics, "messagesRead"), 2);

        let folds_after_first = ledger.scoped_folds();
        let second = collector
            .collect(
                incremental_daily_request("timezone-second", "Asia/Jakarta", day, day),
                &NeverCancelled,
            )
            .expect("second scoped daily collection");
        assert_eq!(latest_stat(&diagnostics, "messagesRead"), 0);
        assert_eq!(second.daily_candidates()[0].tokens.total_tokens(), 19);
        assert!(ledger.scoped_folds() > folds_after_first);
        assert_eq!(ledger.session_record_reads(), 0);
        assert_stats_are_redacted(
            &diagnostics,
            &fixture,
            &[
                "session-timezone",
                "message-in",
                "message-out",
                "PRIVATE_PROMPT_SENTINEL",
            ],
        );
    }

    #[test]
    fn incremental_skips_sessions_outside_the_scope_without_loading_records() {
        let out_of_scope_ms = chrono::DateTime::parse_from_rfc3339("2026-01-01T17:30:00Z")
            .expect("out-of-scope timestamp")
            .timestamp_millis();
        let fixture = Fixture::new(true, false);
        let connection = fixture.source();
        insert_v1_session(
            &connection,
            "session-outside",
            Counters::message(5),
            0.25,
            out_of_scope_ms,
        );
        insert_v1_message(
            &connection,
            "message-outside",
            "session-outside",
            5,
            out_of_scope_ms,
        );
        drop(connection);
        let diagnostics = Arc::new(RecordingDiagnostics::default());
        let (collector, ledger) = fixture.counting_collector(diagnostics.clone());
        let day = chrono::NaiveDate::from_ymd_opt(2026, 1, 1).expect("scope day");
        let request = || incremental_daily_request("outside", "Asia/Jakarta", day, day);

        let first = collector
            .collect(request(), &NeverCancelled)
            .expect("first collection reads the session once");
        assert_eq!(first.outcome(), CollectionOutcome::Empty);
        assert_eq!(latest_stat(&diagnostics, "messagesRead"), 1);
        assert_eq!(ledger.scoped_folds(), 0);
        assert_eq!(ledger.session_record_reads(), 0);

        let second = collector
            .collect(request(), &NeverCancelled)
            .expect("second collection skips the session");
        assert_eq!(second.outcome(), CollectionOutcome::Empty);
        assert_eq!(latest_stat(&diagnostics, "messagesRead"), 0);
        assert_eq!(latest_stat(&diagnostics, "messagePages"), 0);
        assert!(latest_stat(&diagnostics, "sessionsSkipped") >= 1);
        assert_eq!(ledger.scoped_folds(), 0);
        assert_eq!(ledger.session_record_reads(), 0);
    }

    #[test]
    fn incremental_session_keeps_lifetime_totals_across_the_catch_up_window() {
        let early = chrono::DateTime::parse_from_rfc3339("2026-10-02T12:00:00Z")
            .expect("early timestamp")
            .timestamp_millis();
        let late = chrono::DateTime::parse_from_rfc3339("2026-10-12T12:00:00Z")
            .expect("late timestamp")
            .timestamp_millis();
        let fixture = Fixture::new(true, false);
        let connection = fixture.source();
        insert_v1_session(
            &connection,
            "session-span",
            Counters::new(3, 4, 6, 8, 10),
            0.50,
            late,
        );
        insert_v1_message(&connection, "message-early", "session-span", 1, early);
        insert_v1_message(&connection, "message-late", "session-span", 2, late);
        insert_v1_session(
            &connection,
            "session-old",
            Counters::message(5),
            0.25,
            early,
        );
        insert_v1_message(&connection, "message-old", "session-old", 5, early);
        drop(connection);
        let diagnostics = Arc::new(RecordingDiagnostics::default());
        let (collector, ledger) = fixture.counting_collector(diagnostics.clone());
        let start = chrono::NaiveDate::from_ymd_opt(2026, 10, 10).expect("start");
        let end = chrono::NaiveDate::from_ymd_opt(2026, 10, 12).expect("end");
        let catch_up = CollectionScope::incremental(start, end).expect("catch-up scope");
        let session_catch_up = |collection_id: &str| {
            CollectionRequest::session(
                CollectionId::new(collection_id).expect("collection ID"),
                SourceKey::OpenCode,
                catch_up.clone(),
                Utc::now(),
            )
        };

        let first = collector
            .collect(session_catch_up("lifetime-first"), &NeverCancelled)
            .expect("first incremental session collection");
        let span = session_candidate(&first, "session-span");
        let old = session_candidate(&first, "session-old");
        assert_eq!(first.session_candidates().len(), 2);
        assert_eq!(span.tokens.total_tokens(), 31);
        assert_eq!(span.tokens.input_tokens(), Some(3));
        assert_eq!(
            span.first_activity_at,
            chrono::DateTime::from_timestamp_millis(early + 1)
        );
        assert_eq!(old.tokens.total_tokens(), 19);
        assert_eq!(latest_stat(&diagnostics, "messagesRead"), 3);
        assert_eq!(ledger.scoped_folds(), 0);

        let second = collector
            .collect(session_catch_up("lifetime-second"), &NeverCancelled)
            .expect("second incremental session collection");
        let span = session_candidate(&second, "session-span");
        assert_eq!(second.session_candidates().len(), 1);
        assert_eq!(span.tokens.total_tokens(), 31);
        assert_eq!(span.tokens.input_tokens(), Some(3));
        assert_eq!(
            span.first_activity_at,
            chrono::DateTime::from_timestamp_millis(early + 1)
        );
        assert_eq!(latest_stat(&diagnostics, "messagesRead"), 0);
        assert_eq!(latest_stat(&diagnostics, "sessionsSkipped"), 1);
        assert_eq!(ledger.scoped_folds(), 1);

        let daily = collector
            .collect(
                incremental_daily_request("lifetime-daily", "UTC", start, end),
                &NeverCancelled,
            )
            .expect("windowed daily collection");
        assert_eq!(daily.daily_candidates().len(), 1);
        assert_eq!(
            daily.daily_candidates()[0].usage_date,
            chrono::NaiveDate::from_ymd_opt(2026, 10, 12).expect("in-window day")
        );
        assert_eq!(daily.daily_candidates()[0].tokens.total_tokens(), 16);
        assert_eq!(daily.daily_candidates()[0].tokens.input_tokens(), Some(2));
        assert_eq!(ledger.scoped_folds(), 2);
    }

    fn session_candidate<'a>(
        result: &'a crate::application::collection::CollectionResult,
        session_id: &str,
    ) -> &'a crate::application::collection::SessionUsageCandidate {
        result
            .session_candidates()
            .iter()
            .find(|candidate| candidate.source_session_id == session_id)
            .unwrap_or_else(|| panic!("missing {session_id}"))
    }

    #[test]
    fn redundancy_proof_skips_the_id_scan_until_the_ignored_table_grows() {
        let fixture = Fixture::new(true, true);
        let connection = fixture.source();
        insert_v1_session(
            &connection,
            "session-proof",
            Counters::message(5),
            0.25,
            1_000,
        );
        insert_v1_message(&connection, "message-proof", "session-proof", 5, 1_100);
        insert_v2_message(
            &connection,
            "message-proof",
            "session-proof",
            9,
            1_200,
            true,
        );
        drop(connection);
        fixture
            .source()
            .execute("DROP TABLE session_v2", [])
            .expect("drop residual V2 session table");
        let diagnostics = Arc::new(RecordingDiagnostics::default());
        let collector = fixture
            .collector()
            .with_diagnostic_recorder(diagnostics.clone());

        collector
            .collect(incremental_session_request("proof-first"), &NeverCancelled)
            .expect("first proof");
        assert_eq!(latest_stat(&diagnostics, "redundancyScans"), 1);

        collector
            .collect(incremental_session_request("proof-second"), &NeverCancelled)
            .expect("stored proof skips the scan");
        assert_eq!(latest_stat(&diagnostics, "redundancyScans"), 0);
        assert_eq!(latest_stat(&diagnostics, "messagesRead"), 0);
        assert_stats_are_redacted(
            &diagnostics,
            &fixture,
            &[
                "session-proof",
                "message-proof",
                "PRIVATE_PROMPT_SENTINEL",
                "PRIVATE_RESPONSE_SENTINEL",
            ],
        );

        collector
            .collect(
                session_request("proof-full", SourceKey::OpenCode, Utc::now()),
                &NeverCancelled,
            )
            .expect("full scope scans despite a stored proof");
        assert_eq!(latest_stat(&diagnostics, "redundancyScans"), 1);

        fixture
            .source()
            .execute(
                "INSERT INTO session_message (
                    id, session_id, type, seq, time_created, time_updated, data
                 ) VALUES (
                    'late-residual', 'session-proof', 'assistant', 2, 1_300, 1_300, '{}'
                 )",
                [],
            )
            .expect("unmatched ignored row");
        let failure = collector
            .collect(incremental_session_request("proof-third"), &NeverCancelled)
            .expect_err("new ignored id fails closed");
        assert_eq!(failure.code, CollectorFailureCode::IncompatibleEnvelope);
        let events = diagnostics.events();
        let failed = events
            .iter()
            .rev()
            .find(|event| event.code.as_str() == "opencode.collection_failed")
            .expect("failure diagnostic");
        let context = failed.context.as_ref().expect("context").as_str();
        assert!(!context.contains("late-residual"));
        assert!(!context.contains("session-proof"));
        assert!(!context.contains("PRIVATE_PROMPT_SENTINEL"));
        assert!(!context.contains(fixture.directory.path().to_string_lossy().as_ref()));
    }

    #[test]
    fn missing_ignored_detail_table_fails_closed() {
        let fixture = Fixture::new(true, false);
        let connection = fixture.source();
        insert_v1_session(&connection, "session-v1", Counters::message(5), 0.25, 1_000);
        insert_v1_message(&connection, "message-v1", "session-v1", 5, 1_100);
        connection
            .execute_batch(
                "CREATE TABLE session_v2 (
                    id TEXT, cost REAL, tokens_input INTEGER, tokens_output INTEGER,
                    tokens_reasoning INTEGER, tokens_cache_read INTEGER,
                    tokens_cache_write INTEGER, time_created INTEGER,
                    time_updated INTEGER, time_idle INTEGER
                );",
            )
            .expect("incomplete V2 session table");
        drop(connection);

        let failure = fixture
            .collector()
            .collect(
                session_request("missing-detail", SourceKey::OpenCode, Utc::now()),
                &NeverCancelled,
            )
            .expect_err("missing ignored detail table");
        assert_eq!(failure.code, CollectorFailureCode::IncompatibleEnvelope);
    }

    struct Fixture {
        directory: TempDir,
        source_path: PathBuf,
        ledger_path: PathBuf,
    }

    #[derive(Debug, Default, PartialEq, Eq)]
    struct AggregateTokens {
        input: u64,
        output: u64,
        cache_write: u64,
        cache_read: u64,
        reasoning: u64,
        total: u64,
    }

    fn aggregate_tokens<'a>(
        tokens: impl IntoIterator<Item = &'a crate::domain::usage::TokenUsage>,
    ) -> AggregateTokens {
        tokens
            .into_iter()
            .fold(AggregateTokens::default(), |mut total, tokens| {
                total.input = total
                    .input
                    .checked_add(tokens.input_tokens().expect("known input tokens"))
                    .expect("input total");
                total.output = total
                    .output
                    .checked_add(tokens.output_tokens().expect("known output tokens"))
                    .expect("output total");
                total.cache_write = total
                    .cache_write
                    .checked_add(
                        tokens
                            .cache_creation_tokens()
                            .expect("known cache-write tokens"),
                    )
                    .expect("cache-write total");
                total.cache_read = total
                    .cache_read
                    .checked_add(tokens.cache_read_tokens().expect("known cache-read tokens"))
                    .expect("cache-read total");
                total.reasoning = total
                    .reasoning
                    .checked_add(
                        tokens
                            .unclassified_tokens()
                            .expect("known reasoning tokens"),
                    )
                    .expect("reasoning total");
                total.total = total
                    .total
                    .checked_add(tokens.total_tokens())
                    .expect("token total");
                total
            })
    }

    impl Fixture {
        fn new(v1: bool, v2: bool) -> Self {
            let directory = TempDir::new().expect("fixture directory");
            let source_path = directory.path().join("opencode.db");
            let source = Connection::open(&source_path).expect("source database");
            if v1 {
                create_v1_schema(&source);
            }
            if v2 {
                create_v2_schema(&source);
            }
            drop(source);
            let ledger_path = directory.path().join("burnly.sqlite3");
            let mut ledger = Database::open(&ledger_path).expect("ledger database");
            ledger.migrate_to_latest().expect("ledger migrations");
            drop(ledger);
            Self {
                directory,
                source_path,
                ledger_path,
            }
        }

        fn source(&self) -> Connection {
            Connection::open(&self.source_path).expect("source connection")
        }

        fn ledger(&self) -> Arc<dyn OpenCodeUsageLedger> {
            Arc::new(SqliteOpenCodeUsageLedgerStore::new(
                Database::open(&self.ledger_path).expect("ledger connection"),
            ))
        }

        fn collector(&self) -> OpenCodeCollector {
            OpenCodeCollector::from_database_path(&self.source_path, self.ledger())
        }

        fn counting_collector(
            &self,
            diagnostics: Arc<RecordingDiagnostics>,
        ) -> (OpenCodeCollector, Arc<CountingLedger>) {
            let ledger = Arc::new(CountingLedger::new(SqliteOpenCodeUsageLedgerStore::new(
                Database::open(&self.ledger_path).expect("ledger connection"),
            )));
            let collector = OpenCodeCollector::from_database_path(
                &self.source_path,
                Arc::clone(&ledger) as Arc<dyn OpenCodeUsageLedger>,
            )
            .with_diagnostic_recorder(diagnostics);
            (collector, ledger)
        }

        fn isolated_collector(&self, name: &str) -> OpenCodeCollector {
            let ledger_path = self.directory.path().join(name);
            let mut database = Database::open(&ledger_path).expect("isolated ledger");
            database
                .migrate_to_latest()
                .expect("migrate isolated ledger");
            drop(database);
            OpenCodeCollector::from_database_path(
                &self.source_path,
                Arc::new(SqliteOpenCodeUsageLedgerStore::new(
                    Database::open(&ledger_path).expect("reopen isolated ledger"),
                )),
            )
        }
    }

    struct CancelAfter {
        calls: AtomicUsize,
        cancel_at: usize,
    }

    impl CancelAfter {
        fn new(cancel_at: usize) -> Self {
            Self {
                calls: AtomicUsize::new(0),
                cancel_at,
            }
        }
    }

    impl CancellationSignal for CancelAfter {
        fn is_cancelled(&self) -> bool {
            self.calls.fetch_add(1, Ordering::SeqCst) + 1 >= self.cancel_at
        }
    }

    struct CountingLedger {
        inner: SqliteOpenCodeUsageLedgerStore,
        session_record_reads: AtomicUsize,
        scoped_folds: AtomicUsize,
    }

    impl CountingLedger {
        fn new(inner: SqliteOpenCodeUsageLedgerStore) -> Self {
            Self {
                inner,
                session_record_reads: AtomicUsize::new(0),
                scoped_folds: AtomicUsize::new(0),
            }
        }

        fn session_record_reads(&self) -> usize {
            self.session_record_reads.load(Ordering::SeqCst)
        }

        fn scoped_folds(&self) -> usize {
            self.scoped_folds.load(Ordering::SeqCst)
        }
    }

    impl OpenCodeUsageLedger for CountingLedger {
        fn reconcile_session(
            &self,
            snapshot: &OpenCodeSessionLedgerSnapshot,
        ) -> Result<OpenCodeLedgerReconcileResult, OpenCodeUsageLedgerError> {
            self.inner.reconcile_session(snapshot)
        }

        fn read_checkpoint(
            &self,
            session_id: &str,
        ) -> Result<Option<OpenCodeSessionCheckpoint>, OpenCodeUsageLedgerError> {
            self.inner.read_checkpoint(session_id)
        }

        fn read_session_records(
            &self,
            session_id: &str,
        ) -> Result<Vec<OpenCodeLedgerRecord>, OpenCodeUsageLedgerError> {
            self.session_record_reads.fetch_add(1, Ordering::SeqCst);
            self.inner.read_session_records(session_id)
        }

        fn session_activity_bounds(
            &self,
            session_id: &str,
        ) -> Result<Option<OpenCodeActivityBounds>, OpenCodeUsageLedgerError> {
            self.inner.session_activity_bounds(session_id)
        }

        fn fold_scoped_records(
            &self,
            session_id: &str,
            activity_window_ms: Option<(i64, i64)>,
            visitor: &mut dyn FnMut(OpenCodeLedgerRecord) -> Result<(), OpenCodeUsageLedgerError>,
        ) -> Result<(), OpenCodeUsageLedgerError> {
            self.scoped_folds.fetch_add(1, Ordering::SeqCst);
            self.inner
                .fold_scoped_records(session_id, activity_window_ms, visitor)
        }

        fn store_message_cursor(
            &self,
            session_id: &str,
            cursor: Option<&str>,
        ) -> Result<(), OpenCodeUsageLedgerError> {
            self.inner.store_message_cursor(session_id, cursor)
        }

        fn read_deferred_message_ids(
            &self,
            session_id: &str,
        ) -> Result<Vec<String>, OpenCodeUsageLedgerError> {
            self.inner.read_deferred_message_ids(session_id)
        }

        fn store_message_progress(
            &self,
            session_id: &str,
            cursor: Option<&str>,
            deferred_message_ids: &[String],
        ) -> Result<(), OpenCodeUsageLedgerError> {
            self.inner
                .store_message_progress(session_id, cursor, deferred_message_ids)
        }

        fn read_redundancy_proof(
            &self,
        ) -> Result<Option<OpenCodeRedundancyProof>, OpenCodeUsageLedgerError> {
            self.inner.read_redundancy_proof()
        }

        fn write_redundancy_proof(
            &self,
            proof: &OpenCodeRedundancyProof,
        ) -> Result<(), OpenCodeUsageLedgerError> {
            self.inner.write_redundancy_proof(proof)
        }
    }

    struct ReconcileGate {
        inner: SqliteOpenCodeUsageLedgerStore,
        fail: AtomicBool,
    }

    impl ReconcileGate {
        fn new(inner: SqliteOpenCodeUsageLedgerStore) -> Self {
            Self {
                inner,
                fail: AtomicBool::new(false),
            }
        }

        fn fail_next(&self) {
            self.fail.store(true, Ordering::SeqCst);
        }
    }

    impl OpenCodeUsageLedger for ReconcileGate {
        fn reconcile_session(
            &self,
            snapshot: &OpenCodeSessionLedgerSnapshot,
        ) -> Result<OpenCodeLedgerReconcileResult, OpenCodeUsageLedgerError> {
            if self.fail.swap(false, Ordering::SeqCst) {
                return Err(OpenCodeUsageLedgerError::Storage);
            }
            self.inner.reconcile_session(snapshot)
        }

        fn read_checkpoint(
            &self,
            session_id: &str,
        ) -> Result<Option<OpenCodeSessionCheckpoint>, OpenCodeUsageLedgerError> {
            self.inner.read_checkpoint(session_id)
        }

        fn read_session_records(
            &self,
            session_id: &str,
        ) -> Result<Vec<OpenCodeLedgerRecord>, OpenCodeUsageLedgerError> {
            self.inner.read_session_records(session_id)
        }

        fn session_activity_bounds(
            &self,
            session_id: &str,
        ) -> Result<Option<OpenCodeActivityBounds>, OpenCodeUsageLedgerError> {
            self.inner.session_activity_bounds(session_id)
        }

        fn fold_scoped_records(
            &self,
            session_id: &str,
            activity_window_ms: Option<(i64, i64)>,
            visitor: &mut dyn FnMut(OpenCodeLedgerRecord) -> Result<(), OpenCodeUsageLedgerError>,
        ) -> Result<(), OpenCodeUsageLedgerError> {
            self.inner
                .fold_scoped_records(session_id, activity_window_ms, visitor)
        }

        fn store_message_cursor(
            &self,
            session_id: &str,
            cursor: Option<&str>,
        ) -> Result<(), OpenCodeUsageLedgerError> {
            self.inner.store_message_cursor(session_id, cursor)
        }

        fn read_deferred_message_ids(
            &self,
            session_id: &str,
        ) -> Result<Vec<String>, OpenCodeUsageLedgerError> {
            self.inner.read_deferred_message_ids(session_id)
        }

        fn store_message_progress(
            &self,
            session_id: &str,
            cursor: Option<&str>,
            deferred_message_ids: &[String],
        ) -> Result<(), OpenCodeUsageLedgerError> {
            self.inner
                .store_message_progress(session_id, cursor, deferred_message_ids)
        }

        fn read_redundancy_proof(
            &self,
        ) -> Result<Option<OpenCodeRedundancyProof>, OpenCodeUsageLedgerError> {
            self.inner.read_redundancy_proof()
        }

        fn write_redundancy_proof(
            &self,
            proof: &OpenCodeRedundancyProof,
        ) -> Result<(), OpenCodeUsageLedgerError> {
            self.inner.write_redundancy_proof(proof)
        }
    }

    fn incremental_session_request(collection_id: &str) -> CollectionRequest {
        CollectionRequest::session(
            CollectionId::new(collection_id).expect("collection ID"),
            SourceKey::OpenCode,
            CollectionScope::incremental(
                chrono::NaiveDate::from_ymd_opt(1970, 1, 1).expect("start date"),
                chrono::NaiveDate::from_ymd_opt(2099, 12, 31).expect("end date"),
            )
            .expect("incremental scope"),
            Utc::now(),
        )
    }

    fn wide_dates() -> (chrono::NaiveDate, chrono::NaiveDate) {
        (
            chrono::NaiveDate::from_ymd_opt(1970, 1, 1).expect("start date"),
            chrono::NaiveDate::from_ymd_opt(2099, 12, 31).expect("end date"),
        )
    }

    fn incremental_daily_request(
        collection_id: &str,
        timezone: &str,
        start: chrono::NaiveDate,
        end: chrono::NaiveDate,
    ) -> CollectionRequest {
        daily_request(
            collection_id,
            SourceKey::OpenCode,
            CollectionScope::incremental(start, end).expect("incremental scope"),
            timezone,
            Utc::now(),
        )
    }

    fn latest_stat(diagnostics: &RecordingDiagnostics, key: &str) -> u64 {
        let events = diagnostics.events();
        let event = events
            .iter()
            .rev()
            .find(|event| event.code.as_str() == "opencode.collection_stats")
            .expect("collection stats");
        let context = event.context.as_ref().expect("context").as_str();
        let value: serde_json::Value = serde_json::from_str(context).expect("stats json");
        value
            .get(key)
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_else(|| panic!("missing {key}"))
    }

    fn assert_stats_are_redacted(
        diagnostics: &RecordingDiagnostics,
        fixture: &Fixture,
        forbidden: &[&str],
    ) {
        let events = diagnostics.events();
        let event = events
            .iter()
            .rev()
            .find(|event| event.code.as_str() == "opencode.collection_stats")
            .expect("collection stats");
        let context = event.context.as_ref().expect("context").as_str();
        assert!(!context.contains(fixture.directory.path().to_string_lossy().as_ref()));
        for token in forbidden {
            assert!(
                !context.contains(token),
                "{token} leaked into collection stats"
            );
        }
    }

    fn checkpoint_cursor(ledger: &Connection, session_id: &str) -> Option<String> {
        ledger
            .query_row(
                "SELECT last_source_message_id
                 FROM opencode_session_checkpoint
                 WHERE session_id = ?1",
                [session_id],
                |row| row.get(0),
            )
            .expect("cursor")
    }

    fn checkpoint_costs(ledger: &Connection, session_id: &str) -> (Option<i64>, Option<i64>) {
        ledger
            .query_row(
                "SELECT accepted_cost_micros, observed_cost_micros
                 FROM opencode_session_checkpoint
                 WHERE session_id = ?1",
                [session_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("checkpoint costs")
    }

    fn checkpoint_reconciled_at(ledger: &Connection, session_id: &str) -> i64 {
        ledger
            .query_row(
                "SELECT last_reconciled_at_ms
                 FROM opencode_session_checkpoint
                 WHERE session_id = ?1",
                [session_id],
                |row| row.get(0),
            )
            .expect("reconciled at")
    }

    fn deferred_message_ids(ledger: &Connection, session_id: &str) -> Vec<String> {
        let mut statement = ledger
            .prepare(
                "SELECT message_id
                 FROM opencode_deferred_message
                 WHERE session_id = ?1
                 ORDER BY message_id ASC",
            )
            .expect("deferred query");
        statement
            .query_map([session_id], |row| row.get(0))
            .expect("deferred rows")
            .collect::<Result<Vec<_>, _>>()
            .expect("deferred ids")
    }

    fn model_input(result: &crate::application::collection::CollectionResult) -> u64 {
        result.session_candidates()[0].model_breakdowns[0]
            .tokens
            .input_tokens()
            .expect("model input")
    }

    #[derive(Clone, Copy)]
    struct Counters {
        input: i64,
        output: i64,
        reasoning: i64,
        cache_read: i64,
        cache_write: i64,
    }

    impl Counters {
        const fn new(
            input: i64,
            output: i64,
            reasoning: i64,
            cache_read: i64,
            cache_write: i64,
        ) -> Self {
            Self {
                input,
                output,
                reasoning,
                cache_read,
                cache_write,
            }
        }

        const fn message(input: i64) -> Self {
            Self::new(input, 2, 3, 4, 5)
        }
    }

    fn create_v1_schema(connection: &Connection) {
        connection
            .execute_batch(
                "CREATE TABLE session (
                    id TEXT PRIMARY KEY, cost REAL NOT NULL,
                    tokens_input INTEGER NOT NULL, tokens_output INTEGER NOT NULL,
                    tokens_reasoning INTEGER NOT NULL, tokens_cache_read INTEGER NOT NULL,
                    tokens_cache_write INTEGER NOT NULL, time_created INTEGER NOT NULL,
                    time_updated INTEGER NOT NULL
                );
                CREATE TABLE message (
                    id TEXT PRIMARY KEY, session_id TEXT NOT NULL,
                    time_created INTEGER NOT NULL, time_updated INTEGER NOT NULL,
                    data TEXT NOT NULL
                );",
            )
            .expect("V1 schema");
    }

    fn create_v2_schema(connection: &Connection) {
        connection
            .execute_batch(
                "CREATE TABLE session_v2 (
                    id TEXT PRIMARY KEY, cost REAL NOT NULL,
                    tokens_input INTEGER NOT NULL, tokens_output INTEGER NOT NULL,
                    tokens_reasoning INTEGER NOT NULL, tokens_cache_read INTEGER NOT NULL,
                    tokens_cache_write INTEGER NOT NULL, time_created INTEGER NOT NULL,
                    time_updated INTEGER NOT NULL, time_idle INTEGER
                );
                CREATE TABLE session_message (
                    id TEXT PRIMARY KEY, session_id TEXT NOT NULL, type TEXT NOT NULL,
                    seq INTEGER NOT NULL, time_created INTEGER NOT NULL,
                    time_updated INTEGER NOT NULL, data TEXT NOT NULL
                );",
            )
            .expect("V2 schema");
    }

    fn insert_v1_session(
        connection: &Connection,
        id: &str,
        counters: Counters,
        cost: f64,
        updated_at: i64,
    ) {
        connection
            .execute(
                "INSERT INTO session (
                    id, cost, tokens_input, tokens_output, tokens_reasoning,
                    tokens_cache_read, tokens_cache_write, time_created, time_updated
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
                params![
                    id,
                    cost,
                    counters.input,
                    counters.output,
                    counters.reasoning,
                    counters.cache_read,
                    counters.cache_write,
                    updated_at
                ],
            )
            .expect("V1 session");
    }

    fn insert_v2_session(
        connection: &Connection,
        id: &str,
        counters: Counters,
        cost: f64,
        updated_at: i64,
    ) {
        connection
            .execute(
                "INSERT INTO session_v2 (
                    id, cost, tokens_input, tokens_output, tokens_reasoning,
                    tokens_cache_read, tokens_cache_write, time_created, time_updated, time_idle
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8, ?8)",
                params![
                    id,
                    cost,
                    counters.input,
                    counters.output,
                    counters.reasoning,
                    counters.cache_read,
                    counters.cache_write,
                    updated_at
                ],
            )
            .expect("V2 session");
    }

    fn update_v2_session(
        connection: &Connection,
        id: &str,
        counters: Counters,
        cost: f64,
        updated_at: i64,
    ) {
        connection
            .execute(
                "UPDATE session_v2 SET
                    cost = ?2, tokens_input = ?3, tokens_output = ?4,
                    tokens_reasoning = ?5, tokens_cache_read = ?6,
                    tokens_cache_write = ?7, time_updated = ?8, time_idle = ?8
                 WHERE id = ?1",
                params![
                    id,
                    cost,
                    counters.input,
                    counters.output,
                    counters.reasoning,
                    counters.cache_read,
                    counters.cache_write,
                    updated_at
                ],
            )
            .expect("update V2 session");
    }

    fn update_v1_session(
        connection: &Connection,
        id: &str,
        counters: Counters,
        cost: f64,
        updated_at: i64,
    ) {
        connection
            .execute(
                "UPDATE session SET
                    cost = ?2, tokens_input = ?3, tokens_output = ?4,
                    tokens_reasoning = ?5, tokens_cache_read = ?6,
                    tokens_cache_write = ?7, time_updated = ?8
                 WHERE id = ?1",
                params![
                    id,
                    cost,
                    counters.input,
                    counters.output,
                    counters.reasoning,
                    counters.cache_read,
                    counters.cache_write,
                    updated_at
                ],
            )
            .expect("update V1 session");
    }

    fn update_v1_message(connection: &Connection, id: &str, input: i64, created_at: i64) {
        let payload = json!({
            "role": "assistant",
            "providerID": "provider-v1",
            "modelID": "model",
            "time": {"created": created_at, "completed": created_at + 1},
            "tokens": {
                "input": input, "output": 2, "reasoning": 3,
                "cache": {"read": 4, "write": 5}
            },
            "cost": 0.25,
            "content": {"prompt": "PRIVATE_PROMPT_SENTINEL"}
        })
        .to_string();
        connection
            .execute(
                "UPDATE message SET time_created = ?2, time_updated = ?2, data = ?3 WHERE id = ?1",
                params![id, created_at, payload],
            )
            .expect("update V1 message");
    }

    fn insert_v1_message(
        connection: &Connection,
        id: &str,
        session_id: &str,
        input: i64,
        created_at: i64,
    ) {
        insert_v1_message_cost(connection, id, session_id, input, created_at, 0.25);
    }

    fn insert_v1_message_cost(
        connection: &Connection,
        id: &str,
        session_id: &str,
        input: i64,
        created_at: i64,
        cost: f64,
    ) {
        let payload = json!({
            "role": "assistant",
            "providerID": "provider-v1",
            "modelID": "model",
            "time": {"created": created_at, "completed": created_at + 1},
            "tokens": {
                "input": input, "output": 2, "reasoning": 3,
                "cache": {"read": 4, "write": 5}
            },
            "cost": cost,
            "content": {"prompt": "PRIVATE_PROMPT_SENTINEL"}
        })
        .to_string();
        connection
            .execute(
                "INSERT INTO message (id, session_id, time_created, time_updated, data)
                 VALUES (?1, ?2, ?3, ?3, ?4)",
                params![id, session_id, created_at, payload],
            )
            .expect("V1 message");
    }

    fn insert_v2_message(
        connection: &Connection,
        id: &str,
        session_id: &str,
        input: i64,
        created_at: i64,
        completed: bool,
    ) {
        let payload = v2_payload(created_at, input, completed).to_string();
        connection
            .execute(
                "INSERT INTO session_message (
                    id, session_id, type, seq, time_created, time_updated, data
                 ) VALUES (?1, ?2, 'assistant', ?3, ?3, ?3, ?4)",
                params![id, session_id, created_at, payload],
            )
            .expect("V2 message");
    }

    fn v2_payload(created_at: i64, input: i64, completed: bool) -> serde_json::Value {
        let mut time = json!({"created": created_at});
        if completed {
            time["completed"] = json!(created_at + 1);
        }
        json!({
            "model": {"providerID": "provider-v2", "id": "model"},
            "time": time,
            "tokens": {
                "input": input, "output": 2, "reasoning": 3,
                "cache": {"read": 4, "write": 5}
            },
            "cost": 0.5,
            "content": {"response": "PRIVATE_RESPONSE_SENTINEL"}
        })
    }
}
