use std::path::{Path, PathBuf};
use std::time::Instant;

use chrono::{DateTime, Days, NaiveTime, TimeZone, Utc};
use chrono_tz::Tz;

use crate::application::collection::{
    CollectionMetadata, CollectionPeriod, CollectionProjection, CollectionRequest,
    CollectionResult, CollectionScope, CollectorDescriptor, CollectorFailure, CollectorFailureCode,
    CollectorIntegrity, DetectionRequest, DetectionResult, ProcessSummary,
};
use crate::application::ports::collector::{CancellationSignal, Collector};
use crate::domain::source::SourceKey;

use super::super::support::{
    available_detection, cancelled_detection, collector_key, daily_session_projections,
    detection_issue, invalid_configuration_detection, missing_or_invalid_location_code,
    not_found_detection, request_failure, single_source_descriptor, unsupported_detection,
    validate_source, validation_failure_as_internal, CollectorIdentity,
};
use super::mapper::{self, ZCodeMappingContext};
use super::ZCodeStore;

const COLLECTOR_KEY: &str = "zcode";
const DISPLAY_NAME: &str = "ZCode";
const COLLECTOR_VERSION: &str = "local";
const ADAPTER_VERSION: u16 = 1;
const PROFILE_VERSION: u16 = 1;
const IDENTITY: CollectorIdentity = CollectorIdentity {
    key: COLLECTOR_KEY,
    display_name: DISPLAY_NAME,
    runtime_version: COLLECTOR_VERSION,
    adapter_version: ADAPTER_VERSION,
    source: SourceKey::ZCode,
    profile_version: PROFILE_VERSION,
};

#[derive(Debug, Clone)]
pub(crate) struct ZCodeCollector {
    database_path: PathBuf,
}

impl ZCodeCollector {
    pub(crate) fn from_database_path(path: impl Into<PathBuf>) -> Self {
        Self {
            database_path: path.into(),
        }
    }

    #[allow(dead_code, reason = "default data root is wired in the runtime chunk")]
    pub(crate) fn from_data_dir(data_dir: impl AsRef<Path>) -> Self {
        Self::from_database_path(data_dir.as_ref().join("cli").join("db").join("db.sqlite"))
    }
}

impl Collector for ZCodeCollector {
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
        if request.source != SourceKey::ZCode {
            return Ok(unsupported_detection(
                &request,
                detection_issue("zcode.unsupported_source", "Source is not ZCode."),
            ));
        }
        if !self.database_path.exists() {
            return Ok(not_found_detection(
                &request,
                SourceKey::ZCode,
                supported_projections(),
                detection_issue(
                    "zcode.database_missing",
                    "ZCode usage database was not found.",
                ),
            ));
        }

        match ZCodeStore::open_read_only(&self.database_path)
            .and_then(|store| store.read_model_usage_between(0, i64::MAX))
        {
            Ok(rows) => Ok(available_detection(
                &request,
                SourceKey::ZCode,
                supported_projections(),
                !rows.is_empty(),
            )),
            Err(_) => Ok(invalid_configuration_detection(
                &request,
                SourceKey::ZCode,
                supported_projections(),
                detection_issue(
                    "zcode.database_incompatible",
                    "ZCode usage database is not readable by Burnly.",
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

        let store = ZCodeStore::open_read_only(&self.database_path).map_err(|_| {
            request_failure(
                &request,
                missing_or_invalid_location_code(&self.database_path),
            )
        })?;
        let (start_ms, end_ms) = collection_window(&request)?;
        let rows = store
            .read_model_usage_between(start_ms, end_ms)
            .map_err(|_| request_failure(&request, CollectorFailureCode::IncompatibleEnvelope))?;
        if cancellation.is_cancelled() {
            return Err(request_failure(&request, CollectorFailureCode::Cancelled));
        }

        let finished_at = Utc::now();
        let metadata = metadata(&request, started_at, finished_at)?;
        let context = ZCodeMappingContext::new(
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
                let candidates = mapper::map_daily(rows, timezone, request.scope(), &context)
                    .map_err(|_| {
                        request_failure(&request, CollectorFailureCode::IncompatibleEnvelope)
                    })?;
                CollectionResult::daily(
                    metadata,
                    candidates,
                    Vec::new(),
                    Vec::new(),
                    process_summary,
                )
                .map_err(|error| validation_failure_as_internal(&request, error))
            }
            CollectionProjection::Session => {
                let candidates = mapper::map_sessions(rows, &context).map_err(|_| {
                    request_failure(&request, CollectorFailureCode::IncompatibleEnvelope)
                })?;
                CollectionResult::session(
                    metadata,
                    candidates,
                    Vec::new(),
                    Vec::new(),
                    process_summary,
                )
                .map_err(|error| validation_failure_as_internal(&request, error))
            }
        }
    }
}

fn collection_window(request: &CollectionRequest) -> Result<(i64, i64), CollectorFailure> {
    match request.scope() {
        CollectionScope::Full => Ok((0, i64::MAX)),
        CollectionScope::Incremental(scope) => {
            let timezone = request
                .aggregation_timezone()
                .unwrap_or("UTC")
                .parse::<Tz>()
                .map_err(|_| {
                    request_failure(request, CollectorFailureCode::ScopeNotRepresentable)
                })?;
            let start = timezone
                .from_local_datetime(&scope.start_date().and_time(NaiveTime::MIN))
                .single()
                .ok_or_else(|| {
                    request_failure(request, CollectorFailureCode::ScopeNotRepresentable)
                })?;
            let end_date = scope
                .end_date()
                .checked_add_days(Days::new(1))
                .ok_or_else(|| {
                    request_failure(request, CollectorFailureCode::ScopeNotRepresentable)
                })?;
            let end = timezone
                .from_local_datetime(&end_date.and_time(NaiveTime::MIN))
                .single()
                .ok_or_else(|| {
                    request_failure(request, CollectorFailureCode::ScopeNotRepresentable)
                })?;
            Ok((start.timestamp_millis(), end.timestamp_millis()))
        }
    }
}

fn empty_result(
    request: &CollectionRequest,
    started: Instant,
    started_at: DateTime<Utc>,
) -> Result<CollectionResult, CollectorFailure> {
    let finished_at = Utc::now();
    let metadata = metadata(request, started_at, finished_at)?;
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

fn metadata(
    request: &CollectionRequest,
    started_at: DateTime<Utc>,
    finished_at: DateTime<Utc>,
) -> Result<CollectionMetadata, CollectorFailure> {
    CollectionMetadata::new(
        request.collection_id().clone(),
        collector_key(IDENTITY)?,
        COLLECTOR_VERSION.to_owned(),
        SourceKey::ZCode,
        request.scope().clone(),
        PROFILE_VERSION,
        CollectionPeriod {
            started_at,
            finished_at,
        },
    )
    .map_err(|error| validation_failure_as_internal(request, error))
}

fn validate_request(request: &CollectionRequest) -> Result<(), CollectorFailure> {
    validate_source(request, SourceKey::ZCode)
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
    use chrono::{NaiveDate, TimeZone};
    use rusqlite::Connection;
    use tempfile::NamedTempFile;

    use super::*;
    use crate::application::collection::{
        CollectionId, CollectionOutcome, CollectionScope, DetectionReason, DetectionState,
    };

    #[test]
    fn describes_zcode_profile() {
        let collector = ZCodeCollector::from_database_path("/missing");

        let descriptor = collector.describe().expect("descriptor");

        assert_eq!(descriptor.display_name, "ZCode");
        assert_eq!(descriptor.profiles[0].source, SourceKey::ZCode);
        assert_eq!(
            descriptor.profiles[0].supported_projections,
            vec![CollectionProjection::Daily, CollectionProjection::Session]
        );
    }

    #[test]
    fn detects_available_zcode_database() {
        let database = fixture_database("valid.sql");
        let collector = ZCodeCollector::from_database_path(database.path());

        let result = collector
            .detect(detection_request(SourceKey::ZCode), &NeverCancelled)
            .expect("detection");

        assert_eq!(result.state, DetectionState::Available);
        assert!(result.usage_artifacts_found);
    }

    #[test]
    fn detects_available_no_data_for_empty_database() {
        let database = fixture_database("empty.sql");
        let collector = ZCodeCollector::from_database_path(database.path());

        let result = collector
            .detect(detection_request(SourceKey::ZCode), &NeverCancelled)
            .expect("detection");

        assert_eq!(result.state, DetectionState::AvailableNoData);
        assert!(!result.usage_artifacts_found);
    }

    #[test]
    fn detects_missing_database() {
        let collector = ZCodeCollector::from_database_path("/missing/zcode.sqlite");

        let result = collector
            .detect(detection_request(SourceKey::ZCode), &NeverCancelled)
            .expect("detection");

        assert_eq!(result.state, DetectionState::NotFound);
    }

    #[test]
    fn rejects_non_zcode_collection_request() {
        let database = fixture_database("valid.sql");
        let collector = ZCodeCollector::from_database_path(database.path());

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
    fn collects_daily_usage_from_completed_model_usage_rows() {
        let database = fixture_database("valid.sql");
        let collector = ZCodeCollector::from_database_path(database.path());

        let result = collector
            .collect(daily_request(CollectionScope::Full), &NeverCancelled)
            .expect("daily collection");

        assert_eq!(result.outcome(), CollectionOutcome::Complete);
        assert_eq!(result.daily_candidates().len(), 1);
        let daily = &result.daily_candidates()[0];
        assert_eq!(daily.source_key, "zcode:daily:v1:Asia/Jakarta:2026-07-02");
        assert_eq!(daily.tokens.input_tokens(), Some(14_224));
        assert_eq!(daily.tokens.cache_read_tokens(), Some(7_360));
        assert_eq!(daily.tokens.total_tokens(), 24_883);
        assert_eq!(daily.model_breakdowns.len(), 2);
        assert!(daily
            .model_breakdowns
            .iter()
            .any(|model| model.raw_model_id == "GLM-5.2"));
        assert!(daily
            .model_breakdowns
            .iter()
            .any(|model| model.raw_model_id == "GLM-5-Turbo"));
    }

    #[test]
    fn applies_incremental_daily_scope() {
        let database = fixture_database("valid.sql");
        let collector = ZCodeCollector::from_database_path(database.path());

        let result = collector
            .collect(
                daily_request(
                    CollectionScope::incremental(
                        NaiveDate::from_ymd_opt(2026, 7, 3).expect("start"),
                        NaiveDate::from_ymd_opt(2026, 7, 3).expect("end"),
                    )
                    .expect("scope"),
                ),
                &NeverCancelled,
            )
            .expect("daily collection");

        assert_eq!(result.outcome(), CollectionOutcome::Empty);
        assert!(result.daily_candidates().is_empty());
    }

    #[test]
    fn collects_session_usage_grouped_by_session_and_model() {
        let database = fixture_database("valid.sql");
        let collector = ZCodeCollector::from_database_path(database.path());

        let result = collector
            .collect(session_request(), &NeverCancelled)
            .expect("session collection");

        assert_eq!(result.outcome(), CollectionOutcome::Complete);
        assert_eq!(result.session_candidates().len(), 2);
        let main = result
            .session_candidates()
            .iter()
            .find(|candidate| candidate.source_session_id == "sess-main")
            .expect("main session");
        assert_eq!(main.project_path, None);
        assert_eq!(main.first_activity_at, Some(millis(1_782_952_270_000)));
        assert_eq!(main.last_activity_at, Some(millis(1_782_952_275_000)));
    }

    struct NeverCancelled;

    impl CancellationSignal for NeverCancelled {
        fn is_cancelled(&self) -> bool {
            false
        }
    }

    fn fixture_database(name: &str) -> NamedTempFile {
        let database = NamedTempFile::new().expect("temp database");
        let connection = Connection::open(database.path()).expect("database");
        let sql = std::fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../tests/fixtures/collectors/zcode/db")
                .join(name),
        )
        .expect("fixture sql");
        connection.execute_batch(&sql).expect("fixture schema");
        drop(connection);
        database
    }

    fn detection_request(source: SourceKey) -> DetectionRequest {
        DetectionRequest {
            source,
            reason: DetectionReason::Startup,
            requested_at: timestamp(),
        }
    }

    fn daily_request(scope: CollectionScope) -> CollectionRequest {
        CollectionRequest::daily(
            CollectionId::new("zcode-daily").expect("collection id"),
            SourceKey::ZCode,
            scope,
            "Asia/Jakarta",
            timestamp(),
        )
        .expect("daily request")
    }

    fn session_request() -> CollectionRequest {
        CollectionRequest::session(
            CollectionId::new("zcode-session").expect("collection id"),
            SourceKey::ZCode,
            CollectionScope::Full,
            timestamp(),
        )
    }

    fn timestamp() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 7, 2, 12, 0, 0)
            .single()
            .expect("timestamp")
    }

    fn millis(value: i64) -> DateTime<Utc> {
        Utc.timestamp_millis_opt(value).single().expect("timestamp")
    }
}
