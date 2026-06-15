//! Application-owned port for the overview read query.

#![allow(
    dead_code,
    reason = "Phase 5A defines the overview port before Phase 5B composition"
)]

use thiserror::Error;

use crate::application::usage::{OverviewPeriod, OverviewStoreResult};

pub(crate) trait OverviewStore: Send + Sync {
    fn read_overview(
        &self,
        period: &OverviewPeriod,
    ) -> Result<OverviewStoreResult, OverviewStoreError>;
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OverviewStoreError {
    #[error("an overview value exceeded the supported integer range")]
    ValueOutOfRange,
    #[error("overview cost contains multiple currencies")]
    MixedCurrencies,
    #[error("the overview store backend failed")]
    Backend,
}
