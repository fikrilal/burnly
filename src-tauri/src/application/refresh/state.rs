//! Refresh state model exposed by the coordinator.

#![allow(
    dead_code,
    reason = "Refresh state is surfaced through the Phase 4F IPC commands"
)]

use crate::application::reconciliation::RefreshTrigger;

/// Stable refresh lifecycle states owned by the coordinator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RefreshStatus {
    Idle,
    Queued,
    Running,
    Cancelling,
    Succeeded,
    Partial,
    Failed,
}

impl RefreshStatus {
    /// Whether a refresh is in flight and a new request must coalesce rather than
    /// start a competing job.
    pub(crate) const fn is_active(self) -> bool {
        matches!(self, Self::Queued | Self::Running | Self::Cancelling)
    }
}

/// A point-in-time view of refresh state for callers and the tray snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RefreshSnapshot {
    pub status: RefreshStatus,
    pub job_id: Option<String>,
    pub trigger: Option<RefreshTrigger>,
    pub last_successful_refresh_at_ms: Option<i64>,
}
