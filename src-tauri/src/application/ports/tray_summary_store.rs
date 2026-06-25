//! Application-owned port for the compact tray summary read query.

use thiserror::Error;

use crate::application::usage::{TraySummaryScope, TraySummaryStoreResult};

pub(crate) trait TraySummaryStore: Send + Sync {
    fn read_tray_summary(
        &self,
        scope: &TraySummaryScope,
    ) -> Result<TraySummaryStoreResult, TraySummaryStoreError>;
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TraySummaryStoreError {
    #[error("a tray summary value exceeded the supported integer range")]
    ValueOutOfRange,
    #[error("the tray summary store backend failed")]
    Backend,
}
