//! DeepSeek Harness collector adapter (detection stub).
//!
//! Phase 1 wires source identity and detection only. Collection is not yet
//! implemented; `collect` and `describe` fail closed until a later chunk wires
//! the session-log reader, usage fold, and mapper.

use std::path::{Path, PathBuf};

use crate::application::collection::{
    CollectionProjection, CollectionRequest, CollectionResult, CollectorDescriptor,
    CollectorFailure, CollectorFailureCode, DetectionIssue, DetectionRequest, DetectionResult,
};
use crate::application::ports::collector::{CancellationSignal, Collector};
use crate::domain::source::SourceKey;
use crate::infrastructure::collectors::support::{
    available_detection, cancelled_detection, detection_issue, invalid_configuration_detection,
    not_found_detection, unsupported_detection,
};

use super::detection::{inspect_deepseek_harness_home, DeepSeekHarnessHomeInspection};

#[allow(
    dead_code,
    reason = "collector identity used once wired in a later chunk"
)]
const COLLECTOR_KEY: &str = "deepseek-harness";
#[allow(
    dead_code,
    reason = "collector identity used once wired in a later chunk"
)]
const DISPLAY_NAME: &str = "DeepSeek Harness";
#[allow(dead_code, reason = "adapter version used once wired in a later chunk")]
const ADAPTER_VERSION: u16 = 1;

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

#[allow(
    dead_code,
    reason = "collector is constructed once wired in a later chunk"
)]
pub(crate) struct DeepSeekHarnessCollector {
    home: PathBuf,
}

impl DeepSeekHarnessCollector {
    #[allow(dead_code, reason = "constructor used once wired in a later chunk")]
    pub(crate) fn from_data_dir(home: PathBuf) -> Self {
        Self { home }
    }

    fn inspect(&self, override_path: Option<&Path>) -> DeepSeekHarnessHomeInspection {
        inspect_deepseek_harness_home(override_path)
    }

    fn inspect_stored_home(&self) -> DeepSeekHarnessHomeInspection {
        self.inspect(Some(&self.home))
    }

    fn supported_projections(&self) -> Vec<CollectionProjection> {
        vec![CollectionProjection::Daily, CollectionProjection::Session]
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
        Err(CollectorFailure::new(
            CollectorFailureCode::UnsupportedSource,
            Some(SourceKey::DeepSeekHarness),
            None,
        ))
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
        _cancellation: &dyn CancellationSignal,
    ) -> Result<CollectionResult, CollectorFailure> {
        Err(CollectorFailure::new(
            CollectorFailureCode::UnsupportedSource,
            Some(request.source()),
            Some(request.projection()),
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use chrono::{TimeZone, Utc};
    use tempfile::TempDir;

    use super::*;
    use crate::application::collection::{
        CollectionId, CollectionScope, DetectionReason, DetectionState,
    };

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

    fn collection_request(source: SourceKey) -> CollectionRequest {
        CollectionRequest::daily(
            CollectionId::new(format!("{}-daily", source.as_str())).expect("collection id"),
            source,
            CollectionScope::Full,
            "UTC",
            timestamp(),
        )
        .expect("request")
    }

    fn write_session_log(home: &Path, project: &str, session: &str, filename: &str) {
        let session_dir = home.join("sessions").join(project).join(session);
        fs::create_dir_all(&session_dir).expect("session dir");
        fs::write(session_dir.join(filename), b"{}").expect("session log");
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
    fn collect_fails_closed_until_native_collector_is_wired() {
        let temp = TempDir::new().expect("temp dir");
        let collector = DeepSeekHarnessCollector::from_data_dir(temp.path().join("home"));

        let failure = collector
            .collect(
                collection_request(SourceKey::DeepSeekHarness),
                &NeverCancelled,
            )
            .expect_err("collect fails closed");

        assert_eq!(failure.code, CollectorFailureCode::UnsupportedSource);
        assert_eq!(failure.source_key, Some(SourceKey::DeepSeekHarness));
    }
}
