//! Refresh outcome mapping and aggregate bookkeeping.
//!
//! These types translate collection outcomes into import/refresh run outcomes and
//! aggregate per-target results into a single refresh outcome. They are pure and
//! have no threading or persistence side effects.

use crate::application::collection::CollectionOutcome;
use crate::application::reconciliation::{ImportOutcome, RefreshOutcome, RunError};
use crate::application::refresh::state::RefreshStatus;

/// The outcome of a refresh run, derived from collection outcomes and target
/// aggregation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RunOutcome {
    Succeeded,
    Partial,
    Failed,
}

impl RunOutcome {
    pub(super) fn from_collection(outcome: CollectionOutcome) -> Self {
        match outcome {
            CollectionOutcome::Partial => Self::Partial,
            CollectionOutcome::Complete | CollectionOutcome::Empty => Self::Succeeded,
        }
    }

    pub(super) const fn status(self) -> RefreshStatus {
        match self {
            Self::Succeeded => RefreshStatus::Succeeded,
            Self::Partial => RefreshStatus::Partial,
            Self::Failed => RefreshStatus::Failed,
        }
    }

    pub(super) const fn refresh_outcome(self) -> RefreshOutcome {
        match self {
            Self::Succeeded => RefreshOutcome::Succeeded,
            Self::Partial => RefreshOutcome::Partial,
            Self::Failed => RefreshOutcome::Failed,
        }
    }

    pub(super) const fn import_outcome(self) -> ImportOutcome {
        match self {
            Self::Succeeded => ImportOutcome::Succeeded,
            Self::Partial => ImportOutcome::Partial,
            Self::Failed => ImportOutcome::Failed,
        }
    }
}

/// Accumulates per-target outcomes to derive a single refresh outcome.
#[derive(Default)]
pub(super) struct TargetRunAccumulator {
    succeeded: u16,
    partial: u16,
    failed: u16,
}

impl TargetRunAccumulator {
    pub(super) const fn record(&mut self, outcome: RunOutcome) {
        match outcome {
            RunOutcome::Succeeded => self.succeeded += 1,
            RunOutcome::Partial => self.partial += 1,
            RunOutcome::Failed => self.failed += 1,
        }
    }

    pub(super) const fn outcome(&self) -> RunOutcome {
        if self.failed == 0 && self.partial == 0 {
            return RunOutcome::Succeeded;
        }
        if self.succeeded == 0 && self.partial == 0 {
            return RunOutcome::Failed;
        }
        RunOutcome::Partial
    }
}

/// The result of a completed refresh execution flow.
pub(super) struct ExecutionResult {
    pub(super) outcome: RunOutcome,
    pub(super) finished_at_ms: i64,
    pub(super) usage_changed: bool,
    /// Successful daily targets eligible for cloud upload (never blocks refresh).
    pub(super) committed_daily_upload: crate::application::collect_sync::CommittedDailyUpload,
    pub(super) target_outcomes:
        Vec<crate::application::ports::baseline_repair::TargetExecutionOutcome>,
}

/// Carries enough failure context to terminalize an open import and refresh run.
pub(super) struct ExecutionFailure {
    pub(super) import_run_id: Option<crate::application::reconciliation::ImportRunId>,
    pub(super) records_seen: u32,
    pub(super) records_rejected: u32,
    pub(super) finished_at_ms: i64,
    pub(super) usage_changed: bool,
    pub(super) code: &'static str,
    pub(super) summary: &'static str,
    pub(super) committed_daily_upload: crate::application::collect_sync::CommittedDailyUpload,
    pub(super) target_outcomes:
        Vec<crate::application::ports::baseline_repair::TargetExecutionOutcome>,
}

pub(super) fn run_error(code: impl Into<String>, summary: impl Into<String>) -> Option<RunError> {
    RunError::new(code, summary).ok()
}

pub(super) fn clamp_count(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collection_outcomes_map_to_run_outcomes() {
        assert_eq!(
            RunOutcome::from_collection(CollectionOutcome::Complete),
            RunOutcome::Succeeded
        );
        assert_eq!(
            RunOutcome::from_collection(CollectionOutcome::Empty),
            RunOutcome::Succeeded
        );
        assert_eq!(
            RunOutcome::from_collection(CollectionOutcome::Partial),
            RunOutcome::Partial
        );
    }

    #[test]
    fn run_outcomes_map_to_refresh_import_and_status_values() {
        assert_eq!(RunOutcome::Succeeded.status(), RefreshStatus::Succeeded);
        assert_eq!(
            RunOutcome::Succeeded.refresh_outcome(),
            RefreshOutcome::Succeeded
        );
        assert_eq!(
            RunOutcome::Succeeded.import_outcome(),
            ImportOutcome::Succeeded
        );

        assert_eq!(RunOutcome::Partial.status(), RefreshStatus::Partial);
        assert_eq!(
            RunOutcome::Partial.refresh_outcome(),
            RefreshOutcome::Partial
        );
        assert_eq!(RunOutcome::Partial.import_outcome(), ImportOutcome::Partial);

        assert_eq!(RunOutcome::Failed.status(), RefreshStatus::Failed);
        assert_eq!(RunOutcome::Failed.refresh_outcome(), RefreshOutcome::Failed);
        assert_eq!(RunOutcome::Failed.import_outcome(), ImportOutcome::Failed);
    }

    #[test]
    fn target_accumulator_derives_aggregate_outcome() {
        let mut all_success = TargetRunAccumulator::default();
        all_success.record(RunOutcome::Succeeded);
        all_success.record(RunOutcome::Succeeded);
        assert_eq!(all_success.outcome(), RunOutcome::Succeeded);

        let mut all_failed = TargetRunAccumulator::default();
        all_failed.record(RunOutcome::Failed);
        all_failed.record(RunOutcome::Failed);
        assert_eq!(all_failed.outcome(), RunOutcome::Failed);

        let mut mixed = TargetRunAccumulator::default();
        mixed.record(RunOutcome::Succeeded);
        mixed.record(RunOutcome::Failed);
        assert_eq!(mixed.outcome(), RunOutcome::Partial);

        let mut partial = TargetRunAccumulator::default();
        partial.record(RunOutcome::Partial);
        assert_eq!(partial.outcome(), RunOutcome::Partial);
    }

    #[test]
    fn clamp_count_caps_values_above_u32() {
        assert_eq!(clamp_count(42), 42);
        assert_eq!(clamp_count(u32::MAX as usize), u32::MAX);
        assert_eq!(clamp_count(u32::MAX as usize + 1), u32::MAX);
    }
}
