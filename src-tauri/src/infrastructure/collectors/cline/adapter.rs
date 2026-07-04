use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use chrono::Utc;

use crate::application::collection::{
    CollectionMetadata, CollectionPeriod, CollectionProjection, CollectionRequest,
    CollectionResult, CollectorDescriptor, CollectorFailure, CollectorFailureCode,
    CollectorIntegrity, DetectionRequest, DetectionResult, ProcessSummary, RejectedRecord,
};
use crate::application::ports::collector::{CancellationSignal, Collector};
use crate::domain::source::SourceKey;

use super::super::support::{
    available_detection, cancelled_detection, collector_key, daily_session_projections,
    detection_issue, invalid_configuration_detection, missing_or_invalid_location_code,
    not_found_detection, request_failure, single_source_descriptor, unsupported_detection,
    validate_source, validation_failure_as_internal, validation_failure_preserving_all_rejected,
    CollectorIdentity,
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

#[derive(Debug, Clone)]
pub(crate) struct ClineCollector {
    database_path: PathBuf,
}

impl ClineCollector {
    pub(crate) fn from_database_path(path: impl Into<PathBuf>) -> Self {
        Self {
            database_path: path.into(),
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
        let started = Instant::now();
        let started_at = Utc::now();
        validate_request(&request)?;
        if cancellation.is_cancelled() {
            return Err(request_failure(&request, CollectorFailureCode::Cancelled));
        }
        if !self.database_path.exists() {
            return empty_result(&request, started, started_at);
        }

        let store = ClineStore::open_read_only(&self.database_path).map_err(|_| {
            request_failure(
                &request,
                missing_or_invalid_location_code(&self.database_path),
            )
        })?;
        let sessions = store
            .read_sessions()
            .map_err(|_| request_failure(&request, CollectorFailureCode::IncompatibleEnvelope))?;
        let (sessions, rejections) = load_session_messages(sessions);
        if cancellation.is_cancelled() {
            return Err(request_failure(&request, CollectorFailureCode::Cancelled));
        }

        let finished_at = Utc::now();
        let metadata = CollectionMetadata::new(
            request.collection_id().clone(),
            collector_key(IDENTITY)?,
            COLLECTOR_VERSION.to_owned(),
            SourceKey::Cline,
            request.scope().clone(),
            PROFILE_VERSION,
            CollectionPeriod {
                started_at,
                finished_at,
            },
        )
        .map_err(|error| validation_failure_as_internal(&request, error))?;
        let context = ClineMappingContext::new(
            collector_key(IDENTITY)?,
            COLLECTOR_VERSION.to_owned(),
            request.collection_id().clone(),
            finished_at,
        )
        .map_err(|_| request_failure(&request, CollectorFailureCode::Internal))?;
        let process_summary = ProcessSummary {
            runtime_ms: started.elapsed().as_millis().try_into().unwrap_or(u64::MAX),
            stdout_bytes: 0,
            stderr_bytes: 0,
            exit_code: None,
        };

        match request.projection() {
            CollectionProjection::Daily => {
                let timezone = request.aggregation_timezone().ok_or_else(|| {
                    request_failure(&request, CollectorFailureCode::ScopeNotRepresentable)
                })?;
                let candidates = mapper::map_daily(sessions, timezone, request.scope(), &context)
                    .map_err(|_| {
                    request_failure(&request, CollectorFailureCode::IncompatibleEnvelope)
                })?;
                CollectionResult::daily(
                    metadata,
                    candidates,
                    rejections,
                    Vec::new(),
                    process_summary,
                )
                .map_err(|error| validation_failure_preserving_all_rejected(&request, error))
            }
            CollectionProjection::Session => {
                let candidates = mapper::map_sessions(sessions, &context).map_err(|_| {
                    request_failure(&request, CollectorFailureCode::IncompatibleEnvelope)
                })?;
                CollectionResult::session(
                    metadata,
                    candidates,
                    rejections,
                    Vec::new(),
                    process_summary,
                )
                .map_err(|error| validation_failure_preserving_all_rejected(&request, error))
            }
        }
    }
}

fn empty_result(
    request: &CollectionRequest,
    started: Instant,
    started_at: chrono::DateTime<Utc>,
) -> Result<CollectionResult, CollectorFailure> {
    let finished_at = Utc::now();
    let metadata = CollectionMetadata::new(
        request.collection_id().clone(),
        collector_key(IDENTITY)?,
        COLLECTOR_VERSION.to_owned(),
        SourceKey::Cline,
        request.scope().clone(),
        PROFILE_VERSION,
        CollectionPeriod {
            started_at,
            finished_at,
        },
    )
    .map_err(|error| validation_failure_as_internal(request, error))?;
    let process_summary = ProcessSummary {
        runtime_ms: started.elapsed().as_millis().try_into().unwrap_or(u64::MAX),
        stdout_bytes: 0,
        stderr_bytes: 0,
        exit_code: None,
    };

    match request.projection() {
        CollectionProjection::Daily => CollectionResult::daily(
            metadata,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            process_summary,
        ),
        CollectionProjection::Session => CollectionResult::session(
            metadata,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            process_summary,
        ),
    }
    .map_err(|error| validation_failure_as_internal(request, error))
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
    use std::path::Path;

    use chrono::{TimeZone, Utc};
    use rusqlite::Connection;
    use tempfile::TempDir;

    use super::*;
    use crate::application::collection::{
        CollectionId, CollectionOutcome, CollectionScope, DetectionState,
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
                DetectionRequest {
                    source: SourceKey::Cline,
                    reason: crate::application::collection::DetectionReason::Startup,
                    requested_at: timestamp(),
                },
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
        assert_eq!(candidate.first_activity_at, Some(millis(1_782_782_160_000)));
        assert_eq!(candidate.last_activity_at, Some(millis(1_782_782_700_000)));
    }

    #[test]
    fn rejects_unreadable_message_files_without_exposing_content() {
        let fixture = FixtureCline::new();
        fixture.seed_session(METADATA, "missing.messages.json");
        let collector = ClineCollector::from_database_path(fixture.database_path());

        let error = collector
            .collect(daily_request(CollectionScope::Full), &NeverCancelled)
            .expect_err("all records rejected");

        assert_eq!(error.code, CollectorFailureCode::AllRecordsRejected);
    }

    struct NeverCancelled;

    impl CancellationSignal for NeverCancelled {
        fn is_cancelled(&self) -> bool {
            false
        }
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
        CollectionRequest::daily(
            CollectionId::new("cline-daily").expect("collection id"),
            SourceKey::Cline,
            scope,
            "Asia/Jakarta",
            timestamp(),
        )
        .expect("daily request")
    }

    fn session_request() -> CollectionRequest {
        CollectionRequest::session(
            CollectionId::new("cline-session").expect("collection id"),
            SourceKey::Cline,
            CollectionScope::Full,
            timestamp(),
        )
    }

    fn timestamp() -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 6, 30, 12, 0, 0)
            .single()
            .expect("timestamp")
    }

    fn millis(value: i64) -> chrono::DateTime<Utc> {
        Utc.timestamp_millis_opt(value).single().expect("timestamp")
    }

    fn message_fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/collectors/cline/messages")
            .join(name)
    }
}
