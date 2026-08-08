//! Command Code collector adapter.
//!
//! Wires the transcript reader, parser, and mapper into the collector port.
//! Collection reads `~/.commandcode/projects/**/<session>.jsonl` transcripts
//! read-only and maps usage-bearing messages into Burnly daily/session
//! candidates. No durable cache: transcripts are re-read per refresh with
//! `(session id, message id)` dedupe in the mapper.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::Utc;

use crate::application::collection::{
    CollectionProjection, CollectionRequest, CollectionResult, CollectorDescriptor,
    CollectorFailure, CollectorFailureCode, CollectorIntegrity, DetectionIssue, DetectionRequest,
    DetectionResult,
};
use crate::application::diagnostics::DiagnosticSeverity;
use crate::application::ports::collector::{CancellationSignal, Collector};
use crate::application::ports::diagnostic_recorder::DiagnosticRecorder;
use crate::domain::source::SourceKey;
use crate::infrastructure::collectors::support::{
    available_detection, cancelled_detection, collection_metadata, collector_key,
    daily_session_projections, detection_issue, empty_collection_result, not_found_detection,
    record_collector_diagnostic, request_failure, single_source_descriptor, unsupported_detection,
    validate_source, validation_failure_as_internal, CollectorDiagnosticCounter, CollectorIdentity,
    LocalCollectionRun,
};

use super::detection::{inspect_commandcode_home, CommandCodeHomeInspection};
use super::mapper::{self, CommandCodeMappingContext};
use super::transcript_reader::TranscriptReader;

const COLLECTOR_KEY: &str = "command-code";
const DISPLAY_NAME: &str = "Command Code";
const COLLECTOR_VERSION: &str = "local";
const ADAPTER_VERSION: u16 = 1;
const PROFILE_VERSION: u16 = 1;
const IDENTITY: CollectorIdentity = CollectorIdentity {
    key: COLLECTOR_KEY,
    display_name: DISPLAY_NAME,
    runtime_version: COLLECTOR_VERSION,
    adapter_version: ADAPTER_VERSION,
    source: SourceKey::CommandCode,
    profile_version: PROFILE_VERSION,
};

/// Issue codes emitted by Command Code detection.
pub(crate) const ISSUE_HOME_MISSING: &str = "commandcode.home_missing";
pub(crate) const ISSUE_PROJECTS_MISSING: &str = "commandcode.projects_missing";
pub(crate) const ISSUE_PROJECTS_UNREADABLE: &str = "commandcode.projects_unreadable";
pub(crate) const ISSUE_NO_USAGE_TRANSCRIPTS: &str = "commandcode.no_usage_transcripts";
pub(crate) const ISSUE_LEGACY_ONLY_TRANSCRIPTS: &str = "commandcode.legacy_only_transcripts";

#[derive(Clone)]
pub(crate) struct CommandCodeCollector {
    commandcode_home: PathBuf,
    diagnostics: Option<Arc<dyn DiagnosticRecorder>>,
}

impl CommandCodeCollector {
    pub(crate) fn from_data_dir(commandcode_home: PathBuf) -> Self {
        Self {
            commandcode_home,
            diagnostics: None,
        }
    }

    pub(crate) fn with_diagnostic_recorder(
        mut self,
        diagnostics: Arc<dyn DiagnosticRecorder>,
    ) -> Self {
        self.diagnostics = Some(diagnostics);
        self
    }

    fn inspect(&self, override_path: Option<&Path>) -> CommandCodeHomeInspection {
        inspect_commandcode_home(override_path)
    }

    fn inspect_stored_home(&self) -> CommandCodeHomeInspection {
        self.inspect(Some(&self.commandcode_home))
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
        let issues = self.detection_issues(&inspection);

        if issues.is_empty() {
            return Ok(available_detection(
                &request,
                SourceKey::CommandCode,
                supported_projections(),
                true,
            ));
        }

        let first_issue = issues[0].clone();
        // The data root exists but holds no usable usage data yet; the source
        // is installed, just not collecting.
        if first_issue.code == ISSUE_LEGACY_ONLY_TRANSCRIPTS
            || first_issue.code == ISSUE_NO_USAGE_TRANSCRIPTS
        {
            let mut result = available_detection(
                &request,
                SourceKey::CommandCode,
                supported_projections(),
                false,
            );
            result.issues.push(first_issue);
            return Ok(result);
        }
        // The data root itself is absent or unreadable.
        Ok(not_found_detection(
            &request,
            SourceKey::CommandCode,
            supported_projections(),
            first_issue,
        ))
    }

    fn collect(
        &self,
        request: CollectionRequest,
        cancellation: &dyn CancellationSignal,
    ) -> Result<CollectionResult, CollectorFailure> {
        let run = LocalCollectionRun::start();
        validate_source(&request, SourceKey::CommandCode)?;
        if cancellation.is_cancelled() {
            return Err(request_failure(&request, CollectorFailureCode::Cancelled));
        }
        if !self.commandcode_home.is_dir() {
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

        let (_, parsed, _) = TranscriptReader::scan(&self.commandcode_home);
        let rows_found = u64::try_from(parsed.len()).unwrap_or(u64::MAX);

        let finished_at = Utc::now();
        let metadata = collection_metadata(IDENTITY, &request, run.started_at(), finished_at)?;
        let context = CommandCodeMappingContext::new(
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
                let candidates = mapper::map_daily(parsed, timezone, request.scope(), &context)
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
                let candidates = mapper::map_sessions(parsed, &context).map_err(|_| {
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

impl CommandCodeCollector {
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
            "commandcode.collection_failed",
            "Command Code collection failed.",
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
    use std::sync::Arc;

    use chrono::{DateTime, Utc};
    use tempfile::TempDir;

    use super::*;
    use crate::application::collection::{
        CollectionId, CollectionOutcome, CollectionScope, DetectionState,
    };
    use crate::application::diagnostics::DiagnosticSeverity;
    use crate::infrastructure::collectors::support::{
        daily_request as support_daily_request, detection_request, fixed_timestamp,
        session_request as support_session_request, NeverCancelled, RecordingDiagnostics,
    };

    const VALID_TRANSCRIPT: &str = r#"{"type":"session","version":3,"id":"sess-1","timestamp":"2026-08-04T10:00:00Z","cwd":"/tmp/proj"}
{"type":"message","id":"m1","parentId":null,"timestamp":"2026-08-04T10:00:01Z","message":{"role":"user","content":[{"type":"text","text":"redacted"}]}}
{"type":"message","id":"m2","parentId":"m1","timestamp":"2026-08-04T10:00:02Z","message":{"role":"assistant","content":[{"type":"text","text":"redacted"}]},"usage":{"inputTokens":10,"outputTokens":2,"cacheReadTokens":3,"cacheWriteTokens":0,"costUsd":0.001},"model":"deepseek/deepseek-v4-flash","effort":"max"}"#;

    #[test]
    fn describes_command_code_profile() {
        let collector = CommandCodeCollector::from_data_dir(PathBuf::from("/missing"));

        let descriptor = collector.describe().expect("descriptor");

        assert_eq!(descriptor.display_name, "Command Code");
        assert_eq!(descriptor.profiles[0].source, SourceKey::CommandCode);
        assert_eq!(
            descriptor.profiles[0].supported_projections,
            vec![CollectionProjection::Daily, CollectionProjection::Session]
        );
    }

    #[test]
    fn detects_available_with_valid_transcript() {
        let fixture = FixtureCommandCode::new().with_valid_transcript();
        let collector = CommandCodeCollector::from_data_dir(fixture.commandcode_home());

        let result = collector
            .detect(
                detection_request(SourceKey::CommandCode, timestamp()),
                &NeverCancelled,
            )
            .expect("detection");

        assert_eq!(result.state, DetectionState::Available);
        assert!(result.usage_artifacts_found);
    }

    #[test]
    fn detects_available_no_data_with_legacy_only_transcripts() {
        let fixture = FixtureCommandCode::new().with_legacy_transcript();
        let collector = CommandCodeCollector::from_data_dir(fixture.commandcode_home());

        let result = collector
            .detect(
                detection_request(SourceKey::CommandCode, timestamp()),
                &NeverCancelled,
            )
            .expect("detection");

        assert_eq!(result.state, DetectionState::AvailableNoData);
        assert!(!result.usage_artifacts_found);
        assert_eq!(result.issues[0].code, ISSUE_LEGACY_ONLY_TRANSCRIPTS);
    }

    #[test]
    fn detects_not_found_when_home_missing() {
        let collector = CommandCodeCollector::from_data_dir(PathBuf::from("/missing/commandcode"));

        let result = collector
            .detect(
                detection_request(SourceKey::CommandCode, timestamp()),
                &NeverCancelled,
            )
            .expect("detection");

        assert_eq!(result.state, DetectionState::NotFound);
        assert_eq!(result.issues[0].code, ISSUE_HOME_MISSING);
    }

    #[test]
    fn rejects_non_commandcode_source_in_detection() {
        let fixture = FixtureCommandCode::new().with_valid_transcript();
        let collector = CommandCodeCollector::from_data_dir(fixture.commandcode_home());

        let result = collector
            .detect(
                detection_request(SourceKey::Cline, timestamp()),
                &NeverCancelled,
            )
            .expect("detection");

        assert_eq!(result.state, DetectionState::Unsupported);
        assert_eq!(result.issues[0].code, "commandcode.unsupported_source");
    }

    #[test]
    fn records_diagnostic_when_commandcode_home_is_missing() {
        let diagnostics = Arc::new(RecordingDiagnostics::default());
        let collector = CommandCodeCollector::from_data_dir(PathBuf::from("/missing/commandcode"))
            .with_diagnostic_recorder(diagnostics.clone());

        let result = collector
            .collect(daily_request(CollectionScope::Full), &NeverCancelled)
            .expect("missing home is empty");

        assert_eq!(result.outcome(), CollectionOutcome::Empty);
        let events = diagnostics.events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].severity, DiagnosticSeverity::Warning);
        assert_eq!(events[0].code.as_str(), "commandcode.collection_failed");
        let context = events[0].context.as_ref().expect("context").as_str();
        assert!(context.contains(r#""source":"command-code""#));
        assert!(context.contains(r#""projection":"daily""#));
        assert!(context.contains(r#""failureCode":"source.not_found""#));
        assert!(context.contains(r#""rowsFound":0"#));
        assert!(!context.contains("/missing"));
    }

    #[test]
    fn rejects_non_commandcode_collection_request() {
        let fixture = FixtureCommandCode::new().with_valid_transcript();
        let collector = CommandCodeCollector::from_data_dir(fixture.commandcode_home());

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
    fn collects_daily_usage_from_transcripts() {
        let fixture = FixtureCommandCode::new().with_valid_transcript();
        let collector = CommandCodeCollector::from_data_dir(fixture.commandcode_home());

        let result = collector
            .collect(daily_request(CollectionScope::Full), &NeverCancelled)
            .expect("daily collection");

        assert_eq!(result.outcome(), CollectionOutcome::Complete);
        assert_eq!(result.daily_candidates().len(), 1);
        let daily = &result.daily_candidates()[0];
        assert_eq!(
            daily.source_key,
            "command-code:daily:v1:Asia/Jakarta:2026-08-04"
        );
        assert_eq!(daily.tokens.input_tokens(), Some(7));
        assert_eq!(daily.tokens.output_tokens(), Some(2));
        assert_eq!(daily.tokens.cache_read_tokens(), Some(3));
        assert_eq!(daily.tokens.total_tokens(), 12);
        assert_eq!(daily.model_breakdowns.len(), 1);
        assert_eq!(
            daily.model_breakdowns[0].raw_model_id,
            "deepseek/deepseek-v4-flash"
        );
    }

    #[test]
    fn collects_session_usage_from_transcripts() {
        let fixture = FixtureCommandCode::new().with_valid_transcript();
        let collector = CommandCodeCollector::from_data_dir(fixture.commandcode_home());

        let result = collector
            .collect(session_request(), &NeverCancelled)
            .expect("session collection");

        assert_eq!(result.outcome(), CollectionOutcome::Complete);
        assert_eq!(result.session_candidates().len(), 1);
        let session = &result.session_candidates()[0];
        assert!(session
            .source_key
            .starts_with("command-code:session:v1:sess-1:"));
        assert_eq!(session.source_session_id, "sess-1");
        assert_eq!(session.project_path.as_deref(), Some("/tmp/proj"));
        assert_eq!(
            session.first_activity_at,
            Some(fixed_timestamp(2026, 8, 4, 10, 0, 2))
        );
        assert_eq!(
            session.last_activity_at,
            Some(fixed_timestamp(2026, 8, 4, 10, 0, 2))
        );
    }

    struct FixtureCommandCode {
        workspace: TempDir,
    }

    impl FixtureCommandCode {
        fn new() -> Self {
            Self {
                workspace: TempDir::new().expect("workspace"),
            }
        }

        fn commandcode_home(&self) -> PathBuf {
            self.workspace.path().to_path_buf()
        }

        fn with_valid_transcript(self) -> Self {
            fs::create_dir_all(self.commandcode_home().join("projects").join("proj-a"))
                .expect("projects dir");
            fs::write(
                self.commandcode_home()
                    .join("projects")
                    .join("proj-a")
                    .join("sess-1.jsonl"),
                VALID_TRANSCRIPT,
            )
            .expect("transcript");
            self
        }

        fn with_legacy_transcript(self) -> Self {
            fs::create_dir_all(self.commandcode_home().join("projects").join("proj-legacy"))
                .expect("projects dir");
            fs::write(
                self.commandcode_home()
                    .join("projects")
                    .join("proj-legacy")
                    .join("sess-legacy.jsonl"),
                r#"{"id":"legacy-1","timestamp":"2026-05-07T03:23:01Z","sessionId":"sess-legacy","parentId":null,"role":"user","content":[{"type":"text","text":"redacted"}]}"#,
            )
            .expect("transcript");
            self
        }
    }

    fn daily_request(scope: CollectionScope) -> CollectionRequest {
        support_daily_request(
            "command-code-daily",
            SourceKey::CommandCode,
            scope,
            "Asia/Jakarta",
            timestamp(),
        )
    }

    fn session_request() -> CollectionRequest {
        support_session_request("command-code-session", SourceKey::CommandCode, timestamp())
    }

    fn timestamp() -> DateTime<Utc> {
        fixed_timestamp(2026, 8, 4, 12, 0, 0)
    }
}
