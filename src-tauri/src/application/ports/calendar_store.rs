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
    #[error("the calendar store backend failed")]
    Backend,
    #[error("calendar data contains a value outside the supported range")]
    ValueOutOfRange,
    #[error("calendar data contains mixed currencies")]
    MixedCurrencies,
}
