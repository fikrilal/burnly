use chrono::{DateTime, Utc};
use thiserror::Error;

use crate::domain::source::SourceKey;

use super::{
    CollectionId, CollectionProjection, CollectionScope, CollectorKey, DailyUsageCandidate,
    SessionUsageCandidate,
};

const MAX_REJECTIONS: usize = 100;
const MAX_WARNINGS: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CollectionOutcome {
    Complete,
    Partial,
    Empty,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CollectionResult {
    metadata: CollectionMetadata,
    projection: CollectionProjection,
    outcome: CollectionOutcome,
    daily_candidates: Vec<DailyUsageCandidate>,
    session_candidates: Vec<SessionUsageCandidate>,
    rejections: Vec<RejectedRecord>,
    warnings: Vec<CollectionWarning>,
    process_summary: ProcessSummary,
}

impl CollectionResult {
    pub(crate) fn daily(
        metadata: CollectionMetadata,
        daily_candidates: Vec<DailyUsageCandidate>,
        rejections: Vec<RejectedRecord>,
        warnings: Vec<CollectionWarning>,
        process_summary: ProcessSummary,
    ) -> Result<Self, ResultValidationError> {
        if rejections.len() > MAX_REJECTIONS {
            return Err(ResultValidationError::TooManyRejections);
        }
        if warnings.len() > MAX_WARNINGS {
            return Err(ResultValidationError::TooManyWarnings);
        }
        if daily_candidates.is_empty() && !rejections.is_empty() {
            return Err(ResultValidationError::AllRecordsRejected);
        }

        let outcome = if daily_candidates.is_empty() {
            CollectionOutcome::Empty
        } else if rejections.is_empty() {
            CollectionOutcome::Complete
        } else {
            CollectionOutcome::Partial
        };

        Ok(Self {
            metadata,
            projection: CollectionProjection::Daily,
            outcome,
            daily_candidates,
            session_candidates: Vec::new(),
            rejections,
            warnings,
            process_summary,
        })
    }

    pub(crate) fn session(
        metadata: CollectionMetadata,
        session_candidates: Vec<SessionUsageCandidate>,
        rejections: Vec<RejectedRecord>,
        warnings: Vec<CollectionWarning>,
        process_summary: ProcessSummary,
    ) -> Result<Self, ResultValidationError> {
        if rejections.len() > MAX_REJECTIONS {
            return Err(ResultValidationError::TooManyRejections);
        }
        if warnings.len() > MAX_WARNINGS {
            return Err(ResultValidationError::TooManyWarnings);
        }
        if session_candidates.is_empty() && !rejections.is_empty() {
            return Err(ResultValidationError::AllRecordsRejected);
        }

        let outcome = if session_candidates.is_empty() {
            CollectionOutcome::Empty
        } else if rejections.is_empty() {
            CollectionOutcome::Complete
        } else {
            CollectionOutcome::Partial
        };

        Ok(Self {
            metadata,
            projection: CollectionProjection::Session,
            outcome,
            daily_candidates: Vec::new(),
            session_candidates,
            rejections,
            warnings,
            process_summary,
        })
    }

    pub(crate) const fn projection(&self) -> CollectionProjection {
        self.projection
    }

    pub(crate) const fn outcome(&self) -> CollectionOutcome {
        self.outcome
    }

    pub(crate) fn daily_candidates(&self) -> &[DailyUsageCandidate] {
        &self.daily_candidates
    }

    pub(crate) fn session_candidates(&self) -> &[SessionUsageCandidate] {
        &self.session_candidates
    }

    pub(crate) const fn process_summary(&self) -> &ProcessSummary {
        &self.process_summary
    }

    pub(crate) const fn metadata(&self) -> &CollectionMetadata {
        &self.metadata
    }

    pub(crate) fn rejection_count(&self) -> usize {
        self.rejections.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CollectionMetadata {
    collection_id: CollectionId,
    collector: CollectorKey,
    collector_version: String,
    source: SourceKey,
    effective_scope: CollectionScope,
    profile_version: u16,
    started_at: DateTime<Utc>,
    finished_at: DateTime<Utc>,
}

impl CollectionMetadata {
    pub(crate) fn new(
        collection_id: CollectionId,
        collector: CollectorKey,
        collector_version: String,
        source: SourceKey,
        effective_scope: CollectionScope,
        profile_version: u16,
        period: CollectionPeriod,
    ) -> Result<Self, ResultValidationError> {
        if collector_version.trim().is_empty() {
            return Err(ResultValidationError::EmptyCollectorVersion);
        }
        if period.finished_at < period.started_at {
            return Err(ResultValidationError::FinishedBeforeStarted);
        }

        Ok(Self {
            collection_id,
            collector,
            collector_version,
            source,
            effective_scope,
            profile_version,
            started_at: period.started_at,
            finished_at: period.finished_at,
        })
    }

    pub(crate) const fn collector(&self) -> &CollectorKey {
        &self.collector
    }

    pub(crate) fn collector_version(&self) -> &str {
        &self.collector_version
    }

    pub(crate) const fn profile_version(&self) -> u16 {
        self.profile_version
    }

    pub(crate) const fn effective_scope(&self) -> &CollectionScope {
        &self.effective_scope
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CollectionPeriod {
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RejectedRecord {
    pub code: String,
    pub record_index: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CollectionWarning {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProcessSummary {
    pub runtime_ms: u64,
    pub stdout_bytes: u64,
    pub stderr_bytes: u64,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DetectionState {
    Available,
    AvailableNoData,
    NotFound,
    PermissionDenied,
    Unsupported,
    CollectorUnavailable,
    InvalidConfiguration,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DetectionResult {
    pub source: SourceKey,
    pub state: DetectionState,
    pub supported_projections: Vec<CollectionProjection>,
    pub data_roots_found: u16,
    pub usage_artifacts_found: bool,
    pub checked_at: DateTime<Utc>,
    pub issues: Vec<DetectionIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DetectionIssue {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub(crate) enum ResultValidationError {
    #[error("collector version must not be empty")]
    EmptyCollectorVersion,

    #[error("collection finish time must not be before start time")]
    FinishedBeforeStarted,

    #[error("collection result contains too many rejected-record summaries")]
    TooManyRejections,

    #[error("collection result contains too many warnings")]
    TooManyWarnings,

    #[error("all collector records were rejected")]
    AllRecordsRejected,
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    #[test]
    fn empty_daily_collection_is_a_successful_empty_result() {
        let result = CollectionResult::daily(
            metadata(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            process_summary(),
        )
        .expect("empty result");

        assert_eq!(result.projection(), CollectionProjection::Daily);
        assert_eq!(result.outcome(), CollectionOutcome::Empty);
        assert!(result.daily_candidates().is_empty());
    }

    #[test]
    fn all_rejected_records_are_not_reported_as_successful_empty_output() {
        let error = CollectionResult::daily(
            metadata(),
            Vec::new(),
            vec![RejectedRecord {
                code: "record.invalid".to_owned(),
                record_index: Some(0),
            }],
            Vec::new(),
            process_summary(),
        )
        .expect_err("all rejected must fail");

        assert_eq!(error, ResultValidationError::AllRecordsRejected);
    }

    #[test]
    fn metadata_rejects_reversed_collection_period() {
        let period = CollectionPeriod {
            started_at: timestamp(8),
            finished_at: timestamp(7),
        };

        let error = CollectionMetadata::new(
            CollectionId::new("collection-1").expect("collection id"),
            CollectorKey::new("fixture").expect("collector key"),
            "1.0.0".to_owned(),
            SourceKey::ClaudeCode,
            CollectionScope::Full,
            1,
            period,
        )
        .expect_err("invalid period");

        assert_eq!(error, ResultValidationError::FinishedBeforeStarted);
    }

    fn metadata() -> CollectionMetadata {
        CollectionMetadata::new(
            CollectionId::new("collection-1").expect("collection id"),
            CollectorKey::new("fixture").expect("collector key"),
            "1.0.0".to_owned(),
            SourceKey::ClaudeCode,
            CollectionScope::Full,
            1,
            CollectionPeriod {
                started_at: timestamp(7),
                finished_at: timestamp(8),
            },
        )
        .expect("metadata")
    }

    fn process_summary() -> ProcessSummary {
        ProcessSummary {
            runtime_ms: 10,
            stdout_bytes: 2,
            stderr_bytes: 0,
            exit_code: Some(0),
        }
    }

    fn timestamp(hour: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 6, 14, hour, 0, 0)
            .single()
            .expect("timestamp")
    }
}
