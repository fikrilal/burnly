//! Command Code collector adapter (detection stub).
//!
//! Phase 1 wires source identity and detection only. Collection is not yet
//! implemented; `collect` and `describe` fail closed until a later chunk wires
//! the transcript reader and mapper.

use std::path::{Path, PathBuf};

use crate::application::collection::{
    CollectionProjection, CollectionRequest, CollectionResult, CollectorDescriptor,
    CollectorFailure, CollectorFailureCode, DetectionIssue, DetectionRequest, DetectionResult,
};
use crate::application::ports::collector::{CancellationSignal, Collector};
use crate::domain::source::SourceKey;
use crate::infrastructure::collectors::support::{
    available_detection, cancelled_detection, detection_issue, not_found_detection,
    unsupported_detection,
};

use super::detection::{inspect_commandcode_home, CommandCodeHomeInspection};

#[allow(
    dead_code,
    reason = "collector identity used once wired in a later chunk"
)]
const COLLECTOR_KEY: &str = "command-code";
#[allow(
    dead_code,
    reason = "collector identity used once wired in a later chunk"
)]
const DISPLAY_NAME: &str = "Command Code";
#[allow(dead_code, reason = "adapter version used once wired in a later chunk")]
const ADAPTER_VERSION: u16 = 1;

/// Issue codes emitted by Command Code detection.
pub(crate) const ISSUE_HOME_MISSING: &str = "commandcode.home_missing";
pub(crate) const ISSUE_PROJECTS_MISSING: &str = "commandcode.projects_missing";
pub(crate) const ISSUE_PROJECTS_UNREADABLE: &str = "commandcode.projects_unreadable";
pub(crate) const ISSUE_NO_USAGE_TRANSCRIPTS: &str = "commandcode.no_usage_transcripts";
pub(crate) const ISSUE_LEGACY_ONLY_TRANSCRIPTS: &str = "commandcode.legacy_only_transcripts";

#[allow(
    dead_code,
    reason = "collector is constructed once wired in a later chunk"
)]
pub(crate) struct CommandCodeCollector {
    commandcode_home: PathBuf,
}

impl CommandCodeCollector {
    #[allow(dead_code, reason = "constructor used once wired in a later chunk")]
    pub(crate) fn from_data_dir(commandcode_home: PathBuf) -> Self {
        Self { commandcode_home }
    }

    fn inspect(&self, override_path: Option<&Path>) -> CommandCodeHomeInspection {
        inspect_commandcode_home(override_path)
    }

    fn inspect_stored_home(&self) -> CommandCodeHomeInspection {
        self.inspect(Some(&self.commandcode_home))
    }

    fn supported_projections(&self) -> Vec<CollectionProjection> {
        vec![CollectionProjection::Daily, CollectionProjection::Session]
    }

    fn detection_issues(&self, inspection: &CommandCodeHomeInspection) -> Vec<DetectionIssue> {
        let mut issues = Vec::new();
        if !inspection.commandcode_home_exists {
            issues.push(detection_issue(
                ISSUE_HOME_MISSING,
                "Command Code data directory was not found.",
            ));
        } else if !inspection.projects_root_exists {
            issues.push(detection_issue(
                ISSUE_PROJECTS_MISSING,
                "Command Code projects directory was not found.",
            ));
        } else if !inspection.projects_root_readable {
            issues.push(detection_issue(
                ISSUE_PROJECTS_UNREADABLE,
                "Command Code projects directory is not readable by Burnly.",
            ));
        } else if inspection.new_format_transcripts == 0 && inspection.legacy_transcripts > 0 {
            issues.push(detection_issue(
                ISSUE_LEGACY_ONLY_TRANSCRIPTS,
                "Only legacy Command Code transcripts were found; they contain no usage data.",
            ));
        } else if !inspection.has_usage_transcripts {
            issues.push(detection_issue(
                ISSUE_NO_USAGE_TRANSCRIPTS,
                "No Command Code transcripts with usage data were found.",
            ));
        }
        issues
    }
}

impl Collector for CommandCodeCollector {
    fn describe(&self) -> Result<CollectorDescriptor, CollectorFailure> {
        Err(CollectorFailure::new(
            CollectorFailureCode::UnsupportedSource,
            Some(SourceKey::CommandCode),
            None,
        ))
    }

    fn detect(
        &self,
        request: DetectionRequest,
        cancellation: &dyn CancellationSignal,
    ) -> Result<DetectionResult, CollectorFailure> {
        if request.source != SourceKey::CommandCode {
            return Ok(unsupported_detection(
                &request,
                detection_issue(
                    "commandcode.unsupported_source",
                    "Source is not Command Code.",
                ),
            ));
        }
        if cancellation.is_cancelled() {
            return Ok(cancelled_detection(&request));
        }

        let inspection = self.inspect_stored_home();
        let projections = self.supported_projections();
        let issues = self.detection_issues(&inspection);

        if issues.is_empty() {
            return Ok(available_detection(
                &request,
                SourceKey::CommandCode,
                projections,
                true,
            ));
        }

        let first_issue = issues[0].clone();
        // The data root exists but holds no usable usage data yet; the source
        // is installed, just not collecting.
        if first_issue.code == ISSUE_LEGACY_ONLY_TRANSCRIPTS
            || first_issue.code == ISSUE_NO_USAGE_TRANSCRIPTS
        {
            let mut result =
                available_detection(&request, SourceKey::CommandCode, projections, false);
            result.issues.push(first_issue);
            return Ok(result);
        }
        // The data root itself is absent or unreadable.
        Ok(not_found_detection(
            &request,
            SourceKey::CommandCode,
            projections,
            first_issue,
        ))
    }

    fn collect(
        &self,
        request: CollectionRequest,
        _cancellation: &dyn CancellationSignal,
    ) -> Result<CollectionResult, CollectorFailure> {
        if request.source() != SourceKey::CommandCode {
            return Err(CollectorFailure::new(
                CollectorFailureCode::UnsupportedSource,
                Some(request.source()),
                Some(request.projection()),
            ));
        }
        Err(CollectorFailure::new(
            CollectorFailureCode::UnsupportedSource,
            Some(SourceKey::CommandCode),
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
        Utc.with_ymd_and_hms(2026, 8, 4, 1, 2, 3)
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

    struct NeverCancelled;

    impl CancellationSignal for NeverCancelled {
        fn is_cancelled(&self) -> bool {
            false
        }
    }

    const VALID_TRANSCRIPT: &str = r#"{"type":"session","version":3,"id":"sess-1","timestamp":"2026-08-04T10:00:00Z","cwd":"/tmp/proj"}
{"type":"message","id":"m1","parentId":null,"timestamp":"2026-08-04T10:00:01Z","message":{"role":"assistant","content":[{"type":"text","text":"hi"}]},"usage":{"inputTokens":10,"outputTokens":2,"cacheReadTokens":3,"cacheWriteTokens":0,"costUsd":0.001},"model":"deepseek/deepseek-v4-flash","effort":"max"}"#;

    fn write_transcript(project_dir: &std::path::Path, name: &str, contents: &str) {
        fs::create_dir_all(project_dir).expect("project dir");
        fs::write(project_dir.join(name), contents).expect("transcript");
    }

    #[test]
    fn detects_available_with_valid_transcript() {
        let temp = TempDir::new().expect("temp dir");
        let commandcode_home = temp.path().join("commandcode-home");
        write_transcript(
            &commandcode_home.join("projects").join("proj-a"),
            "sess-1.jsonl",
            VALID_TRANSCRIPT,
        );

        let collector = CommandCodeCollector::from_data_dir(commandcode_home);
        let result = collector
            .detect(detection_request(SourceKey::CommandCode), &NeverCancelled)
            .expect("detect");

        assert_eq!(result.source, SourceKey::CommandCode);
        assert_eq!(result.state, DetectionState::Available);
        assert!(result.usage_artifacts_found);
        assert!(result.issues.is_empty());
    }

    #[test]
    fn detects_available_no_data_with_legacy_only_transcripts() {
        let temp = TempDir::new().expect("temp dir");
        let commandcode_home = temp.path().join("commandcode-home");
        write_transcript(
            &commandcode_home.join("projects").join("proj-legacy"),
            "sess-legacy.jsonl",
            r#"{"id":"legacy-1","timestamp":"2026-05-07T03:23:01Z","sessionId":"sess-legacy","parentId":null,"role":"user","content":[{"type":"text","text":"hi"}]}"#,
        );

        let collector = CommandCodeCollector::from_data_dir(commandcode_home);
        let result = collector
            .detect(detection_request(SourceKey::CommandCode), &NeverCancelled)
            .expect("detect");

        assert_eq!(result.source, SourceKey::CommandCode);
        assert_eq!(result.state, DetectionState::AvailableNoData);
        assert!(!result.usage_artifacts_found);
        assert_eq!(result.issues[0].code, ISSUE_LEGACY_ONLY_TRANSCRIPTS);
    }

    #[test]
    fn detects_not_found_when_home_missing() {
        let temp = TempDir::new().expect("temp dir");
        let commandcode_home = temp.path().join("missing-home");

        let collector = CommandCodeCollector::from_data_dir(commandcode_home);
        let result = collector
            .detect(detection_request(SourceKey::CommandCode), &NeverCancelled)
            .expect("detect");

        assert_eq!(result.state, DetectionState::NotFound);
        assert_eq!(result.issues[0].code, ISSUE_HOME_MISSING);
    }

    #[test]
    fn rejects_non_commandcode_source_in_detection() {
        let temp = TempDir::new().expect("temp dir");
        let collector = CommandCodeCollector::from_data_dir(temp.path().join("home"));

        let result = collector
            .detect(detection_request(SourceKey::Cline), &NeverCancelled)
            .expect("detect");

        assert_eq!(result.state, DetectionState::Unsupported);
        assert_eq!(result.issues[0].code, "commandcode.unsupported_source");
    }

    #[test]
    fn collect_fails_closed_until_native_collector_is_wired() {
        let temp = TempDir::new().expect("temp dir");
        let collector = CommandCodeCollector::from_data_dir(temp.path().join("home"));

        let failure = collector
            .collect(collection_request(SourceKey::CommandCode), &NeverCancelled)
            .expect_err("collect fails closed");

        assert_eq!(failure.code, CollectorFailureCode::UnsupportedSource);
        assert_eq!(failure.source_key, Some(SourceKey::CommandCode));
    }
}
