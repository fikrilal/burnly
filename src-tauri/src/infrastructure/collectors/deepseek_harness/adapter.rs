//! DeepSeek Harness collector adapter.
//!
//! Wires session-root detection, bounded log reading, usage-only event
//! parsing, the replacement fold, and candidate mapping into the collector
//! port. Collection reads `$DSH_HOME/sessions` read-only.

use std::path::{Path, PathBuf};

use chrono::Utc;

use crate::application::collection::{
    CollectionProjection, CollectionRequest, CollectionResult, CollectionWarning,
    CollectorDescriptor, CollectorFailure, CollectorFailureCode, CollectorIntegrity,
    DetectionIssue, DetectionRequest, DetectionResult, RejectedRecord,
};
use crate::application::cost::BurnlyCostCalculator;
use crate::application::ports::collector::{CancellationSignal, Collector};
use crate::domain::source::SourceKey;
use crate::infrastructure::collectors::support::{
    available_detection, cancelled_detection, collection_metadata, daily_session_projections,
    detection_issue, empty_collection_result, invalid_configuration_detection, not_found_detection,
    path_is_missing, request_failure, single_source_descriptor, unsupported_detection,
    validate_source, validation_failure_preserving_all_rejected, CollectorIdentity,
    LocalCollectionRun,
};

use super::deepseek_home::sessions_root;
use super::detection::{inspect_deepseek_harness_home, DeepSeekHarnessHomeInspection};
use super::discovery::{inspect_session_directories, CURRENT_SESSION_FORMAT_VERSION};
use super::event_parser::parse_session_events;
use super::mapper::{
    map_daily, map_sessions, DeepSeekHarnessMappingContext, SessionUsage, COLLECTOR_KEY,
    PROFILE_VERSION,
};
use super::session_log_reader::read_session_log;
use super::usage_fold::fold_events;

const DISPLAY_NAME: &str = "DeepSeek Harness";
const COLLECTOR_VERSION: &str = "local";
const ADAPTER_VERSION: u16 = 1;

const REJECTION_UNSUPPORTED_SESSION_FORMAT: &str = "deepseek_harness.unsupported_session_format";
const REJECTION_OLDER_SESSION_FORMAT: &str = "deepseek_harness.older_session_format";
const REJECTION_SESSION_DIRECTORY_UNREADABLE: &str =
    "deepseek_harness.session_directory_unreadable";
const REJECTION_SESSION_LOG_UNREADABLE: &str = "deepseek_harness.session_log_unreadable";
const WARNING_TORN_FINAL_FRAME: &str = "deepseek_harness.torn_final_frame";

const IDENTITY: CollectorIdentity = CollectorIdentity {
    key: COLLECTOR_KEY,
    display_name: DISPLAY_NAME,
    runtime_version: COLLECTOR_VERSION,
    adapter_version: ADAPTER_VERSION,
    source: SourceKey::DeepSeekHarness,
    profile_version: PROFILE_VERSION,
};

/// Issue codes emitted by DeepSeek Harness detection.
pub(crate) const ISSUE_HOME_MISSING: &str = "deepseek_harness.home_missing";
pub(crate) const ISSUE_SESSIONS_MISSING: &str = "deepseek_harness.sessions_missing";
pub(crate) const ISSUE_SESSIONS_UNREADABLE: &str = "deepseek_harness.sessions_unreadable";
pub(crate) const ISSUE_SESSION_DIRECTORY_UNREADABLE: &str =
    "deepseek_harness.session_directory_unreadable";
pub(crate) const ISSUE_NO_SESSION_LOGS: &str = "deepseek_harness.no_session_logs";
pub(crate) const ISSUE_OLDER_FORMAT_ONLY: &str = "deepseek_harness.older_format_only";
pub(crate) const ISSUE_UNSUPPORTED_SESSION_FORMAT: &str =
    "deepseek_harness.unsupported_session_format";

#[derive(Debug, Clone)]
pub(crate) struct DeepSeekHarnessCollector {
    home: PathBuf,
    calculator: BurnlyCostCalculator,
}

impl DeepSeekHarnessCollector {
    pub(crate) fn from_data_dir(home: PathBuf) -> Self {
        Self {
            home,
            calculator: BurnlyCostCalculator::new(),
        }
    }

    fn inspect(&self, override_path: Option<&Path>) -> DeepSeekHarnessHomeInspection {
        inspect_deepseek_harness_home(override_path)
    }

    fn inspect_stored_home(&self) -> DeepSeekHarnessHomeInspection {
        self.inspect(Some(&self.home))
    }

    fn supported_projections(&self) -> Vec<CollectionProjection> {
        daily_session_projections()
    }

    fn detection_issues(&self, inspection: &DeepSeekHarnessHomeInspection) -> Vec<DetectionIssue> {
        let mut issues = Vec::new();
        if !inspection.home_exists {
            issues.push(detection_issue(
                ISSUE_HOME_MISSING,
                "DeepSeek Harness home directory was not found.",
            ));
            return issues;
        }
        if !inspection.sessions_root_exists {
            issues.push(detection_issue(
                ISSUE_SESSIONS_MISSING,
                "DeepSeek Harness sessions directory was not found.",
            ));
            return issues;
        }
        if !inspection.sessions_root_readable {
            issues.push(detection_issue(
                ISSUE_SESSIONS_UNREADABLE,
                "DeepSeek Harness sessions directory is not readable by Burnly.",
            ));
            return issues;
        }
        if inspection.newer_format_session_logs > 0 {
            issues.push(detection_issue(
                ISSUE_UNSUPPORTED_SESSION_FORMAT,
                "A DeepSeek Harness session format is newer than the supported format.",
            ));
        }
        if inspection.older_format_session_logs > 0 {
            issues.push(detection_issue(
                ISSUE_OLDER_FORMAT_ONLY,
                "Older DeepSeek Harness session generations were found; reader support is not yet implemented.",
            ));
        }
        if inspection.unreadable_session_directories > 0 {
            issues.push(detection_issue(
                ISSUE_SESSION_DIRECTORY_UNREADABLE,
                "At least one DeepSeek Harness session directory is not readable by Burnly.",
            ));
        }
        if !inspection.has_supported_session_logs() && issues.is_empty() {
            issues.push(detection_issue(
                ISSUE_NO_SESSION_LOGS,
                "No DeepSeek Harness session logs were found.",
            ));
        }
        issues
    }
}

impl Collector for DeepSeekHarnessCollector {
    fn describe(&self) -> Result<CollectorDescriptor, CollectorFailure> {
        single_source_descriptor(
            IDENTITY,
            self.supported_projections(),
            CollectorIntegrity::UnverifiedDevelopment,
        )
    }

    fn detect(
        &self,
        request: DetectionRequest,
        cancellation: &dyn CancellationSignal,
    ) -> Result<DetectionResult, CollectorFailure> {
        if request.source != SourceKey::DeepSeekHarness {
            return Ok(unsupported_detection(
                &request,
                detection_issue(
                    "deepseek_harness.unsupported_source",
                    "Source is not DeepSeek Harness.",
                ),
            ));
        }
        if cancellation.is_cancelled() {
            return Ok(cancelled_detection(&request));
        }

        let inspection = self.inspect_stored_home();
        let projections = self.supported_projections();
        let issues = self.detection_issues(&inspection);

        if let Some(first) = issues.first() {
            match first.code.as_str() {
                ISSUE_HOME_MISSING | ISSUE_SESSIONS_MISSING => {
                    return Ok(not_found_detection(
                        &request,
                        SourceKey::DeepSeekHarness,
                        projections,
                        first.clone(),
                    ));
                }
                ISSUE_SESSIONS_UNREADABLE => {
                    return Ok(invalid_configuration_detection(
                        &request,
                        SourceKey::DeepSeekHarness,
                        projections,
                        first.clone(),
                    ));
                }
                ISSUE_UNSUPPORTED_SESSION_FORMAT if !inspection.has_supported_session_logs() => {
                    return Ok(unsupported_detection(&request, first.clone()));
                }
                _ => {}
            }
        }

        let mut result = available_detection(
            &request,
            SourceKey::DeepSeekHarness,
            projections,
            inspection.has_supported_session_logs(),
        );
        result.issues = issues;
        Ok(result)
    }

    fn collect(
        &self,
        request: CollectionRequest,
        cancellation: &dyn CancellationSignal,
    ) -> Result<CollectionResult, CollectorFailure> {
        let run = LocalCollectionRun::start();
        validate_source(&request, SourceKey::DeepSeekHarness)?;
        if cancellation.is_cancelled() {
            return Err(request_failure(&request, CollectorFailureCode::Cancelled));
        }

        let sessions_root = sessions_root(&self.home);
        if path_is_missing(&sessions_root) {
            return empty_collection_result(IDENTITY, &request, &run);
        }
        if !sessions_root.is_dir() {
            return Err(request_failure(
                &request,
                CollectorFailureCode::SourceInvalidLocation,
            ));
        }

        let inspection = match inspect_session_directories(&sessions_root) {
            Ok(inspection) => inspection,
            Err(_) => {
                return Err(request_failure(
                    &request,
                    CollectorFailureCode::SourcePermissionDenied,
                ));
            }
        };
        if inspection.current_format_directories == 0 {
            if inspection.newer_format_directories > 0 {
                return Err(request_failure(
                    &request,
                    CollectorFailureCode::IncompatibleEnvelope,
                ));
            }
            let mut rejections = Vec::new();
            extend_rejections(
                &mut rejections,
                REJECTION_OLDER_SESSION_FORMAT,
                inspection.older_format_directories,
            );
            extend_rejections(
                &mut rejections,
                REJECTION_SESSION_DIRECTORY_UNREADABLE,
                inspection.unreadable_session_directories,
            );
            if !rejections.is_empty() {
                return Err(request_failure(
                    &request,
                    CollectorFailureCode::AllRecordsRejected,
                ));
            }
            return empty_collection_result(IDENTITY, &request, &run);
        }

        let mut rejections = Vec::new();
        let mut warnings = Vec::new();
        extend_rejections(
            &mut rejections,
            REJECTION_UNSUPPORTED_SESSION_FORMAT,
            inspection.newer_format_directories,
        );
        extend_rejections(
            &mut rejections,
            REJECTION_OLDER_SESSION_FORMAT,
            inspection.older_format_directories,
        );
        extend_rejections(
            &mut rejections,
            REJECTION_SESSION_DIRECTORY_UNREADABLE,
            inspection.unreadable_session_directories,
        );

        let mut sessions = Vec::new();
        for file in &inspection.files {
            if cancellation.is_cancelled() {
                return Err(request_failure(&request, CollectorFailureCode::Cancelled));
            }
            if file.format_version != CURRENT_SESSION_FORMAT_VERSION {
                continue;
            }

            let decoded = match read_session_log(file) {
                Ok(decoded) => decoded,
                Err(_) => {
                    rejections.push(RejectedRecord {
                        code: REJECTION_SESSION_LOG_UNREADABLE.to_owned(),
                        record_index: None,
                    });
                    continue;
                }
            };
            let parsed = parse_session_events(&decoded.events_jsonl);
            for rejection in &parsed.rejections {
                rejections.push(RejectedRecord {
                    code: rejection.code.to_owned(),
                    record_index: rejection.seq.and_then(|seq| u32::try_from(seq).ok()),
                });
            }
            let observations = fold_events(&parsed.events);
            if decoded.truncated_tail {
                warnings.push(CollectionWarning {
                    code: WARNING_TORN_FINAL_FRAME.to_owned(),
                    message: "DeepSeek Harness session log ended with a torn final frame; complete records were imported."
                        .to_owned(),
                });
            }
            if !observations.is_empty() {
                sessions.push(SessionUsage {
                    header: decoded.header,
                    observations,
                });
            }
        }

        let finished_at = Utc::now();
        let metadata = collection_metadata(IDENTITY, &request, run.started_at(), finished_at)?;
        let context = DeepSeekHarnessMappingContext::new(
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
                let candidates = map_daily(
                    &sessions,
                    timezone,
                    request.scope(),
                    &context,
                    &self.calculator,
                )
                .map_err(|_| {
                    request_failure(&request, CollectorFailureCode::IncompatibleEnvelope)
                })?;
                CollectionResult::daily(metadata, candidates, rejections, warnings, process_summary)
                    .map_err(|error| validation_failure_preserving_all_rejected(&request, error))
            }
            CollectionProjection::Session => {
                let candidates =
                    map_sessions(sessions, &context, &self.calculator).map_err(|_| {
                        request_failure(&request, CollectorFailureCode::IncompatibleEnvelope)
                    })?;
                CollectionResult::session(
                    metadata,
                    candidates,
                    rejections,
                    warnings,
                    process_summary,
                )
                .map_err(|error| validation_failure_preserving_all_rejected(&request, error))
            }
        }
    }
}

fn extend_rejections(rejections: &mut Vec<RejectedRecord>, code: &str, count: u32) {
    rejections.extend((0..count).map(|_| RejectedRecord {
        code: code.to_owned(),
        record_index: None,
    }));
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use chrono::{TimeZone, Utc};
    use tempfile::TempDir;

    use super::*;
    use crate::application::collection::{
        CollectionId, CollectionOutcome, CollectionScope, DetectionReason, DetectionState,
    };

    const VALID_ROOT: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/collectors/deepseek-harness/sessions/valid-root-session.jsonl"
    ));

    fn timestamp() -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 4, 1, 2, 3)
            .single()
            .expect("timestamp")
    }

    fn detection_request(source: SourceKey) -> DetectionRequest {
        DetectionRequest {
            source,
            reason: DetectionReason::Startup,
            requested_at: timestamp(),
        }
    }

    fn daily_request(source: SourceKey) -> CollectionRequest {
        CollectionRequest::daily(
            CollectionId::new(format!("{}-daily", source.as_str())).expect("collection id"),
            source,
            CollectionScope::Full,
            "UTC",
            timestamp(),
        )
        .expect("request")
    }

    fn session_request(source: SourceKey) -> CollectionRequest {
        CollectionRequest::session(
            CollectionId::new(format!("{}-session", source.as_str())).expect("collection id"),
            source,
            CollectionScope::Full,
            timestamp(),
        )
    }

    fn write_session_log(home: &Path, project: &str, session: &str, filename: &str) {
        let session_dir = home.join("sessions").join(project).join(session);
        fs::create_dir_all(&session_dir).expect("session dir");
        fs::write(session_dir.join(filename), b"{}").expect("session log");
    }

    fn write_valid_compressed_session_log(home: &Path) -> PathBuf {
        let (header, events) = VALID_ROOT.split_once('\n').expect("fixture header");
        let mut bytes =
            zstd::stream::encode_all(format!("{header}\n").as_bytes(), 3).expect("header frame");
        bytes.extend(zstd::stream::encode_all(events.as_bytes(), 3).expect("events frame"));
        let session_dir = home.join("sessions").join("--project--").join("session-a");
        fs::create_dir_all(&session_dir).expect("session dir");
        let path = session_dir.join("session.v4.jsonl.zstd");
        fs::write(&path, bytes).expect("session log");
        path
    }

    struct NeverCancelled;

    impl CancellationSignal for NeverCancelled {
        fn is_cancelled(&self) -> bool {
            false
        }
    }

    #[test]
    fn detects_available_with_current_format_session_log() {
        let temp = TempDir::new().expect("temp dir");
        let home = temp.path().join("dsh-home");
        write_session_log(&home, "--project--", "session-a", "session.v4.jsonl.zstd");

        let collector = DeepSeekHarnessCollector::from_data_dir(home);
        let result = collector
            .detect(
                detection_request(SourceKey::DeepSeekHarness),
                &NeverCancelled,
            )
            .expect("detect");

        assert_eq!(result.source, SourceKey::DeepSeekHarness);
        assert_eq!(result.state, DetectionState::Available);
        assert!(result.usage_artifacts_found);
        assert!(result.issues.is_empty());
    }

    #[test]
    fn detects_available_no_data_with_only_older_format() {
        let temp = TempDir::new().expect("temp dir");
        let home = temp.path().join("dsh-home");
        write_session_log(&home, "--project--", "session-a", "session.v3.jsonl.zstd");

        let collector = DeepSeekHarnessCollector::from_data_dir(home);
        let result = collector
            .detect(
                detection_request(SourceKey::DeepSeekHarness),
                &NeverCancelled,
            )
            .expect("detect");

        assert_eq!(result.state, DetectionState::AvailableNoData);
        assert!(!result.usage_artifacts_found);
        assert_eq!(result.issues[0].code, ISSUE_OLDER_FORMAT_ONLY);
    }

    #[test]
    fn detects_unsupported_with_only_newer_format() {
        let temp = TempDir::new().expect("temp dir");
        let home = temp.path().join("dsh-home");
        write_session_log(&home, "--project--", "session-a", "session.v5.jsonl.zstd");

        let collector = DeepSeekHarnessCollector::from_data_dir(home);
        let result = collector
            .detect(
                detection_request(SourceKey::DeepSeekHarness),
                &NeverCancelled,
            )
            .expect("detect");

        assert_eq!(result.state, DetectionState::Unsupported);
        assert_eq!(result.issues[0].code, ISSUE_UNSUPPORTED_SESSION_FORMAT);
    }

    #[test]
    fn detects_available_with_warning_when_some_sessions_are_newer() {
        let temp = TempDir::new().expect("temp dir");
        let home = temp.path().join("dsh-home");
        write_session_log(&home, "--project--", "session-a", "session.v4.jsonl.zstd");
        write_session_log(&home, "--project--", "session-b", "session.v5.jsonl.zstd");

        let collector = DeepSeekHarnessCollector::from_data_dir(home);
        let result = collector
            .detect(
                detection_request(SourceKey::DeepSeekHarness),
                &NeverCancelled,
            )
            .expect("detect");

        assert_eq!(result.state, DetectionState::Available);
        assert!(result.usage_artifacts_found);
        assert_eq!(result.issues.len(), 1);
        assert_eq!(result.issues[0].code, ISSUE_UNSUPPORTED_SESSION_FORMAT);
    }

    #[test]
    fn reports_not_found_when_home_is_missing() {
        let temp = TempDir::new().expect("temp dir");
        let home = temp.path().join("missing-home");

        let collector = DeepSeekHarnessCollector::from_data_dir(home);
        let result = collector
            .detect(
                detection_request(SourceKey::DeepSeekHarness),
                &NeverCancelled,
            )
            .expect("detect");

        assert_eq!(result.state, DetectionState::NotFound);
        assert_eq!(result.issues[0].code, ISSUE_HOME_MISSING);
    }

    #[test]
    fn reports_not_found_when_sessions_root_is_missing() {
        let temp = TempDir::new().expect("temp dir");
        let home = temp.path().join("dsh-home");
        fs::create_dir_all(&home).expect("home dir");

        let collector = DeepSeekHarnessCollector::from_data_dir(home);
        let result = collector
            .detect(
                detection_request(SourceKey::DeepSeekHarness),
                &NeverCancelled,
            )
            .expect("detect");

        assert_eq!(result.state, DetectionState::NotFound);
        assert_eq!(result.issues[0].code, ISSUE_SESSIONS_MISSING);
    }

    #[test]
    fn reports_available_no_data_when_sessions_root_is_empty() {
        let temp = TempDir::new().expect("temp dir");
        let home = temp.path().join("dsh-home");
        fs::create_dir_all(home.join("sessions")).expect("sessions dir");

        let collector = DeepSeekHarnessCollector::from_data_dir(home);
        let result = collector
            .detect(
                detection_request(SourceKey::DeepSeekHarness),
                &NeverCancelled,
            )
            .expect("detect");

        assert_eq!(result.state, DetectionState::AvailableNoData);
        assert!(!result.usage_artifacts_found);
        assert_eq!(result.issues[0].code, ISSUE_NO_SESSION_LOGS);
    }

    #[cfg(unix)]
    #[test]
    fn reports_invalid_configuration_when_sessions_root_is_unreadable() {
        use std::os::unix::fs::PermissionsExt;

        let temp = TempDir::new().expect("temp dir");
        let home = temp.path().join("dsh-home");
        let sessions = home.join("sessions");
        fs::create_dir_all(&sessions).expect("sessions dir");
        fs::set_permissions(&sessions, fs::Permissions::from_mode(0o000))
            .expect("remove permissions");

        let collector = DeepSeekHarnessCollector::from_data_dir(home);
        let result = collector
            .detect(
                detection_request(SourceKey::DeepSeekHarness),
                &NeverCancelled,
            )
            .expect("detect");

        fs::set_permissions(&sessions, fs::Permissions::from_mode(0o700))
            .expect("restore permissions");

        assert_eq!(result.state, DetectionState::InvalidConfiguration);
        assert_eq!(result.issues[0].code, ISSUE_SESSIONS_UNREADABLE);
    }

    #[cfg(unix)]
    #[test]
    fn reports_available_no_data_when_session_directory_is_unreadable() {
        use std::os::unix::fs::PermissionsExt;

        let temp = TempDir::new().expect("temp dir");
        let home = temp.path().join("dsh-home");
        write_session_log(&home, "--project--", "session-a", "session.v4.jsonl.zstd");
        let session_dir = home.join("sessions").join("--project--").join("session-a");
        fs::set_permissions(&session_dir, fs::Permissions::from_mode(0o000))
            .expect("remove permissions");

        let collector = DeepSeekHarnessCollector::from_data_dir(home);
        let result = collector
            .detect(
                detection_request(SourceKey::DeepSeekHarness),
                &NeverCancelled,
            )
            .expect("detect");

        fs::set_permissions(&session_dir, fs::Permissions::from_mode(0o700))
            .expect("restore permissions");

        assert_eq!(result.state, DetectionState::AvailableNoData);
        assert!(!result.usage_artifacts_found);
        assert_eq!(result.issues[0].code, ISSUE_SESSION_DIRECTORY_UNREADABLE);
    }

    #[test]
    fn rejects_non_deepseek_harness_source_in_detection() {
        let temp = TempDir::new().expect("temp dir");
        let collector = DeepSeekHarnessCollector::from_data_dir(temp.path().join("home"));

        let result = collector
            .detect(detection_request(SourceKey::Zed), &NeverCancelled)
            .expect("detect");

        assert_eq!(result.state, DetectionState::Unsupported);
        assert_eq!(result.issues[0].code, "deepseek_harness.unsupported_source");
    }

    #[test]
    fn describes_deepseek_harness_profile() {
        let temp = TempDir::new().expect("temp dir");
        let collector = DeepSeekHarnessCollector::from_data_dir(temp.path().join("home"));

        let descriptor = collector.describe().expect("descriptor");

        assert_eq!(descriptor.collector.as_str(), COLLECTOR_KEY);
        assert_eq!(descriptor.display_name, DISPLAY_NAME);
        assert_eq!(descriptor.profiles.len(), 1);
        assert_eq!(descriptor.profiles[0].source, SourceKey::DeepSeekHarness);
        assert_eq!(descriptor.profiles[0].profile_version, PROFILE_VERSION);
        assert_eq!(
            descriptor.profiles[0].supported_projections,
            vec![CollectionProjection::Daily, CollectionProjection::Session]
        );
    }

    #[test]
    fn empty_missing_home_collection_is_successful_empty() {
        let temp = TempDir::new().expect("temp dir");
        let collector = DeepSeekHarnessCollector::from_data_dir(temp.path().join("missing"));

        let result = collector
            .collect(daily_request(SourceKey::DeepSeekHarness), &NeverCancelled)
            .expect("collection");

        assert_eq!(result.outcome(), CollectionOutcome::Empty);
        assert!(result.daily_candidates().is_empty());
    }

    #[test]
    fn collects_daily_usage_from_compressed_session_log() {
        let temp = TempDir::new().expect("temp dir");
        let home = temp.path().join("dsh-home");
        write_valid_compressed_session_log(&home);
        let collector = DeepSeekHarnessCollector::from_data_dir(home);

        let result = collector
            .collect(daily_request(SourceKey::DeepSeekHarness), &NeverCancelled)
            .expect("collection");

        assert_eq!(result.outcome(), CollectionOutcome::Complete);
        assert_eq!(result.rejection_count(), 0);
        let candidates = result.daily_candidates();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].tokens.total_tokens(), 12);
        assert_eq!(candidates[0].model_breakdowns.len(), 1);
        assert_eq!(
            candidates[0].model_breakdowns[0].raw_model_id,
            "deepseek-flash"
        );
    }

    #[test]
    fn collects_session_usage_from_compressed_session_log() {
        let temp = TempDir::new().expect("temp dir");
        let home = temp.path().join("dsh-home");
        write_valid_compressed_session_log(&home);
        let collector = DeepSeekHarnessCollector::from_data_dir(home);

        let result = collector
            .collect(session_request(SourceKey::DeepSeekHarness), &NeverCancelled)
            .expect("collection");

        assert_eq!(result.outcome(), CollectionOutcome::Complete);
        let candidates = result.session_candidates();
        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0].source_session_id,
            "session-00000000-0000-0000-0000-000000000000"
        );
        assert_eq!(
            candidates[0].project_path.as_deref(),
            Some("/redacted/project")
        );
        assert_eq!(candidates[0].tokens.total_tokens(), 12);
    }

    #[test]
    fn older_format_only_collection_fails_closed() {
        let temp = TempDir::new().expect("temp dir");
        let home = temp.path().join("dsh-home");
        write_session_log(&home, "--project--", "session-a", "session.v3.jsonl.zstd");
        let collector = DeepSeekHarnessCollector::from_data_dir(home);

        let failure = collector
            .collect(daily_request(SourceKey::DeepSeekHarness), &NeverCancelled)
            .expect_err("unsupported older format");

        assert_eq!(failure.code, CollectorFailureCode::AllRecordsRejected);
    }

    #[cfg(unix)]
    #[test]
    fn unreadable_session_directory_only_collection_fails_closed() {
        use std::os::unix::fs::PermissionsExt;

        let temp = TempDir::new().expect("temp dir");
        let home = temp.path().join("dsh-home");
        write_session_log(&home, "--project--", "session-a", "session.v4.jsonl.zstd");
        let session_dir = home.join("sessions").join("--project--").join("session-a");
        fs::set_permissions(&session_dir, fs::Permissions::from_mode(0o000))
            .expect("remove permissions");

        let collector = DeepSeekHarnessCollector::from_data_dir(home);
        let failure = collector
            .collect(daily_request(SourceKey::DeepSeekHarness), &NeverCancelled)
            .expect_err("unreadable session directory");

        fs::set_permissions(&session_dir, fs::Permissions::from_mode(0o700))
            .expect("restore permissions");

        assert_eq!(failure.code, CollectorFailureCode::AllRecordsRejected);
    }

    #[test]
    fn mixed_current_and_older_formats_produce_partial_collection() {
        let temp = TempDir::new().expect("temp dir");
        let home = temp.path().join("dsh-home");
        write_valid_compressed_session_log(&home);
        write_session_log(&home, "--project--", "session-b", "session.v3.jsonl.zstd");
        let collector = DeepSeekHarnessCollector::from_data_dir(home);

        let result = collector
            .collect(daily_request(SourceKey::DeepSeekHarness), &NeverCancelled)
            .expect("collection");

        assert_eq!(result.outcome(), CollectionOutcome::Partial);
        assert_eq!(result.rejection_count(), 1);
        assert_eq!(result.daily_candidates().len(), 1);
    }

    #[test]
    fn newer_format_only_collection_fails_closed() {
        let temp = TempDir::new().expect("temp dir");
        let home = temp.path().join("dsh-home");
        write_session_log(&home, "--project--", "session-a", "session.v5.jsonl.zstd");
        let collector = DeepSeekHarnessCollector::from_data_dir(home);

        let failure = collector
            .collect(daily_request(SourceKey::DeepSeekHarness), &NeverCancelled)
            .expect_err("unsupported format");

        assert_eq!(failure.code, CollectorFailureCode::IncompatibleEnvelope);
    }
}
