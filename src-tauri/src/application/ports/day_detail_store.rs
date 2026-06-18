//! Application-owned port for the day detail read query.

use thiserror::Error;

use crate::application::usage::{DayDetailPeriod, DayDetailReadModel};

pub(crate) trait DayDetailStore: Send + Sync {
    fn read_day_detail(
        &self,
        period: &DayDetailPeriod,
    ) -> Result<DayDetailReadModel, DayDetailStoreError>;
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DayDetailStoreError {
    #[error("the day detail store backend failed")]
    Backend,
    #[error("day detail data contains a value outside the supported range")]
    ValueOutOfRange,
    #[error("day detail data contains mixed currencies")]
    MixedCurrencies,
}
