use std::path::Path;

use crate::application::collection::{
    CollectionRequest, CollectorFailure, CollectorFailureCode, ResultValidationError,
};
use crate::domain::source::SourceKey;

pub(in crate::infrastructure::collectors) fn validate_source(
    request: &CollectionRequest,
    expected: SourceKey,
) -> Result<(), CollectorFailure> {
    if request.source() == expected {
        Ok(())
    } else {
        Err(request_failure(
            request,
            CollectorFailureCode::UnsupportedSource,
        ))
    }
}

pub(in crate::infrastructure::collectors) fn request_failure(
    request: &CollectionRequest,
    code: CollectorFailureCode,
) -> CollectorFailure {
    CollectorFailure::new(code, Some(request.source()), Some(request.projection()))
}

pub(in crate::infrastructure::collectors) fn missing_or_invalid_location_code(
    path: &Path,
) -> CollectorFailureCode {
    if path.exists() {
        CollectorFailureCode::SourceInvalidLocation
    } else {
        CollectorFailureCode::SourceNotFound
    }
}

pub(in crate::infrastructure::collectors) fn validation_failure_as_internal(
    request: &CollectionRequest,
    _error: ResultValidationError,
) -> CollectorFailure {
    request_failure(request, CollectorFailureCode::Internal)
}

pub(in crate::infrastructure::collectors) fn validation_failure_preserving_all_rejected(
    request: &CollectionRequest,
    error: ResultValidationError,
) -> CollectorFailure {
    let code = if error == ResultValidationError::AllRecordsRejected {
        CollectorFailureCode::AllRecordsRejected
    } else {
        CollectorFailureCode::Internal
    };
    request_failure(request, code)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use chrono::{NaiveDate, Utc};

    use super::*;
    use crate::application::collection::{CollectionId, CollectionProjection, CollectionScope};

    #[test]
    fn validates_expected_source() {
        let request = request(SourceKey::ZCode);

        validate_source(&request, SourceKey::ZCode).expect("expected source");

        let error = validate_source(&request, SourceKey::Cline).expect_err("wrong source");
        assert_eq!(error.code, CollectorFailureCode::UnsupportedSource);
        assert_eq!(error.source_key, Some(SourceKey::ZCode));
        assert_eq!(error.projection, Some(CollectionProjection::Daily));
    }

    #[test]
    fn classifies_missing_and_invalid_locations() {
        assert_eq!(
            missing_or_invalid_location_code(Path::new("/definitely/missing/burnly/source.db")),
            CollectorFailureCode::SourceNotFound
        );
        assert_eq!(
            missing_or_invalid_location_code(Path::new(".")),
            CollectorFailureCode::SourceInvalidLocation
        );
    }

    #[test]
    fn maps_validation_errors_by_collector_policy() {
        let request = request(SourceKey::Cline);

        assert_eq!(
            validation_failure_as_internal(&request, ResultValidationError::AllRecordsRejected)
                .code,
            CollectorFailureCode::Internal
        );
        assert_eq!(
            validation_failure_preserving_all_rejected(
                &request,
                ResultValidationError::AllRecordsRejected,
            )
            .code,
            CollectorFailureCode::AllRecordsRejected
        );
        assert_eq!(
            validation_failure_preserving_all_rejected(
                &request,
                ResultValidationError::TooManyWarnings,
            )
            .code,
            CollectorFailureCode::Internal
        );
    }

    fn request(source: SourceKey) -> CollectionRequest {
        CollectionRequest::daily(
            CollectionId::new("collection-1").expect("collection id"),
            source,
            CollectionScope::incremental(
                NaiveDate::from_ymd_opt(2026, 7, 4).expect("date"),
                NaiveDate::from_ymd_opt(2026, 7, 4).expect("date"),
            )
            .expect("scope"),
            "UTC",
            Utc::now(),
        )
        .expect("request")
    }
}
