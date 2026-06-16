//! Application-owned port for the calendar read query.

use thiserror::Error;

use crate::application::usage::{CalendarPeriod, CalendarReadModel};

pub(crate) trait CalendarStore: Send + Sync {
    fn read_calendar(
        &self,
        period: &CalendarPeriod,
    ) -> Result<CalendarReadModel, CalendarStoreError>;
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CalendarStoreError {
    #[error("a calendar value exceeded the supported integer range")]
    ValueOutOfRange,
    #[error("calendar cost contains multiple currencies")]
    MixedCurrencies,
    #[error("the calendar store backend failed")]
    Backend,
}
