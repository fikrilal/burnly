use thiserror::Error;

use crate::domain::source::SourceKey;

use super::CollectionProjection;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CollectorFailureCategory {
    Configuration,
    Binary,
    Detection,
    Permission,
    Execution,
    Timeout,
    Cancelled,
    OutputLimit,
    IncompatibleOutput,
    Validation,
    Unsupported,
    Internal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CollectorFailureCode {
    BinaryMissing,
    BinaryChecksumMismatch,
    VersionMismatch,
    SpawnFailed,
    TimedOut,
    Cancelled,
    StdoutLimitExceeded,
    StderrLimitExceeded,
    NonUtf8Output,
    NonzeroExit,
    InvalidJson,
    IncompatibleEnvelope,
    UnsupportedSource,
    UnsupportedProjection,
    ScopeNotRepresentable,
    SourceNotFound,
    SourcePermissionDenied,
    SourceInvalidLocation,
    AllRecordsRejected,
    Internal,
}

impl CollectorFailureCode {
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::BinaryMissing => "collector.binary_missing",
            Self::BinaryChecksumMismatch => "collector.binary_checksum_mismatch",
            Self::VersionMismatch => "collector.version_mismatch",
            Self::SpawnFailed => "collector.spawn_failed",
            Self::TimedOut => "collector.timed_out",
            Self::Cancelled => "collector.cancelled",
            Self::StdoutLimitExceeded => "collector.stdout_limit_exceeded",
            Self::StderrLimitExceeded => "collector.stderr_limit_exceeded",
            Self::NonUtf8Output => "collector.non_utf8_output",
            Self::NonzeroExit => "collector.nonzero_exit",
            Self::InvalidJson => "collector.invalid_json",
            Self::IncompatibleEnvelope => "collector.incompatible_envelope",
            Self::UnsupportedSource => "collector.unsupported_source",
            Self::UnsupportedProjection => "collector.unsupported_projection",
            Self::ScopeNotRepresentable => "collector.scope_not_representable",
            Self::SourceNotFound => "source.not_found",
            Self::SourcePermissionDenied => "source.permission_denied",
            Self::SourceInvalidLocation => "source.invalid_location",
            Self::AllRecordsRejected => "collection.all_records_rejected",
            Self::Internal => "collector.internal",
        }
    }

    pub(crate) const fn category(self) -> CollectorFailureCategory {
        match self {
            Self::BinaryMissing | Self::BinaryChecksumMismatch | Self::VersionMismatch => {
                CollectorFailureCategory::Binary
            }
            Self::SpawnFailed | Self::NonUtf8Output | Self::NonzeroExit => {
                CollectorFailureCategory::Execution
            }
            Self::TimedOut => CollectorFailureCategory::Timeout,
            Self::Cancelled => CollectorFailureCategory::Cancelled,
            Self::StdoutLimitExceeded | Self::StderrLimitExceeded => {
                CollectorFailureCategory::OutputLimit
            }
            Self::InvalidJson | Self::IncompatibleEnvelope => {
                CollectorFailureCategory::IncompatibleOutput
            }
            Self::UnsupportedSource | Self::UnsupportedProjection | Self::ScopeNotRepresentable => {
                CollectorFailureCategory::Unsupported
            }
            Self::SourceNotFound => CollectorFailureCategory::Detection,
            Self::SourcePermissionDenied => CollectorFailureCategory::Permission,
            Self::SourceInvalidLocation => CollectorFailureCategory::Configuration,
            Self::AllRecordsRejected => CollectorFailureCategory::Validation,
            Self::Internal => CollectorFailureCategory::Internal,
        }
    }

    pub(crate) const fn retryable(self) -> bool {
        matches!(
            self,
            Self::SpawnFailed | Self::TimedOut | Self::Cancelled | Self::SourcePermissionDenied
        )
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("{message}")]
pub(crate) struct CollectorFailure {
    pub code: CollectorFailureCode,
    pub source_key: Option<SourceKey>,
    pub projection: Option<CollectionProjection>,
    pub context: CollectorFailureContext,
    message: &'static str,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct CollectorFailureContext {
    pub runtime_ms: Option<u64>,
    pub stdout_bytes: Option<u64>,
    pub stderr_bytes: Option<u64>,
    pub exit_code: Option<i32>,
}

impl CollectorFailure {
    pub(crate) fn new(
        code: CollectorFailureCode,
        source_key: Option<SourceKey>,
        projection: Option<CollectionProjection>,
    ) -> Self {
        Self {
            code,
            source_key,
            projection,
            context: CollectorFailureContext::default(),
            message: safe_message(code),
        }
    }

    pub(crate) fn with_context(mut self, context: CollectorFailureContext) -> Self {
        self.context = context;
        self
    }

    pub(crate) const fn category(&self) -> CollectorFailureCategory {
        self.code.category()
    }

    pub(crate) const fn retryable(&self) -> bool {
        self.code.retryable()
    }
}

const fn safe_message(code: CollectorFailureCode) -> &'static str {
    match code {
        CollectorFailureCode::BinaryMissing => "The collector binary is unavailable.",
        CollectorFailureCode::BinaryChecksumMismatch => {
            "The collector binary failed integrity verification."
        }
        CollectorFailureCode::VersionMismatch => "The collector version is unsupported.",
        CollectorFailureCode::SpawnFailed => "The collector could not be started.",
        CollectorFailureCode::TimedOut => "The collector exceeded its time limit.",
        CollectorFailureCode::Cancelled => "Collection was cancelled.",
        CollectorFailureCode::StdoutLimitExceeded | CollectorFailureCode::StderrLimitExceeded => {
            "The collector produced more output than allowed."
        }
        CollectorFailureCode::NonUtf8Output => "The collector returned unreadable output.",
        CollectorFailureCode::NonzeroExit => "The collector reported an execution failure.",
        CollectorFailureCode::InvalidJson | CollectorFailureCode::IncompatibleEnvelope => {
            "The collector returned incompatible output."
        }
        CollectorFailureCode::UnsupportedSource => "The requested source is unsupported.",
        CollectorFailureCode::UnsupportedProjection => "The requested projection is unsupported.",
        CollectorFailureCode::ScopeNotRepresentable => {
            "The requested collection scope cannot be represented."
        }
        CollectorFailureCode::SourceNotFound => "The requested source was not found.",
        CollectorFailureCode::SourcePermissionDenied => "Burnly cannot read the requested source.",
        CollectorFailureCode::SourceInvalidLocation => "The configured source location is invalid.",
        CollectorFailureCode::AllRecordsRejected => "The collector returned no usable records.",
        CollectorFailureCode::Internal => "Burnly could not complete collection.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_codes_map_to_reviewed_categories_and_retry_semantics() {
        assert_eq!(
            CollectorFailureCode::BinaryChecksumMismatch.code(),
            "collector.binary_checksum_mismatch"
        );
        assert_eq!(
            CollectorFailureCode::BinaryChecksumMismatch.category(),
            CollectorFailureCategory::Binary
        );
        assert!(!CollectorFailureCode::BinaryChecksumMismatch.retryable());
        assert!(CollectorFailureCode::TimedOut.retryable());
        assert_eq!(
            CollectorFailureCode::InvalidJson.category(),
            CollectorFailureCategory::IncompatibleOutput
        );
    }

    #[test]
    fn failure_exposes_safe_context_without_external_details() {
        let failure = CollectorFailure::new(
            CollectorFailureCode::UnsupportedProjection,
            Some(SourceKey::ClaudeCode),
            Some(CollectionProjection::Session),
        )
        .with_context(CollectorFailureContext {
            runtime_ms: Some(10),
            ..CollectorFailureContext::default()
        });

        assert_eq!(failure.code.code(), "collector.unsupported_projection");
        assert_eq!(failure.category(), CollectorFailureCategory::Unsupported);
        assert!(!failure.retryable());
        assert_eq!(failure.context.runtime_ms, Some(10));
        assert_eq!(
            failure.to_string(),
            "The requested projection is unsupported."
        );
    }
}
