//! Zed agent collector adapter.
//!
//! Wires the thread store, mapper, and cost calculator into the collector
//! port. Collection reads `~/.local/share/zed/threads/threads.db` read-only
//! and maps thread cumulative token usage into Burnly daily/session
//! candidates. Cost is Burnly-calculated from the embedded models.dev
//! snapshot with the `zed.dev/` provider prefix normalized away.

use std::path::PathBuf;
use std::sync::Arc;

use chrono::Utc;

use crate::application::collection::{
    CollectionProjection, CollectionRequest, CollectionResult, CollectorDescriptor,
    CollectorFailure, CollectorFailureCode, CollectorIntegrity, DetectionRequest, DetectionResult,
};
use crate::application::cost::BurnlyCostCalculator;
use crate::application::diagnostics::DiagnosticSeverity;
use crate::application::ports::collector::{CancellationSignal, Collector};
use crate::application::ports::diagnostic_recorder::DiagnosticRecorder;
use crate::domain::source::SourceKey;
use crate::infrastructure::collectors::support::{
    available_detection, cancelled_detection, collection_metadata, daily_session_projections,
    detection_issue, empty_collection_result, not_found_detection, record_collector_diagnostic,
    request_failure, single_source_descriptor, unsupported_detection, validate_source,
    validation_failure_as_internal, CollectorDiagnosticCounter, CollectorIdentity,
    LocalCollectionRun,
};

use super::detection::threads_db_path;
use super::mapper::{self, ZedMappingContext};
use super::threads_store::ZedThreadStore;

const COLLECTOR_KEY: &str = "zed";
const DISPLAY_NAME: &str = "Zed";
const COLLECTOR_VERSION: &str = "local";
const ADAPTER_VERSION: u16 = 1;
const PROFILE_VERSION: u16 = 1;
const IDENTITY: CollectorIdentity = CollectorIdentity {
    key: COLLECTOR_KEY,
    display_name: DISPLAY_NAME,
    runtime_version: COLLECTOR_VERSION,
    adapter_version: ADAPTER_VERSION,
    source: SourceKey::Zed,
    profile_version: PROFILE_VERSION,
};

pub(crate) const ISSUE_THREADS_DB_MISSING: &str = "zed.threads_db_missing";
pub(crate) const ISSUE_THREADS_DB_UNREADABLE: &str = "zed.threads_db_unreadable";

#[derive(Clone)]
pub(crate) struct ZedCollector {
    zed_data_dir: PathBuf,
    diagnostics: Option<Arc<dyn DiagnosticRecorder>>,
    calculator: BurnlyCostCalculator,
}

impl ZedCollector {
    pub(crate) fn from_data_dir(zed_data_dir: PathBuf) -> Self {
        Self {
            zed_data_dir,
            diagnostics: None,
            calculator: BurnlyCostCalculator::new(),
        }
    }

    pub(crate) fn with_diagnostic_recorder(
        mut self,
        diagnostics: Arc<dyn DiagnosticRecorder>,
    ) -> Self {
        self.diagnostics = Some(diagnostics);
        self
    }
}

impl Collector for ZedCollector {
    fn describe(&self) -> Result<CollectorDescriptor, CollectorFailure> {
        single_source_descriptor(
            IDENTITY,
            supported_projections(),
            CollectorIntegrity::UnverifiedDevelopment,
        )
    }

    fn detect(
        &self,
        request: DetectionRequest,
        cancellation: &dyn CancellationSignal,
    ) -> Result<DetectionResult, CollectorFailure> {
        if request.source != SourceKey::Zed {
            return Ok(unsupported_detection(
                &request,
                detection_issue("zed.unsupported_source", "Source is not Zed."),
            ));
        }
        if cancellation.is_cancelled() {
            return Ok(cancelled_detection(&request));
        }

        let db_path = threads_db_path(&self.zed_data_dir);
        if !db_path.is_file() {
            return Ok(not_found_detection(
                &request,
                SourceKey::Zed,
                supported_projections(),
                detection_issue(
                    ISSUE_THREADS_DB_MISSING,
                    "Zed threads database was not found.",
                ),
            ));
        }

        match ZedThreadStore::open_read_only(&db_path) {
            Ok(store) => match store.read_threads() {
                Ok(threads) => Ok(available_detection(
                    &request,
                    SourceKey::Zed,
                    supported_projections(),
                    !threads.is_empty(),
                )),
                Err(_) => Ok(not_found_detection(
                    &request,
                    SourceKey::Zed,
                    supported_projections(),
                    detection_issue(
                        ISSUE_THREADS_DB_UNREADABLE,
                        "Zed threads database is not readable by Burnly.",
                    ),
                )),
            },
            Err(_) => Ok(not_found_detection(
                &request,
                SourceKey::Zed,
                supported_projections(),
                detection_issue(
                    ISSUE_THREADS_DB_UNREADABLE,
                    "Zed threads database is not readable by Burnly.",
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
        validate_source(&request, SourceKey::Zed)?;
        if cancellation.is_cancelled() {
            return Err(request_failure(&request, CollectorFailureCode::Cancelled));
        }

        let db_path = threads_db_path(&self.zed_data_dir);
        if !db_path.is_file() {
            self.record_failure(
                &request,
                CollectorFailureCode::SourceNotFound,
                &[CollectorDiagnosticCounter::new("rowsFound", 0)],
            );
            return empty_collection_result(IDENTITY, &request, &run);
        }
        let store = match ZedThreadStore::open_read_only(&db_path) {
            Ok(store) => store,
            Err(_) => {
                self.record_failure(
                    &request,
                    CollectorFailureCode::SourceNotFound,
                    &[CollectorDiagnosticCounter::new("rowsFound", 0)],
                );
                return empty_collection_result(IDENTITY, &request, &run);
            }
        };
        let threads = match store.read_threads() {
            Ok(threads) => threads,
            Err(_) => {
                self.record_failure(
                    &request,
                    CollectorFailureCode::IncompatibleEnvelope,
                    &[CollectorDiagnosticCounter::new("rowsFound", 0)],
                );
                return empty_collection_result(IDENTITY, &request, &run);
            }
        };
        let rows_found = u64::try_from(threads.len()).unwrap_or(u64::MAX);

        let finished_at = Utc::now();
        let metadata = collection_metadata(IDENTITY, &request, run.started_at(), finished_at)?;
        let context = ZedMappingContext::new(
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
                let candidates = mapper::map_threads(
                    threads,
                    timezone,
                    request.scope(),
                    &context,
                    &self.calculator,
                )
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
                let candidates = mapper::map_sessions(threads, &context, &self.calculator)
                    .map_err(|_| {
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

impl ZedCollector {
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
            "zed.collection_failed",
            "Zed collection failed.",
            Some(code),
            counters,
        );
    }
}

fn supported_projections() -> Vec<CollectionProjection> {
    daily_session_projections()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use chrono::{DateTime, Utc};
    use tempfile::TempDir;

    use super::*;
    use crate::application::collection::{CollectionOutcome, CollectionScope, DetectionState};
    use crate::infrastructure::collectors::support::{
        daily_request as support_daily_request, detection_request, fixed_timestamp,
        session_request as support_session_request, NeverCancelled,
    };

    fn zstd_compress(payload: &str) -> Vec<u8> {
        zstd::stream::encode_all(payload.as_bytes(), 3).expect("compress")
    }

    const THREAD_JSON: &str = r#"{
        "title": "Exploration",
        "updated_at": "2026-08-09T03:49:28.634198070Z",
        "created_at": "2026-08-09T03:42:58.149142710Z",
        "cumulative_token_usage": {"input_tokens": 138468, "output_tokens": 9644, "cache_read_input_tokens": 1586296},
        "model": {"provider": "zed.dev", "model": "gpt-5.6-luna"},
        "messages": [{"User": {"id": "u1", "content": [{"Text": "redacted"}]}}]
    }"#;

    fn fixture_zed_dir() -> (TempDir, std::path::PathBuf) {
        let dir = TempDir::new().expect("dir");
        let threads_dir = dir.path().join("threads");
        fs::create_dir_all(&threads_dir).expect("threads dir");
        let conn = rusqlite::Connection::open(threads_dir.join("threads.db")).expect("db");
        conn.execute(
            "CREATE TABLE threads (id TEXT PRIMARY KEY, summary TEXT NOT NULL, updated_at TEXT NOT NULL, data_type TEXT NOT NULL, data BLOB NOT NULL, parent_id TEXT, worktree_branch TEXT, folder_paths TEXT, folder_paths_order TEXT, created_at TEXT)",
            [],
        )
        .expect("create");
        conn.execute(
            "INSERT INTO threads (id, summary, updated_at, data_type, data) VALUES ('t1', '', '2026-08-09T00:00:00Z', 'zstd', ?1)",
            rusqlite::params![zstd_compress(THREAD_JSON)],
        )
        .expect("insert");
        drop(conn);
        let path = dir.path().to_path_buf();
        (dir, path)
    }

    fn timestamp() -> DateTime<Utc> {
        fixed_timestamp(2026, 8, 9, 4, 0, 0)
    }

    #[test]
    fn describes_zed_profile() {
        let collector = ZedCollector::from_data_dir(PathBuf::from("/missing"));
        let descriptor = collector.describe().expect("descriptor");

        assert_eq!(descriptor.display_name, "Zed");
        assert_eq!(descriptor.profiles[0].source, SourceKey::Zed);
        assert_eq!(
            descriptor.profiles[0].supported_projections,
            vec![CollectionProjection::Daily, CollectionProjection::Session]
        );
    }

    #[test]
    fn detects_available_with_threads() {
        let (_dir, data_dir) = fixture_zed_dir();
        let collector = ZedCollector::from_data_dir(data_dir);

        let result = collector
            .detect(
                detection_request(SourceKey::Zed, timestamp()),
                &NeverCancelled,
            )
            .expect("detection");

        assert_eq!(result.state, DetectionState::Available);
        assert!(result.usage_artifacts_found);
    }

    #[test]
    fn detects_not_found_without_threads_db() {
        let collector = ZedCollector::from_data_dir(PathBuf::from("/missing/zed"));

        let result = collector
            .detect(
                detection_request(SourceKey::Zed, timestamp()),
                &NeverCancelled,
            )
            .expect("detection");

        assert_eq!(result.state, DetectionState::NotFound);
        assert_eq!(result.issues[0].code, ISSUE_THREADS_DB_MISSING);
    }

    #[test]
    fn collects_daily_usage_from_threads() {
        let (_dir, data_dir) = fixture_zed_dir();
        let collector = ZedCollector::from_data_dir(data_dir);

        let result = collector
            .collect(
                support_daily_request(
                    "zed-daily",
                    SourceKey::Zed,
                    CollectionScope::Full,
                    "Asia/Jakarta",
                    timestamp(),
                ),
                &NeverCancelled,
            )
            .expect("daily collection");

        assert_eq!(result.outcome(), CollectionOutcome::Complete);
        assert_eq!(result.daily_candidates().len(), 1);
        let daily = &result.daily_candidates()[0];
        assert_eq!(daily.source_key, "zed:daily:v1:Asia/Jakarta:2026-08-09");
        assert_eq!(daily.tokens.total_tokens(), 138468 + 9644 + 1586296);
        assert_eq!(daily.model_breakdowns[0].raw_model_id, "gpt-5.6-luna");
    }

    #[test]
    fn collects_session_usage_from_threads() {
        let (_dir, data_dir) = fixture_zed_dir();
        let collector = ZedCollector::from_data_dir(data_dir);

        let result = collector
            .collect(
                support_session_request("zed-session", SourceKey::Zed, timestamp()),
                &NeverCancelled,
            )
            .expect("session collection");

        assert_eq!(result.outcome(), CollectionOutcome::Complete);
        assert_eq!(result.session_candidates().len(), 1);
        let session = &result.session_candidates()[0];
        assert!(session.source_key.starts_with("zed:session:v1:t1:"));
        assert_eq!(session.tokens.total_tokens(), 138468 + 9644 + 1586296);
    }

    #[test]
    fn missing_threads_db_returns_empty() {
        let collector = ZedCollector::from_data_dir(PathBuf::from("/missing/zed"));

        let result = collector
            .collect(
                support_daily_request(
                    "zed-daily",
                    SourceKey::Zed,
                    CollectionScope::Full,
                    "UTC",
                    timestamp(),
                ),
                &NeverCancelled,
            )
            .expect("empty result");

        assert_eq!(result.outcome(), CollectionOutcome::Empty);
    }
}
