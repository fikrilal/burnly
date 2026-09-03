#![allow(
    dead_code,
    reason = "chunk 01 defines baseline repair coordinator contracts consumed in chunks 04 and 05"
)]

use thiserror::Error;

use crate::application::collection::{CollectionProjection, CollectionScope};
use crate::domain::source::SourceKey;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TargetRunOutcome {
    Succeeded,
    Partial,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TargetExecutionOutcome {
    pub(crate) source: SourceKey,
    pub(crate) projection: CollectionProjection,
    pub(crate) effective_scope: CollectionScope,
    pub(crate) outcome: TargetRunOutcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RepairCompletion {
    pub(crate) usage_changed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AntigravityBaselineRepairStage {
    NotStarted,
    CacheReclassified,
    CanonicalCorrected,
    SyncScheduled,
    Complete,
    Skipped,
}

impl AntigravityBaselineRepairStage {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::NotStarted => "not_started",
            Self::CacheReclassified => "cache_reclassified",
            Self::CanonicalCorrected => "canonical_corrected",
            Self::SyncScheduled => "sync_scheduled",
            Self::Complete => "complete",
            Self::Skipped => "skipped",
        }
    }

    pub(crate) fn from_str(s: &str) -> Option<Self> {
        match s {
            "not_started" => Some(Self::NotStarted),
            "cache_reclassified" => Some(Self::CacheReclassified),
            "canonical_corrected" => Some(Self::CanonicalCorrected),
            "sync_scheduled" => Some(Self::SyncScheduled),
            "complete" => Some(Self::Complete),
            "skipped" => Some(Self::Skipped),
            _ => None,
        }
    }
}

#[derive(Debug, Error)]
pub(crate) enum BaselineRepairError {
    #[error("baseline repair query or transaction failed: {0}")]
    Database(String),
}

pub(crate) trait AntigravityBaselineRepairCoordinator: Send + Sync {
    /// Fallible check: returns true if Antigravity requires a full collection scope
    /// because baseline is Pending or repair stage is cache_reclassified.
    fn requires_full_scope(&self) -> Result<bool, BaselineRepairError>;

    /// Current durable stage of the repair pipeline.
    fn current_stage(&self) -> Result<AntigravityBaselineRepairStage, BaselineRepairError>;

    /// Invoked after refresh execution finishes with target outcomes.
    /// Returns Ok(Some(RepairCompletion)) if canonical repair ran and succeeded.
    /// Returns Ok(None) if repair was not needed or not triggered.
    /// Returns Err(e) if canonical repair or sync scheduling failed.
    fn on_refresh_completed(
        &self,
        target_outcomes: &[TargetExecutionOutcome],
        now_ms: i64,
    ) -> Result<Option<RepairCompletion>, BaselineRepairError>;
}

#[cfg(test)]
pub(crate) struct NoopBaselineRepairCoordinator;

#[cfg(test)]
impl AntigravityBaselineRepairCoordinator for NoopBaselineRepairCoordinator {
    fn requires_full_scope(&self) -> Result<bool, BaselineRepairError> {
        Ok(false)
    }

    fn current_stage(&self) -> Result<AntigravityBaselineRepairStage, BaselineRepairError> {
        Ok(AntigravityBaselineRepairStage::Complete)
    }

    fn on_refresh_completed(
        &self,
        _target_outcomes: &[TargetExecutionOutcome],
        _now_ms: i64,
    ) -> Result<Option<RepairCompletion>, BaselineRepairError> {
        Ok(None)
    }
}
