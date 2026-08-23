//! Native OpenCode collector orchestration.

use std::path::PathBuf;
use std::sync::Arc;

use chrono::Utc;

use crate::application::collection::{
    CollectionProjection, CollectionRequest, CollectionResult, CollectorDescriptor,
    CollectorFailure, CollectorFailureCode, CollectorIntegrity, DetectionRequest, DetectionResult,
    RejectedRecord,
};
use crate::application::cost::BurnlyCostCalculator;
use crate::application::diagnostics::DiagnosticSeverity;
use crate::application::ports::collector::{CancellationSignal, Collector};
use crate::application::ports::diagnostic_recorder::DiagnosticRecorder;
use crate::application::ports::opencode_usage_ledger::{
    OpenCodeExactOrigin, OpenCodeExactUsage, OpenCodeLedgerReconcileResult,
    OpenCodeReconciliationState, OpenCodeRecoveryDisposition, OpenCodeSessionLedgerSnapshot,
    OpenCodeTokenVector, OpenCodeUsageLedger,
};
use crate::domain::source::SourceKey;

use super::super::support::{
    available_detection, cancelled_detection, collection_metadata, daily_session_projections,
    detection_issue, empty_collection_result, invalid_configuration_detection,
    missing_or_invalid_location_code, not_found_detection, path_is_missing,
    record_collector_diagnostic, request_failure, single_source_descriptor, unsupported_detection,
    validate_source, validation_failure_preserving_all_rejected, CollectorDiagnosticCounter,
    CollectorIdentity, LocalCollectionRun,
};
use super::{
    default_opencode_database, map_daily, map_sessions, source_cost_usd_to_micros,
    OpenCodeGeneration, OpenCodeMappingContext, OpenCodeMessageUsage, OpenCodePageSize,
    OpenCodeSessionHeader, OpenCodeStore, OpenCodeTokenCounters,
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

        let mut store = OpenCodeStore::open_read_only(path).map_err(|_| {
            let code = missing_or_invalid_location_code(path);
            self.record_failure(&request, code, CollectionStats::default());
            request_failure(&request, code)
        })?;
        let observed_at = Utc::now();
        let (reconciled, stats) = self
            .reconcile_all_sessions(
                &mut store,
                observed_at.timestamp_millis(),
                cancellation,
                &request,
            )
            .inspect_err(|failure| {
                self.record_failure(&request, failure.code, CollectionStats::default())
            })?;
        ensure_not_cancelled(&request, cancellation)?;

        if stats.counter_regressions > 0 {
            self.record_counter_regression(&request, stats);
        }
        if stats.non_usage_error_rows > 0 {
            self.record_non_usage_errors(&request, stats);
        }
        let metadata = collection_metadata(IDENTITY, &request, run.started_at(), observed_at)?;
        let context = OpenCodeMappingContext::new(
            COLLECTOR_VERSION.to_owned(),
            request.collection_id().clone(),
            observed_at,
        )
        .map_err(|_| request_failure(&request, CollectorFailureCode::Internal))?;
        let rejections = partial_rejections(stats);

        let result = match request.projection() {
            CollectionProjection::Daily => {
                let timezone = request.aggregation_timezone().ok_or_else(|| {
                    request_failure(&request, CollectorFailureCode::ScopeNotRepresentable)
                })?;
                let candidates = map_daily(
                    &reconciled,
                    timezone,
                    request.scope(),
                    &context,
                    &self.calculator,
                )
                .map_err(|_| {
                    self.record_failure(
                        &request,
                        CollectorFailureCode::IncompatibleEnvelope,
                        stats,
                    );
                    request_failure(&request, CollectorFailureCode::IncompatibleEnvelope)
                })?;
                CollectionResult::daily(
                    metadata,
                    candidates,
                    rejections,
                    Vec::new(),
                    run.process_summary(),
                )
            }
            CollectionProjection::Session => {
                let candidates =
                    map_sessions(&reconciled, &context, &self.calculator).map_err(|_| {
                        self.record_failure(
                            &request,
                            CollectorFailureCode::IncompatibleEnvelope,
                            stats,
                        );
                        request_failure(&request, CollectorFailureCode::IncompatibleEnvelope)
                    })?;
                CollectionResult::session(
                    metadata,
                    candidates,
                    rejections,
                    Vec::new(),
                    run.process_summary(),
                )
            }
        };
        result.map_err(|error| {
            let failure = validation_failure_preserving_all_rejected(&request, error);
            self.record_failure(&request, failure.code, stats);
            failure
        })
    }
}

impl OpenCodeCollector {
    fn reconcile_all_sessions(
        &self,
        store: &mut OpenCodeStore,
        observed_at_ms: i64,
        cancellation: &dyn CancellationSignal,
        request: &CollectionRequest,
    ) -> Result<(Vec<OpenCodeLedgerReconcileResult>, CollectionStats), CollectorFailure> {
        let mut after_session_id = None::<String>;
        let mut reconciled = Vec::new();
        let mut stats = CollectionStats::default();

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
                        if !matches!(
                            request.scope(),
                            crate::application::collection::CollectionScope::Full
                        ) && checkpoint.as_ref().is_some_and(|checkpoint| {
                            checkpoint_matches_header(checkpoint, &session, cumulative_cost_micros)
                        }) {
                            let checkpoint = checkpoint.expect("matching checkpoint exists");
                            let records = self
                                .ledger
                                .read_session_records(&session.id)
                                .map_err(|_| internal(request))?;
                            work.push(SessionWork::Cached(OpenCodeLedgerReconcileResult {
                                records,
                                checkpoint,
                                exact_records_accepted: 0,
                                recovery_segments_created: 0,
                                late_exact_reclassified: 0,
                                late_exact_ignored: 0,
                                counter_regressions: 0,
                            }));
                        } else {
                            let (
                                messages,
                                incomplete_live_rows,
                                non_usage_error_rows,
                                message_pages,
                            ) = self.read_all_messages(
                                &snapshot,
                                &session.id,
                                cancellation,
                                request,
                            )?;
                            stats.message_pages = stats.message_pages.saturating_add(message_pages);
                            stats.messages_read = stats
                                .messages_read
                                .saturating_add(u64::try_from(messages.len()).unwrap_or(u64::MAX));
                            stats.non_usage_error_rows = stats
                                .non_usage_error_rows
                                .saturating_add(non_usage_error_rows);
                            let recovery_disposition =
                                match (incomplete_live_rows > 0, stable_deferred_retry) {
                                    (true, true) => OpenCodeRecoveryDisposition::StableIncomplete,
                                    (true, false) => OpenCodeRecoveryDisposition::DeferredLiveWrite,
                                    (false, _) => OpenCodeRecoveryDisposition::Ready,
                                };
                            let deferred_live_rows = u64::from(
                                recovery_disposition
                                    == OpenCodeRecoveryDisposition::DeferredLiveWrite,
                            )
                            .saturating_mul(incomplete_live_rows);
                            stats.deferred_live_rows =
                                stats.deferred_live_rows.saturating_add(deferred_live_rows);
                            work.push(SessionWork::Reconcile(
                                session_snapshot(
                                    session,
                                    messages,
                                    recovery_disposition,
                                    observed_at_ms,
                                )
                                .map_err(|()| incompatible(request))?,
                            ));
                        }
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
                let result = match work {
                    SessionWork::Cached(result) => result,
                    SessionWork::Reconcile(snapshot) => self
                        .ledger
                        .reconcile_session(&snapshot)
                        .map_err(|_| internal(request))?,
                };
                stats.include(&result);
                after_session_id = Some(result.checkpoint.session_id.clone());
                reconciled.push(result);
            }
        }
        Ok((reconciled, stats))
    }

    fn read_all_messages(
        &self,
        snapshot: &super::OpenCodeReadSnapshot<'_>,
        session_id: &str,
        cancellation: &dyn CancellationSignal,
        request: &CollectionRequest,
    ) -> Result<(Vec<OpenCodeMessageUsage>, u64, u64, u64), CollectorFailure> {
        let mut after_message_id = None::<String>;
        let mut messages = Vec::new();
        let mut deferred = 0_u64;
        let mut non_usage_error_rows = 0_u64;
        let mut pages = 0_u64;
        loop {
            ensure_not_cancelled(request, cancellation)?;
            let page = snapshot
                .read_messages_page(
                    session_id,
                    after_message_id.as_deref(),
                    self.message_page_size,
                )
                .map_err(|_| incompatible(request))?;
            if !page.has_rows() {
                break;
            }
            pages = pages.saturating_add(1);
            non_usage_error_rows = non_usage_error_rows.saturating_add(page.non_usage_error_rows);
            after_message_id = page.last_row_id;
            for message in page.messages {
                if message.generation == OpenCodeGeneration::V2 && message.completed_at_ms.is_none()
                {
                    deferred = deferred.saturating_add(1);
                } else {
                    messages.push(message);
                }
            }
        }
        Ok((messages, deferred, non_usage_error_rows, pages))
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
}

enum SessionWork {
    Cached(OpenCodeLedgerReconcileResult),
    Reconcile(OpenCodeSessionLedgerSnapshot),
}

#[derive(Debug, Clone, Copy, Default)]
struct CollectionStats {
    session_pages: u64,
    message_pages: u64,
    sessions_processed: u64,
    messages_read: u64,
    deferred_live_rows: u64,
    non_usage_error_rows: u64,
    exact_records_accepted: u64,
    recovery_segments_created: u64,
    late_exact_reclassified: u64,
    late_exact_ignored: u64,
    counter_regressions: u64,
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

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

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
        assert_eq!(failure.code, CollectorFailureCode::SourceInvalidLocation);
        let events = diagnostics.events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].code.as_str(), "opencode.collection_failed");
        let context = events[0].context.as_ref().expect("context").as_str();
        assert!(context.contains(r#""failureCode":"source.invalid_location""#));
        assert!(!context.contains("invalid-source.db"));
        assert!(!context.contains(fixture.directory.path().to_string_lossy().as_ref()));
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
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].code.as_str(),
            "opencode.session_counter_regressed"
        );
        let context = events[0].context.as_ref().expect("context").as_str();
        assert!(context.contains(r#""counterRegressions":1"#));
        assert!(!context.contains("session-regressed"));
        assert!(!context.contains(fixture.directory.path().to_string_lossy().as_ref()));
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

    fn insert_v1_message(
        connection: &Connection,
        id: &str,
        session_id: &str,
        input: i64,
        created_at: i64,
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
            "cost": 0.25,
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
