//! Application-owned port for the day detail read query.

use chrono::NaiveDate;
use thiserror::Error;

use crate::application::usage::DayDetailReadModel;

pub(crate) trait DayDetailStore: Send + Sync {
    fn read_day_detail(
        &self,
        date: NaiveDate,
    ) -> Result<Option<DayDetailReadModel>, DayDetailStoreError>;
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DayDetailStoreError {
    #[error("the day detail store backend failed")]
    Backend,
}
