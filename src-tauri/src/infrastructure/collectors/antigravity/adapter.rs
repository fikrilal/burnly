use std::time::Instant;

use chrono::{DateTime, Utc};

use crate::application::collection::{
    CollectionMetadata, CollectionPeriod, CollectionProjection, CollectionRequest,
    CollectionResult, CollectorDescriptor, CollectorFailure, CollectorFailureCode,
    CollectorIntegrity, CollectorKey, DetectionIssue, DetectionRequest, DetectionResult,
    DetectionState, ProcessSummary, ProfileDescriptor,
};
use crate::application::ports::collector::{CancellationSignal, Collector};
use crate::domain::source::SourceKey;

const COLLECTOR_KEY: &str = "antigravity";
const DISPLAY_NAME: &str = "Antigravity";
const COLLECTOR_VERSION: &str = "local-rpc";
const ADAPTER_VERSION: u16 = 1;
const PROFILE_VERSION: u16 = 1;

#[derive(Debug, Clone, Default)]
pub(crate) struct AntigravityCollector;

impl AntigravityCollector {
    pub(crate) const fn new() -> Self {
        Self
    }
}

impl Collector for AntigravityCollector {
    fn describe(&self) -> Result<CollectorDescriptor, CollectorFailure> {
        descriptor()
    }

    fn detect(
        &self,
        request: DetectionRequest,
        cancellation: &dyn CancellationSignal,
    ) -> Result<DetectionResult, CollectorFailure> {
        if cancellation.is_cancelled() {
            return Ok(DetectionResult {
                source: request.source,
                state: DetectionState::Cancelled,
                supported_projections: Vec::new(),
                data_roots_found: 0,
                usage_artifacts_found: false,
                checked_at: request.requested_at,
                issues: Vec::new(),
            });
        }
        if request.source != SourceKey::Antigravity {
            return Ok(DetectionResult {
                source: request.source,
                state: DetectionState::Unsupported,
                supported_projections: Vec::new(),
                data_roots_found: 0,
                usage_artifacts_found: false,
                checked_at: request.requested_at,
                issues: vec![issue(
                    "antigravity.unsupported_source",
                    "Source is not Antigravity.",
                )],
            });
        }

        Ok(DetectionResult {
            source: SourceKey::Antigravity,
            state: DetectionState::NotFound,
            supported_projections: supported_projections(),
            data_roots_found: 0,
            usage_artifacts_found: false,
            checked_at: request.requested_at,
            issues: vec![issue(
                "antigravity.runtime_discovery_pending",
                "Antigravity runtime discovery is not implemented yet.",
            )],
        })
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
            return Err(failure(&request, CollectorFailureCode::Cancelled));
        }

        empty_result(&request, started, started_at)
    }
}

fn empty_result(
    request: &CollectionRequest,
    started: Instant,
    started_at: DateTime<Utc>,
) -> Result<CollectionResult, CollectorFailure> {
    let finished_at = Utc::now();
    let metadata = CollectionMetadata::new(
        request.collection_id().clone(),
        collector_key()?,
        COLLECTOR_VERSION.to_owned(),
        SourceKey::Antigravity,
        request.scope().clone(),
        PROFILE_VERSION,
        CollectionPeriod {
            started_at,
            finished_at,
        },
    )
    .map_err(|_| failure(request, CollectorFailureCode::Internal))?;
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
    .map_err(|_| failure(request, CollectorFailureCode::Internal))
}

fn validate_request(request: &CollectionRequest) -> Result<(), CollectorFailure> {
    if request.source() != SourceKey::Antigravity {
        return Err(failure(request, CollectorFailureCode::UnsupportedSource));
    }
    Ok(())
}

fn descriptor() -> Result<CollectorDescriptor, CollectorFailure> {
    Ok(CollectorDescriptor {
        collector: collector_key()?,
        display_name: DISPLAY_NAME.to_owned(),
        runtime_version: COLLECTOR_VERSION.to_owned(),
        expected_version: COLLECTOR_VERSION.to_owned(),
        adapter_version: ADAPTER_VERSION,
        binary_target: std::env::consts::OS.to_owned(),
        integrity: CollectorIntegrity::UnverifiedDevelopment,
        profiles: vec![ProfileDescriptor {
            source: SourceKey::Antigravity,
            profile_version: PROFILE_VERSION,
            supported_projections: supported_projections(),
        }],
    })
}

fn supported_projections() -> Vec<CollectionProjection> {
    vec![CollectionProjection::Daily, CollectionProjection::Session]
}

fn collector_key() -> Result<CollectorKey, CollectorFailure> {
    CollectorKey::new(COLLECTOR_KEY)
        .map_err(|_| CollectorFailure::new(CollectorFailureCode::Internal, None, None))
}

fn failure(request: &CollectionRequest, code: CollectorFailureCode) -> CollectorFailure {
    CollectorFailure::new(code, Some(request.source()), Some(request.projection()))
}

fn issue(code: &str, message: &str) -> DetectionIssue {
    DetectionIssue {
        code: code.to_owned(),
        message: message.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;
    use crate::application::collection::{
        CollectionId, CollectionOutcome, CollectionScope, DetectionReason,
    };

    #[test]
    fn describes_antigravity_profile() {
        let collector = AntigravityCollector::new();

        let descriptor = collector.describe().expect("descriptor");

        assert_eq!(descriptor.collector.as_str(), "antigravity");
        assert_eq!(descriptor.display_name, "Antigravity");
        assert_eq!(descriptor.profiles[0].source, SourceKey::Antigravity);
        assert_eq!(
            descriptor.profiles[0].supported_projections,
            vec![CollectionProjection::Daily, CollectionProjection::Session]
        );
    }

    #[test]
    fn detects_pending_runtime_discovery_as_not_found() {
        let collector = AntigravityCollector::new();

        let result = collector
            .detect(detection_request(SourceKey::Antigravity), &NeverCancelled)
            .expect("detection");

        assert_eq!(result.state, DetectionState::NotFound);
        assert_eq!(result.supported_projections, supported_projections());
        assert!(!result.usage_artifacts_found);
        assert_eq!(
            result.issues[0].code,
            "antigravity.runtime_discovery_pending"
        );
    }

    #[test]
    fn rejects_other_sources() {
        let collector = AntigravityCollector::new();

        let error = collector
            .collect(daily_request(SourceKey::Cline), &NeverCancelled)
            .expect_err("unsupported source");

        assert_eq!(error.code, CollectorFailureCode::UnsupportedSource);
    }

    #[test]
    fn returns_empty_daily_result_until_runtime_client_exists() {
        let collector = AntigravityCollector::new();

        let result = collector
            .collect(daily_request(SourceKey::Antigravity), &NeverCancelled)
            .expect("collection");

        assert_eq!(result.outcome(), CollectionOutcome::Empty);
        assert_eq!(result.metadata().collector().as_str(), "antigravity");
    }

    #[test]
    fn returns_empty_session_result_until_runtime_client_exists() {
        let collector = AntigravityCollector::new();

        let result = collector
            .collect(session_request(SourceKey::Antigravity), &NeverCancelled)
            .expect("collection");

        assert_eq!(result.outcome(), CollectionOutcome::Empty);
        assert_eq!(result.metadata().collector().as_str(), "antigravity");
    }

    struct NeverCancelled;

    impl CancellationSignal for NeverCancelled {
        fn is_cancelled(&self) -> bool {
            false
        }
    }

    fn detection_request(source: SourceKey) -> DetectionRequest {
        DetectionRequest {
            source,
            reason: DetectionReason::Startup,
            requested_at: Utc
                .with_ymd_and_hms(2026, 7, 2, 8, 0, 0)
                .single()
                .expect("timestamp"),
        }
    }

    fn daily_request(source: SourceKey) -> CollectionRequest {
        CollectionRequest::daily(
            CollectionId::new(format!("{}-daily", source.as_str())).expect("collection id"),
            source,
            CollectionScope::Full,
            "UTC",
            Utc.with_ymd_and_hms(2026, 7, 2, 8, 0, 0)
                .single()
                .expect("timestamp"),
        )
        .expect("request")
    }

    fn session_request(source: SourceKey) -> CollectionRequest {
        CollectionRequest::session(
            CollectionId::new(format!("{}-session", source.as_str())).expect("collection id"),
            source,
            CollectionScope::Full,
            Utc.with_ymd_and_hms(2026, 7, 2, 8, 0, 0)
                .single()
                .expect("timestamp"),
        )
    }
}
