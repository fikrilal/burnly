//! Application-owned port for transactional reconciliation of canonical usage.
//!
//! Reconciliation is the only path that mutates imported usage facts. The
//! application invokes one reconcile operation; the implementation owns the
//! transaction, upserts, and child-row replacement.

#![allow(
    dead_code,
    reason = "The usage store contract is implemented now and called by the Phase 4E refresh coordinator"
)]

use thiserror::Error;

use crate::application::reconciliation::{DailyReconciliationRequest, DailyReconciliationSummary};

pub(crate) trait UsageStore: Send + Sync {
    /// Reconciles validated daily candidates into canonical facts within a single
    /// write transaction. Either all candidates in the request commit, or none do.
    fn reconcile_daily(
        &self,
        request: DailyReconciliationRequest,
    ) -> Result<DailyReconciliationSummary, UsageStoreError>;
}

/// Failure categories surfaced by the usage store, independent of the engine.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UsageStoreError {
    #[error("a usage value exceeded the supported integer range")]
    ValueOutOfRange,
    #[error("the usage store backend failed")]
    Backend,
}
