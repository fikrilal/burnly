use crate::application::collection::{
    CollectionProjection, DetectionIssue, DetectionRequest, DetectionResult, DetectionState,
};
use crate::domain::source::SourceKey;

pub(in crate::infrastructure::collectors) fn detection_issue(
    code: &str,
    message: &str,
) -> DetectionIssue {
    DetectionIssue {
        code: code.to_owned(),
        message: message.to_owned(),
    }
}

pub(in crate::infrastructure::collectors) fn cancelled_detection(
    request: &DetectionRequest,
) -> DetectionResult {
    DetectionResult {
        source: request.source,
        state: DetectionState::Cancelled,
        supported_projections: Vec::new(),
        data_roots_found: 0,
        usage_artifacts_found: false,
        checked_at: request.requested_at,
        issues: Vec::new(),
    }
}

pub(in crate::infrastructure::collectors) fn unsupported_detection(
    request: &DetectionRequest,
    issue: DetectionIssue,
) -> DetectionResult {
    DetectionResult {
        source: request.source,
        state: DetectionState::Unsupported,
        supported_projections: Vec::new(),
        data_roots_found: 0,
        usage_artifacts_found: false,
        checked_at: request.requested_at,
        issues: vec![issue],
    }
}

pub(in crate::infrastructure::collectors) fn not_found_detection(
    request: &DetectionRequest,
    source: SourceKey,
    supported_projections: Vec<CollectionProjection>,
    issue: DetectionIssue,
) -> DetectionResult {
    DetectionResult {
        source,
        state: DetectionState::NotFound,
        supported_projections,
        data_roots_found: 0,
        usage_artifacts_found: false,
        checked_at: request.requested_at,
        issues: vec![issue],
    }
}

pub(in crate::infrastructure::collectors) fn available_detection(
    request: &DetectionRequest,
    source: SourceKey,
    supported_projections: Vec<CollectionProjection>,
    usage_artifacts_found: bool,
) -> DetectionResult {
    DetectionResult {
        source,
        state: if usage_artifacts_found {
            DetectionState::Available
        } else {
            DetectionState::AvailableNoData
        },
        supported_projections,
        data_roots_found: 1,
        usage_artifacts_found,
        checked_at: request.requested_at,
        issues: Vec::new(),
    }
}

pub(in crate::infrastructure::collectors) fn invalid_configuration_detection(
    request: &DetectionRequest,
    source: SourceKey,
    supported_projections: Vec<CollectionProjection>,
    issue: DetectionIssue,
) -> DetectionResult {
    DetectionResult {
        source,
        state: DetectionState::InvalidConfiguration,
        supported_projections,
        data_roots_found: 1,
        usage_artifacts_found: false,
        checked_at: request.requested_at,
        issues: vec![issue],
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;
    use crate::application::collection::DetectionReason;

    #[test]
    fn builds_cancelled_detection_without_projection_support() {
        let request = request(SourceKey::Cline);

        let result = cancelled_detection(&request);

        assert_eq!(result.source, SourceKey::Cline);
        assert_eq!(result.state, DetectionState::Cancelled);
        assert!(result.supported_projections.is_empty());
        assert_eq!(result.data_roots_found, 0);
        assert!(!result.usage_artifacts_found);
        assert_eq!(result.checked_at, request.requested_at);
        assert!(result.issues.is_empty());
    }

    #[test]
    fn builds_unsupported_detection_with_issue() {
        let request = request(SourceKey::ZCode);

        let result = unsupported_detection(
            &request,
            detection_issue("zcode.unsupported_source", "Source is not ZCode."),
        );

        assert_eq!(result.source, SourceKey::ZCode);
        assert_eq!(result.state, DetectionState::Unsupported);
        assert!(result.supported_projections.is_empty());
        assert_eq!(result.issues[0].code, "zcode.unsupported_source");
    }

    #[test]
    fn builds_not_found_detection_with_supported_projections() {
        let request = request(SourceKey::Cline);

        let result = not_found_detection(
            &request,
            SourceKey::Cline,
            vec![CollectionProjection::Daily],
            detection_issue("cline.database_missing", "missing"),
        );

        assert_eq!(result.source, SourceKey::Cline);
        assert_eq!(result.state, DetectionState::NotFound);
        assert_eq!(
            result.supported_projections,
            vec![CollectionProjection::Daily]
        );
        assert_eq!(result.data_roots_found, 0);
        assert!(!result.usage_artifacts_found);
        assert_eq!(result.issues[0].message, "missing");
    }

    #[test]
    fn builds_available_and_available_no_data_detection() {
        let request = request(SourceKey::ZCode);

        let available = available_detection(
            &request,
            SourceKey::ZCode,
            vec![CollectionProjection::Daily, CollectionProjection::Session],
            true,
        );
        let no_data = available_detection(
            &request,
            SourceKey::ZCode,
            vec![CollectionProjection::Daily],
            false,
        );

        assert_eq!(available.state, DetectionState::Available);
        assert!(available.usage_artifacts_found);
        assert_eq!(available.data_roots_found, 1);
        assert_eq!(no_data.state, DetectionState::AvailableNoData);
        assert!(!no_data.usage_artifacts_found);
        assert_eq!(no_data.data_roots_found, 1);
    }

    #[test]
    fn builds_invalid_configuration_detection() {
        let request = request(SourceKey::Cline);

        let result = invalid_configuration_detection(
            &request,
            SourceKey::Cline,
            vec![CollectionProjection::Daily],
            detection_issue("cline.database_incompatible", "incompatible"),
        );

        assert_eq!(result.source, SourceKey::Cline);
        assert_eq!(result.state, DetectionState::InvalidConfiguration);
        assert_eq!(
            result.supported_projections,
            vec![CollectionProjection::Daily]
        );
        assert_eq!(result.data_roots_found, 1);
        assert!(!result.usage_artifacts_found);
        assert_eq!(result.issues[0].code, "cline.database_incompatible");
    }

    fn request(source: SourceKey) -> DetectionRequest {
        DetectionRequest {
            source,
            reason: DetectionReason::Startup,
            requested_at: chrono::Utc
                .with_ymd_and_hms(2026, 7, 4, 1, 2, 3)
                .single()
                .expect("timestamp"),
        }
    }
}
