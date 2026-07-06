use std::path::PathBuf;
use std::sync::Arc;

use chrono::Utc;

use crate::application::collection::{
    CollectionProjection, CollectionRequest, CollectionResult, CollectorDescriptor,
    CollectorFailure, CollectorFailureCode, CollectorIntegrity, DetectionRequest, DetectionResult,
};
use crate::application::diagnostics::DiagnosticSeverity;
use crate::application::ports::collector::{CancellationSignal, Collector};
use crate::application::ports::diagnostic_recorder::DiagnosticRecorder;
use crate::domain::source::SourceKey;

use super::super::support::{
    available_detection, cancelled_detection, collection_metadata, collector_key,
    daily_session_projections, detection_issue, empty_collection_result,
    invalid_configuration_detection, not_found_detection, record_collector_diagnostic,
    request_failure, single_source_descriptor, unsupported_detection, validate_source,
    validation_failure_as_internal, CollectorDiagnosticCounter, CollectorIdentity,
    LocalCollectionRun,
};
use super::detection::inspect_grok_home;
use super::grok_home::unified_log_path;
use super::mapper::{self, GrokMappingContext};
use super::model_resolver::GrokModelResolver;
use super::session_index::GrokSessionIndex;
use super::unified_log_reader::UnifiedLogReader;
use super::usage_cache::{GrokUsageCacheClient, NoOpGrokUsageCache};

const COLLECTOR_KEY: &str = "grok-build";
const DISPLAY_NAME: &str = "Grok Build";
const COLLECTOR_VERSION: &str = "local";
const ADAPTER_VERSION: u16 = 1;
const PROFILE_VERSION: u16 = 1;
const IDENTITY: CollectorIdentity = CollectorIdentity {
    key: COLLECTOR_KEY,
    display_name: DISPLAY_NAME,
    runtime_version: COLLECTOR_VERSION,
    adapter_version: ADAPTER_VERSION,
    source: SourceKey::GrokBuild,
    profile_version: PROFILE_VERSION,
};

#[derive(Clone)]
pub(crate) struct GrokCollector {
    grok_home: PathBuf,
    usage_cache: GrokUsageCacheClient,
    diagnostics: Option<Arc<dyn DiagnosticRecorder>>,
}

impl GrokCollector {
    pub(crate) fn from_grok_home(path: impl Into<PathBuf>) -> Self {
        Self {
            grok_home: path.into(),
            usage_cache: GrokUsageCacheClient::new(Arc::new(NoOpGrokUsageCache)),
            diagnostics: None,
        }
    }

    pub(crate) fn with_usage_cache(mut self, usage_cache: GrokUsageCacheClient) -> Self {
        self.usage_cache = usage_cache;
        self
    }

    pub(crate) fn with_diagnostic_recorder(
        mut self,
        diagnostics: Arc<dyn DiagnosticRecorder>,
    ) -> Self {
        self.diagnostics = Some(diagnostics);
        self
    }
}

impl Collector for GrokCollector {
    fn describe(&self) -> Result<CollectorDescriptor, CollectorFailure> {
        descriptor()
    }

    fn detect(
        &self,
        request: DetectionRequest,
        cancellation: &dyn CancellationSignal,
    ) -> Result<DetectionResult, CollectorFailure> {
        if cancellation.is_cancelled() {
            return Ok(cancelled_detection(&request));
        }
        if request.source != SourceKey::GrokBuild {
            return Ok(unsupported_detection(
                &request,
                detection_issue("grok.unsupported_source", "Source is not Grok Build."),
            ));
        }
        let inspection = inspect_grok_home(Some(self.grok_home.as_path()));
        if !inspection.grok_home_exists {
            return Ok(not_found_detection(
                &request,
                SourceKey::GrokBuild,
                supported_projections(),
                detection_issue(
                    "grok.home_missing",
                    "Grok Build data directory was not found.",
                ),
            ));
        }

        let unified_log = unified_log_path(&inspection.grok_home);
        if !inspection.unified_log_exists {
            return Ok(not_found_detection(
                &request,
                SourceKey::GrokBuild,
                supported_projections(),
                detection_issue(
                    "grok.unified_log_missing",
                    "Grok Build unified usage log was not found.",
                ),
            ));
        }

        match UnifiedLogReader::read_from_path(&unified_log) {
            Ok((rows, _)) => Ok(available_detection(
                &request,
                SourceKey::GrokBuild,
                supported_projections(),
                !rows.is_empty(),
            )),
            Err(_) => Ok(invalid_configuration_detection(
                &request,
                SourceKey::GrokBuild,
                supported_projections(),
                detection_issue(
                    "grok.unified_log_incompatible",
                    "Grok Build unified usage log is not readable by Burnly.",
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
        validate_request(&request)?;
        if cancellation.is_cancelled() {
            return Err(request_failure(&request, CollectorFailureCode::Cancelled));
        }
        if !self.grok_home.is_dir() {
            self.record_failure(
                &request,
                CollectorFailureCode::SourceNotFound,
                &[CollectorDiagnosticCounter::new("rowsFound", 0)],
            );
            return empty_collection_result(IDENTITY, &request, &run);
        }

        let unified_log = unified_log_path(&self.grok_home);
        if !unified_log.is_file() {
            self.record_failure(
                &request,
                CollectorFailureCode::SourceNotFound,
                &[CollectorDiagnosticCounter::new("rowsFound", 0)],
            );
            return empty_collection_result(IDENTITY, &request, &run);
        }

        if cancellation.is_cancelled() {
            return Err(request_failure(&request, CollectorFailureCode::Cancelled));
        }

        let summaries = GrokSessionIndex::from_grok_home(&self.grok_home)
            .scan()
            .map_err(|_| {
                self.record_failure(
                    &request,
                    CollectorFailureCode::IncompatibleEnvelope,
                    &[CollectorDiagnosticCounter::new("rowsFound", 0)],
                );
                request_failure(&request, CollectorFailureCode::IncompatibleEnvelope)
            })?;
        let resolver =
            GrokModelResolver::from_grok_home(&self.grok_home, &summaries).map_err(|_| {
                self.record_failure(
                    &request,
                    CollectorFailureCode::IncompatibleEnvelope,
                    &[CollectorDiagnosticCounter::new("rowsFound", 0)],
                );
                request_failure(&request, CollectorFailureCode::IncompatibleEnvelope)
            })?;
        let aggregation_timezone = request.aggregation_timezone().unwrap_or("UTC");
        let (mapped, ingest) = self
            .usage_cache
            .ingest(
                &unified_log,
                request.scope(),
                aggregation_timezone,
                &summaries,
                &resolver,
                COLLECTOR_VERSION,
            )
            .map_err(|_| {
                self.record_failure(
                    &request,
                    CollectorFailureCode::IncompatibleEnvelope,
                    &[CollectorDiagnosticCounter::new("rowsFound", 0)],
                );
                request_failure(&request, CollectorFailureCode::IncompatibleEnvelope)
            })?;
        let rows_found = u64::try_from(mapped.len()).unwrap_or(u64::MAX);
        if ingest.used_cache_fallback {
            self.record_cache_fallback(&request, &ingest);
        }

        let finished_at = Utc::now();
        let metadata = collection_metadata(IDENTITY, &request, run.started_at(), finished_at)?;
        let context = GrokMappingContext::new(
            collector_key(IDENTITY)?,
            COLLECTOR_VERSION.to_owned(),
            request.collection_id().clone(),
            finished_at,
        )
        .map_err(|_| request_failure(&request, CollectorFailureCode::Internal))?;
        let process_summary = run.process_summary();

        match request.projection() {
            CollectionProjection::Daily => {
                let timezone = request.aggregation_timezone().ok_or_else(|| {
                    request_failure(&request, CollectorFailureCode::ScopeNotRepresentable)
                })?;
                let candidates = mapper::map_daily(mapped, timezone, request.scope(), &context)
                    .map_err(|_| {
                        self.record_failure(
                            &request,
                            CollectorFailureCode::IncompatibleEnvelope,
                            &[CollectorDiagnosticCounter::new("rowsFound", rows_found)],
                        );
                        request_failure(&request, CollectorFailureCode::IncompatibleEnvelope)
                    })?;
                CollectionResult::daily(
                    metadata,
                    candidates,
                    Vec::new(),
                    Vec::new(),
                    process_summary,
                )
                .map_err(|error| {
                    let failure = validation_failure_as_internal(&request, error);
                    self.record_failure(
                        &request,
                        failure.code,
                        &[CollectorDiagnosticCounter::new("rowsFound", rows_found)],
                    );
                    failure
                })
            }
            CollectionProjection::Session => {
                let candidates = mapper::map_sessions(mapped, &context).map_err(|_| {
                    self.record_failure(
                        &request,
                        CollectorFailureCode::IncompatibleEnvelope,
                        &[CollectorDiagnosticCounter::new("rowsFound", rows_found)],
                    );
                    request_failure(&request, CollectorFailureCode::IncompatibleEnvelope)
                })?;
                CollectionResult::session(
                    metadata,
                    candidates,
                    Vec::new(),
                    Vec::new(),
                    process_summary,
                )
                .map_err(|error| {
                    let failure = validation_failure_as_internal(&request, error);
                    self.record_failure(
                        &request,
                        failure.code,
                        &[CollectorDiagnosticCounter::new("rowsFound", rows_found)],
                    );
                    failure
                })
            }
        }
    }
}

impl GrokCollector {
    fn record_failure(
        &self,
        request: &CollectionRequest,
        code: CollectorFailureCode,
        counters: &[CollectorDiagnosticCounter],
    ) {
        record_collector_diagnostic(
            self.diagnostics.as_deref(),
            request,
            DiagnosticSeverity::Warning,
            "grok.collection_failed",
            "Grok Build collection failed.",
            Some(code),
            counters,
        );
    }

    fn record_cache_fallback(
        &self,
        request: &CollectionRequest,
        ingest: &super::usage_cache::GrokIngestReport,
    ) {
        record_collector_diagnostic(
            self.diagnostics.as_deref(),
            request,
            DiagnosticSeverity::Info,
            "grok.unified_log_unavailable_cache_used",
            "Grok Build collection used durable usage cache because the unified log was unavailable or truncated.",
            None,
            &[
                CollectorDiagnosticCounter::new("rowsFromLog", u64::from(ingest.rows_from_log)),
                CollectorDiagnosticCounter::new("rowsFromCache", u64::from(ingest.rows_from_cache)),
                CollectorDiagnosticCounter::new(
                    "truncationDetected",
                    u64::from(ingest.truncation_detected),
                ),
            ],
        );
    }
}

fn validate_request(request: &CollectionRequest) -> Result<(), CollectorFailure> {
    validate_source(request, SourceKey::GrokBuild)
}

fn descriptor() -> Result<CollectorDescriptor, CollectorFailure> {
    single_source_descriptor(
        IDENTITY,
        supported_projections(),
        CollectorIntegrity::UnverifiedDevelopment,
    )
}

fn supported_projections() -> Vec<CollectionProjection> {
    daily_session_projections()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Arc;

    use chrono::{DateTime, Utc};
    use tempfile::TempDir;

    use super::*;
    use crate::application::collection::{
        CollectionId, CollectionOutcome, CollectionScope, DetectionState,
    };
    use crate::application::diagnostics::DiagnosticSeverity;
    use crate::infrastructure::collectors::support::{
        daily_request as support_daily_request, date, detection_request, fixed_timestamp,
        session_request as support_session_request, NeverCancelled, RecordingDiagnostics,
    };

    #[test]
    fn describes_grok_build_profile() {
        let collector = GrokCollector::from_grok_home("/missing");

        let descriptor = collector.describe().expect("descriptor");

        assert_eq!(descriptor.display_name, "Grok Build");
        assert_eq!(descriptor.profiles[0].source, SourceKey::GrokBuild);
        assert_eq!(
            descriptor.profiles[0].supported_projections,
            vec![CollectionProjection::Daily, CollectionProjection::Session]
        );
    }

    #[test]
    fn detects_available_grok_home_with_inference_rows() {
        let fixture = FixtureGrok::new().with_single_session_usage();
        let collector = GrokCollector::from_grok_home(fixture.grok_home());

        let result = collector
            .detect(
                detection_request(SourceKey::GrokBuild, timestamp()),
                &NeverCancelled,
            )
            .expect("detection");

        assert_eq!(result.state, DetectionState::Available);
        assert!(result.usage_artifacts_found);
    }

    #[test]
    fn detects_available_no_data_for_empty_unified_log() {
        let fixture = FixtureGrok::new().with_empty_unified_log();
        let collector = GrokCollector::from_grok_home(fixture.grok_home());

        let result = collector
            .detect(
                detection_request(SourceKey::GrokBuild, timestamp()),
                &NeverCancelled,
            )
            .expect("detection");

        assert_eq!(result.state, DetectionState::AvailableNoData);
        assert!(!result.usage_artifacts_found);
    }

    #[test]
    fn detects_missing_grok_home() {
        let collector = GrokCollector::from_grok_home("/missing/grok-home");

        let result = collector
            .detect(
                detection_request(SourceKey::GrokBuild, timestamp()),
                &NeverCancelled,
            )
            .expect("detection");

        assert_eq!(result.state, DetectionState::NotFound);
    }

    #[test]
    fn records_diagnostic_when_grok_home_is_missing() {
        let diagnostics = Arc::new(RecordingDiagnostics::default());
        let collector = GrokCollector::from_grok_home("/missing/grok-home")
            .with_diagnostic_recorder(diagnostics.clone());

        let result = collector
            .collect(daily_request(CollectionScope::Full), &NeverCancelled)
            .expect("missing home is empty");

        assert_eq!(result.outcome(), CollectionOutcome::Empty);
        let events = diagnostics.events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].severity, DiagnosticSeverity::Warning);
        assert_eq!(events[0].code.as_str(), "grok.collection_failed");
        let context = events[0].context.as_ref().expect("context").as_str();
        assert!(context.contains(r#""source":"grok-build""#));
        assert!(context.contains(r#""projection":"daily""#));
        assert!(context.contains(r#""failureCode":"source.not_found""#));
        assert!(context.contains(r#""rowsFound":0"#));
        assert!(!context.contains("/missing"));
    }

    #[test]
    fn rejects_non_grok_collection_request() {
        let fixture = FixtureGrok::new().with_single_session_usage();
        let collector = GrokCollector::from_grok_home(fixture.grok_home());

        let error = collector
            .collect(
                CollectionRequest::session(
                    CollectionId::new("wrong").expect("collection"),
                    SourceKey::Cline,
                    CollectionScope::Full,
                    timestamp(),
                ),
                &NeverCancelled,
            )
            .expect_err("unsupported source");

        assert_eq!(error.code, CollectorFailureCode::UnsupportedSource);
    }

    #[test]
    fn collects_daily_usage_from_inference_rows() {
        let fixture = FixtureGrok::new().with_single_session_usage();
        let collector = GrokCollector::from_grok_home(fixture.grok_home());

        let result = collector
            .collect(daily_request(CollectionScope::Full), &NeverCancelled)
            .expect("daily collection");

        assert_eq!(result.outcome(), CollectionOutcome::Complete);
        assert_eq!(result.daily_candidates().len(), 1);
        let daily = &result.daily_candidates()[0];
        assert_eq!(
            daily.source_key,
            "grok-build:daily:v1:Asia/Jakarta:2026-07-06"
        );
        assert_eq!(daily.tokens.input_tokens(), Some(7000));
        assert_eq!(daily.tokens.cache_read_tokens(), Some(20000));
        assert_eq!(daily.tokens.output_tokens(), Some(420));
        assert_eq!(daily.tokens.total_tokens(), 27420);
        assert_eq!(daily.model_breakdowns.len(), 1);
        assert_eq!(
            daily.model_breakdowns[0].raw_model_id,
            "grok-composer-2.5-fast"
        );
    }

    #[test]
    fn applies_incremental_daily_scope() {
        let fixture = FixtureGrok::new().with_single_session_usage();
        let collector = GrokCollector::from_grok_home(fixture.grok_home());

        let result = collector
            .collect(
                daily_request(
                    CollectionScope::incremental(date(2026, 7, 7), date(2026, 7, 7))
                        .expect("scope"),
                ),
                &NeverCancelled,
            )
            .expect("daily collection");

        assert_eq!(result.outcome(), CollectionOutcome::Empty);
        assert!(result.daily_candidates().is_empty());
    }

    #[test]
    fn upserts_cache_after_successful_collection() {
        use crate::application::ports::grok_usage_cache::GrokUsageCache;
        use crate::infrastructure::collectors::grok::usage_cache::tests::RecordingGrokUsageCache;

        let fixture = FixtureGrok::new().with_single_session_usage();
        let cache = Arc::new(RecordingGrokUsageCache::default());
        let collector = GrokCollector::from_grok_home(fixture.grok_home())
            .with_usage_cache(GrokUsageCacheClient::new(cache.clone()));

        collector
            .collect(daily_request(CollectionScope::Full), &NeverCancelled)
            .expect("collection");

        assert_eq!(cache.upserts().len(), 2);
        assert!(cache.read_checkpoint().expect("checkpoint").is_some());
    }

    #[test]
    fn uses_cached_usage_when_unified_log_is_truncated() {
        use crate::application::ports::grok_usage_cache::GrokUnifiedLogCheckpoint;
        use crate::infrastructure::collectors::grok::usage_cache::tests::{
            cached_record, RecordingGrokUsageCache,
        };

        let diagnostics = Arc::new(RecordingDiagnostics::default());
        let fixture = FixtureGrok::new()
            .with_truncated_unified_log()
            .with_single_session_metadata();
        let cache = Arc::new(
            RecordingGrokUsageCache::default()
                .seed(vec![
                    cached_record("019f0000-0000-7000-8000-000000000001", 1, 12000, 240),
                    cached_record("019f0000-0000-7000-8000-000000000001", 2, 15000, 180),
                ])
                .with_checkpoint(GrokUnifiedLogCheckpoint {
                    file_inode: Some(99),
                    file_size: 10_000,
                    byte_offset: 10_000,
                }),
        );
        let collector = GrokCollector::from_grok_home(fixture.grok_home())
            .with_usage_cache(GrokUsageCacheClient::new(cache))
            .with_diagnostic_recorder(diagnostics.clone());

        let result = collector
            .collect(daily_request(CollectionScope::Full), &NeverCancelled)
            .expect("cached collection");

        assert_eq!(result.outcome(), CollectionOutcome::Complete);
        assert_eq!(result.daily_candidates()[0].tokens.total_tokens(), 27420);
        let events = diagnostics.events();
        assert!(events
            .iter()
            .any(|event| { event.code.as_str() == "grok.unified_log_unavailable_cache_used" }));
        assert_eq!(events[0].severity, DiagnosticSeverity::Info);
    }

    #[test]
    fn collects_session_usage_grouped_by_session_and_model() {
        let fixture = FixtureGrok::new().with_single_session_usage();
        let collector = GrokCollector::from_grok_home(fixture.grok_home());

        let result = collector
            .collect(session_request(), &NeverCancelled)
            .expect("session collection");

        assert_eq!(result.outcome(), CollectionOutcome::Complete);
        assert_eq!(result.session_candidates().len(), 1);
        let session = &result.session_candidates()[0];
        assert_eq!(
            session.source_key,
            "grok-build:session:v1:019f0000-0000-7000-8000-000000000001:grok-composer-2.5-fast"
        );
        assert_eq!(
            session.source_session_id,
            "019f0000-0000-7000-8000-000000000001"
        );
        assert_eq!(
            session.project_path.as_deref(),
            Some("/tmp/grok-fixture-project")
        );
        assert_eq!(
            session.first_activity_at,
            Some(fixed_timestamp(2026, 7, 6, 10, 0, 0))
        );
        assert_eq!(
            session.last_activity_at,
            Some(fixed_timestamp(2026, 7, 6, 10, 0, 8))
        );
    }

    struct FixtureGrok {
        workspace: TempDir,
    }

    impl FixtureGrok {
        fn new() -> Self {
            Self {
                workspace: TempDir::new().expect("workspace"),
            }
        }

        fn grok_home(&self) -> PathBuf {
            self.workspace.path().to_path_buf()
        }

        fn with_single_session_usage(self) -> Self {
            self.seed_unified_log("unified-log/single-session.jsonl");
            self.seed_session(
                "019f0000-0000-7000-8000-000000000001",
                "sessions/summary-valid.json",
                "events/turn-started.jsonl",
            );
            self
        }

        fn with_truncated_unified_log(self) -> Self {
            self.seed_unified_log("unified-log/truncated-log.jsonl");
            self
        }

        fn with_single_session_metadata(self) -> Self {
            self.seed_session(
                "019f0000-0000-7000-8000-000000000001",
                "sessions/summary-valid.json",
                "events/turn-started.jsonl",
            );
            self
        }

        fn with_empty_unified_log(self) -> Self {
            fs::create_dir_all(self.grok_home().join("logs")).expect("logs dir");
            fs::write(unified_log_path(&self.grok_home()), "").expect("empty log");
            self
        }

        fn seed_unified_log(&self, relative: &str) {
            fs::create_dir_all(self.grok_home().join("logs")).expect("logs dir");
            fs::copy(fixture_path(relative), unified_log_path(&self.grok_home()))
                .expect("copy unified log");
        }

        fn seed_session(&self, session_id: &str, summary_fixture: &str, events_fixture: &str) {
            let session_dir = self
                .grok_home()
                .join("sessions")
                .join("%2Ftmp%2Fgrok-fixture-project")
                .join(session_id);
            fs::create_dir_all(&session_dir).expect("session dir");
            fs::copy(
                fixture_path(summary_fixture),
                session_dir.join("summary.json"),
            )
            .expect("copy summary");
            fs::copy(
                fixture_path(events_fixture),
                session_dir.join("events.jsonl"),
            )
            .expect("copy events");
        }
    }

    fn fixture_path(relative: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/collectors/grok")
            .join(relative)
    }

    fn daily_request(scope: CollectionScope) -> CollectionRequest {
        support_daily_request(
            "grok-daily",
            SourceKey::GrokBuild,
            scope,
            "Asia/Jakarta",
            timestamp(),
        )
    }

    fn session_request() -> CollectionRequest {
        support_session_request("grok-session", SourceKey::GrokBuild, timestamp())
    }

    fn timestamp() -> DateTime<Utc> {
        fixed_timestamp(2026, 7, 6, 12, 0, 0)
    }
}
