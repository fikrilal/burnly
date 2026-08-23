use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::Utc;

use crate::application::collection::{
    CollectionProjection, CollectionRequest, CollectionResult, CollectorDescriptor,
    CollectorFailure, CollectorFailureCode, CollectorIntegrity, DetectionRequest, DetectionResult,
    RejectedRecord,
};
use crate::application::diagnostics::DiagnosticSeverity;
use crate::application::ports::collector::{CancellationSignal, Collector};
use crate::application::ports::diagnostic_recorder::DiagnosticRecorder;
use crate::domain::source::SourceKey;

use super::super::support::{
    available_detection, cancelled_detection, collection_metadata, collector_key,
    daily_session_projections, detection_issue, empty_collection_result,
    invalid_configuration_detection, missing_or_invalid_location_code, not_found_detection,
    path_is_missing, record_collector_diagnostic, request_failure, single_source_descriptor,
    unsupported_detection, validate_source, validation_failure_preserving_all_rejected,
    CollectorDiagnosticCounter, CollectorIdentity, LocalCollectionRun,
};
use super::mapper::{self, ClineMappingContext, ClineSessionMessages};
use super::{decode_messages, ClineStore};

const COLLECTOR_KEY: &str = "cline";
const DISPLAY_NAME: &str = "Cline";
const COLLECTOR_VERSION: &str = "local";
const ADAPTER_VERSION: u16 = 1;
const PROFILE_VERSION: u16 = 1;
const IDENTITY: CollectorIdentity = CollectorIdentity {
    key: COLLECTOR_KEY,
    display_name: DISPLAY_NAME,
    runtime_version: COLLECTOR_VERSION,
    adapter_version: ADAPTER_VERSION,
    source: SourceKey::Cline,
    profile_version: PROFILE_VERSION,
};

#[derive(Clone)]
pub(crate) struct ClineCollector {
    database_path: PathBuf,
    diagnostics: Option<Arc<dyn DiagnosticRecorder>>,
}

impl ClineCollector {
    pub(crate) fn from_database_path(path: impl Into<PathBuf>) -> Self {
        Self {
            database_path: path.into(),
            diagnostics: None,
        }
    }

    #[allow(dead_code, reason = "default data root is wired in the runtime chunk")]
    pub(crate) fn from_data_dir(data_dir: impl AsRef<Path>) -> Self {
        Self::from_database_path(
            data_dir
                .as_ref()
                .join("data")
                .join("db")
                .join("sessions.db"),
        )
    }

    pub(crate) fn with_diagnostic_recorder(
        mut self,
        diagnostics: Arc<dyn DiagnosticRecorder>,
    ) -> Self {
        self.diagnostics = Some(diagnostics);
        self
    }
}

impl Collector for ClineCollector {
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
        if request.source != SourceKey::Cline {
            return Ok(unsupported_detection(
                &request,
                detection_issue("cline.unsupported_source", "Source is not Cline."),
            ));
        }
        if !self.database_path.exists() {
            return Ok(not_found_detection(
                &request,
                SourceKey::Cline,
                supported_projections(),
                detection_issue(
                    "cline.database_missing",
                    "Cline sessions database was not found.",
                ),
            ));
        }

        match ClineStore::open_read_only(&self.database_path)
            .and_then(|store| store.read_sessions())
        {
            Ok(sessions) => Ok(available_detection(
                &request,
                SourceKey::Cline,
                supported_projections(),
                !sessions.is_empty(),
            )),
            Err(_) => Ok(invalid_configuration_detection(
                &request,
                SourceKey::Cline,
                supported_projections(),
                detection_issue(
                    "cline.database_incompatible",
                    "Cline sessions database is not readable by Burnly.",
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
        if path_is_missing(&self.database_path) {
            return empty_collection_result(IDENTITY, &request, &run);
        }

        let store = ClineStore::open_read_only(&self.database_path).map_err(|_| {
            let code = missing_or_invalid_location_code(&self.database_path);
            self.record_failure(
                &request,
                code,
                &[CollectorDiagnosticCounter::new("sessionsFound", 0)],
            );
            request_failure(&request, code)
        })?;
        let sessions = store.read_sessions().map_err(|_| {
            self.record_failure(
                &request,
                CollectorFailureCode::IncompatibleEnvelope,
                &[CollectorDiagnosticCounter::new("sessionsFound", 0)],
            );
            request_failure(&request, CollectorFailureCode::IncompatibleEnvelope)
        })?;
        let (sessions, rejections) = load_session_messages(sessions);
        let sessions_found = u64::try_from(sessions.len()).unwrap_or(u64::MAX);
        let records_rejected = u64::try_from(rejections.len()).unwrap_or(u64::MAX);
        if records_rejected > 0 && sessions_found > 0 {
            self.record_failure(
                &request,
                CollectorFailureCode::IncompatibleEnvelope,
                &[
                    CollectorDiagnosticCounter::new("sessionsFound", sessions_found),
                    CollectorDiagnosticCounter::new("recordsRejected", records_rejected),
                ],
            );
        }
        if cancellation.is_cancelled() {
            return Err(request_failure(&request, CollectorFailureCode::Cancelled));
        }

        let finished_at = Utc::now();
        let metadata = collection_metadata(IDENTITY, &request, run.started_at(), finished_at)?;
        let context = ClineMappingContext::new(
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
                let candidates = mapper::map_daily(sessions, timezone, request.scope(), &context)
                    .map_err(|_| {
                    self.record_failure(
                        &request,
                        CollectorFailureCode::IncompatibleEnvelope,
                        &[
                            CollectorDiagnosticCounter::new("sessionsFound", sessions_found),
                            CollectorDiagnosticCounter::new("recordsRejected", records_rejected),
                        ],
                    );
                    request_failure(&request, CollectorFailureCode::IncompatibleEnvelope)
                })?;
                CollectionResult::daily(
                    metadata,
                    candidates,
                    rejections,
                    Vec::new(),
                    process_summary,
                )
                .map_err(|error| {
                    let failure = validation_failure_preserving_all_rejected(&request, error);
                    self.record_failure(
                        &request,
                        failure.code,
                        &[
                            CollectorDiagnosticCounter::new("sessionsFound", sessions_found),
                            CollectorDiagnosticCounter::new("recordsRejected", records_rejected),
                        ],
                    );
                    failure
                })
            }
            CollectionProjection::Session => {
                let candidates = mapper::map_sessions(sessions, &context).map_err(|_| {
                    self.record_failure(
                        &request,
                        CollectorFailureCode::IncompatibleEnvelope,
                        &[
                            CollectorDiagnosticCounter::new("sessionsFound", sessions_found),
                            CollectorDiagnosticCounter::new("recordsRejected", records_rejected),
                        ],
                    );
                    request_failure(&request, CollectorFailureCode::IncompatibleEnvelope)
                })?;
                CollectionResult::session(
                    metadata,
                    candidates,
                    rejections,
                    Vec::new(),
                    process_summary,
                )
                .map_err(|error| {
                    let failure = validation_failure_preserving_all_rejected(&request, error);
                    self.record_failure(
                        &request,
                        failure.code,
                        &[
                            CollectorDiagnosticCounter::new("sessionsFound", sessions_found),
                            CollectorDiagnosticCounter::new("recordsRejected", records_rejected),
                        ],
                    );
                    failure
                })
            }
        }
    }
}

impl ClineCollector {
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
            "cline.collection_failed",
            "Cline collection failed.",
            Some(code),
            counters,
        );
    }
}

fn load_session_messages(
    sessions: Vec<super::ClineSessionRow>,
) -> (Vec<ClineSessionMessages>, Vec<RejectedRecord>) {
    let mut accepted = Vec::new();
    let mut rejected = Vec::new();
    for (index, session) in sessions.into_iter().enumerate() {
        match fs::read_to_string(&session.messages_path)
            .map_err(|_| ())
            .and_then(|content| decode_messages(&content).map_err(|_| ()))
        {
            Ok(messages) => accepted.push(ClineSessionMessages { session, messages }),
            Err(()) => rejected.push(RejectedRecord {
                code: "cline.messages_unreadable".to_owned(),
                record_index: Some(u32::try_from(index).unwrap_or(u32::MAX)),
            }),
        }
    }
    (accepted, rejected)
}

fn validate_request(request: &CollectionRequest) -> Result<(), CollectorFailure> {
    validate_source(request, SourceKey::Cline)
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
    use std::path::Path;
    use std::sync::Arc;

    use rusqlite::Connection;
    use tempfile::TempDir;

    use super::*;
    use crate::application::collection::{CollectionOutcome, CollectionScope, DetectionState};
    use crate::application::diagnostics::DiagnosticSeverity;
    use crate::infrastructure::collectors::support::{
        daily_request as support_daily_request, detection_request, fixed_timestamp,
        session_request as support_session_request, utc_millis, NeverCancelled,
        RecordingDiagnostics,
    };

    const METADATA: &str = r#"{
      "usage": {
        "inputTokens": 12000,
        "outputTokens": 800,
        "cacheReadTokens": 3000,
        "cacheWriteTokens": 0,
        "totalCost": 0.0115
      },
      "aggregateUsage": {
        "inputTokens": 12000,
        "outputTokens": 800,
        "cacheReadTokens": 3000,
        "cacheWriteTokens": 0,
        "totalCost": 0.0115
      }
    }"#;

    #[test]
    fn detects_available_cline_database() {
        let fixture = FixtureCline::new();
        fixture.seed_session(METADATA, "valid-session.messages.json");
        let collector = ClineCollector::from_database_path(fixture.database_path());

        let result = collector
            .detect(
                detection_request(SourceKey::Cline, timestamp()),
                &NeverCancelled,
            )
            .expect("detection");

        assert_eq!(result.state, DetectionState::Available);
        assert_eq!(
            result.supported_projections,
            vec![CollectionProjection::Daily, CollectionProjection::Session]
        );
        assert!(result.usage_artifacts_found);
    }

    #[test]
    fn collects_daily_usage_from_message_timestamps() {
        let fixture = FixtureCline::new();
        fixture.seed_session(METADATA, "valid-session.messages.json");
        let collector = ClineCollector::from_database_path(fixture.database_path());

        let result = collector
            .collect(daily_request(CollectionScope::Full), &NeverCancelled)
            .expect("daily collection");

        assert_eq!(result.outcome(), CollectionOutcome::Complete);
        assert_eq!(result.daily_candidates().len(), 1);
        let candidate = &result.daily_candidates()[0];
        assert_eq!(
            candidate.source_key,
            "cline:daily:v1:Asia/Jakarta:2026-06-30"
        );
        assert_eq!(candidate.tokens.input_tokens(), Some(12_000));
        assert_eq!(candidate.tokens.output_tokens(), Some(800));
        assert_eq!(candidate.tokens.cache_read_tokens(), Some(3_000));
        assert_eq!(candidate.tokens.cache_creation_tokens(), Some(0));
        assert_eq!(candidate.tokens.total_tokens(), 15_800);
        assert_eq!(candidate.model_breakdowns.len(), 1);
        assert_eq!(
            candidate.model_breakdowns[0].raw_model_id,
            "cline-pass/glm-5.2"
        );
    }

    #[test]
    fn collects_session_usage_from_metadata_with_message_activity_window() {
        let fixture = FixtureCline::new();
        fixture.seed_session(METADATA, "valid-session.messages.json");
        let collector = ClineCollector::from_database_path(fixture.database_path());

        let result = collector
            .collect(session_request(), &NeverCancelled)
            .expect("session collection");

        assert_eq!(result.outcome(), CollectionOutcome::Complete);
        assert_eq!(result.session_candidates().len(), 1);
        let candidate = &result.session_candidates()[0];
        assert_eq!(candidate.source_key, "cline:session:v1:cline-session-1");
        assert_eq!(candidate.source_session_id, "cline-session-1");
        assert_eq!(candidate.project_path, None);
        assert_eq!(candidate.tokens.input_tokens(), Some(12_000));
        assert_eq!(
            candidate.first_activity_at,
            Some(utc_millis(1_782_782_160_000))
        );
        assert_eq!(
            candidate.last_activity_at,
            Some(utc_millis(1_782_782_700_000))
        );
    }

    #[test]
    fn rejects_unreadable_message_files_without_exposing_content() {
        let fixture = FixtureCline::new();
        fixture.seed_session(METADATA, "missing.messages.json");
        let diagnostics = Arc::new(RecordingDiagnostics::default());
        let collector = ClineCollector::from_database_path(fixture.database_path())
            .with_diagnostic_recorder(diagnostics.clone());

        let error = collector
            .collect(daily_request(CollectionScope::Full), &NeverCancelled)
            .expect_err("all records rejected");

        assert_eq!(error.code, CollectorFailureCode::AllRecordsRejected);
        let events = diagnostics.events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].severity, DiagnosticSeverity::Warning);
        assert_eq!(events[0].code.as_str(), "cline.collection_failed");
        let context = events[0].context.as_ref().expect("context").as_str();
        assert!(context.contains(r#""source":"cline""#));
        assert!(context.contains(r#""projection":"daily""#));
        assert!(context.contains(r#""failureCode":"collection.all_records_rejected""#));
        assert!(context.contains(r#""recordsRejected":1"#));
        assert!(!context.contains("missing.messages.json"));
    }

    #[test]
    fn missing_database_collection_is_empty_without_diagnostic() {
        let diagnostics = Arc::new(RecordingDiagnostics::default());
        let collector = ClineCollector::from_database_path("/missing/sessions.db")
            .with_diagnostic_recorder(diagnostics.clone());

        let result = collector
            .collect(daily_request(CollectionScope::Full), &NeverCancelled)
            .expect("missing database is empty");

        assert_eq!(result.outcome(), CollectionOutcome::Empty);
        assert!(diagnostics.events().is_empty());
    }

    #[test]
    fn invalid_database_location_records_diagnostic() {
        let temp = TempDir::new().expect("workspace");
        let database_path = temp.path().join("sessions.db");
        fs::create_dir(&database_path).expect("database directory");
        let diagnostics = Arc::new(RecordingDiagnostics::default());
        let collector = ClineCollector::from_database_path(database_path)
            .with_diagnostic_recorder(diagnostics.clone());

        let error = collector
            .collect(daily_request(CollectionScope::Full), &NeverCancelled)
            .expect_err("invalid database location fails");

        assert_eq!(error.code, CollectorFailureCode::SourceInvalidLocation);
        let events = diagnostics.events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].severity, DiagnosticSeverity::Warning);
        assert_eq!(events[0].code.as_str(), "cline.collection_failed");
        let context = events[0].context.as_ref().expect("context").as_str();
        assert!(context.contains(r#""failureCode":"source.invalid_location""#));
    }

    struct FixtureCline {
        workspace: TempDir,
    }

    impl FixtureCline {
        fn new() -> Self {
            let workspace = TempDir::new().expect("workspace");
            let database_path = workspace.path().join("sessions.db");
            create_schema(&database_path);
            Self { workspace }
        }

        fn database_path(&self) -> PathBuf {
            self.workspace.path().join("sessions.db")
        }

        fn seed_session(&self, metadata_json: &str, messages_name: &str) {
            let messages_path = self.workspace.path().join(messages_name);
            if messages_name != "missing.messages.json" {
                fs::copy(
                    message_fixture("valid-session.messages.json"),
                    &messages_path,
                )
                .expect("copy message fixture");
            }
            let connection = Connection::open(self.database_path()).expect("database");
            connection
                .execute(
                    "INSERT INTO sessions (
                        session_id,
                        started_at,
                        ended_at,
                        status,
                        provider,
                        model,
                        cwd,
                        workspace_root,
                        metadata_json,
                        messages_path,
                        updated_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                    (
                        "cline-session-1",
                        "2026-06-30T01:15:00.000Z",
                        Some("2026-06-30T01:25:00.000Z"),
                        "idle",
                        "cline-pass",
                        "cline-pass/glm-5.2",
                        "/fixture/workspace",
                        "/fixture/workspace",
                        metadata_json,
                        messages_path.to_string_lossy().as_ref(),
                        "2026-06-30T01:25:00.000Z",
                    ),
                )
                .expect("seed session");
        }
    }

    fn create_schema(path: &Path) {
        let connection = Connection::open(path).expect("database");
        connection
            .execute(
                "CREATE TABLE sessions (
                    session_id TEXT PRIMARY KEY,
                    started_at TEXT NOT NULL,
                    ended_at TEXT,
                    status TEXT NOT NULL,
                    provider TEXT NOT NULL,
                    model TEXT NOT NULL,
                    cwd TEXT NOT NULL,
                    workspace_root TEXT NOT NULL,
                    metadata_json TEXT,
                    messages_path TEXT,
                    updated_at TEXT NOT NULL
                )",
                [],
            )
            .expect("schema");
    }

    fn daily_request(scope: CollectionScope) -> CollectionRequest {
        support_daily_request(
            "cline-daily",
            SourceKey::Cline,
            scope,
            "Asia/Jakarta",
            timestamp(),
        )
    }

    fn session_request() -> CollectionRequest {
        support_session_request("cline-session", SourceKey::Cline, timestamp())
    }

    fn timestamp() -> chrono::DateTime<Utc> {
        fixed_timestamp(2026, 6, 30, 12, 0, 0)
    }

    fn message_fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/collectors/cline/messages")
            .join(name)
    }
}
