use std::sync::Arc;

use chrono::NaiveDate;
use thiserror::Error;

use crate::application::ports::calendar_store::{CalendarStore, CalendarStoreError};
use crate::application::usage::{OverviewCost, OverviewDataStatus};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CalendarPeriod {
    start_date: NaiveDate,
    end_date: NaiveDate,
    aggregation_timezone: String,
}

impl CalendarPeriod {
    pub(crate) fn new(
        start_date: NaiveDate,
        end_date: NaiveDate,
        aggregation_timezone: impl Into<String>,
    ) -> Result<Self, CalendarQueryError> {
        if start_date > end_date {
            return Err(CalendarQueryError::InvalidPeriod);
        }
        let aggregation_timezone = aggregation_timezone.into();
        if aggregation_timezone.trim().is_empty() {
            return Err(CalendarQueryError::EmptyAggregationTimezone);
        }

        Ok(Self {
            start_date,
            end_date,
            aggregation_timezone,
        })
    }

    pub(crate) const fn start_date(&self) -> NaiveDate {
        self.start_date
    }

    pub(crate) const fn end_date(&self) -> NaiveDate {
        self.end_date
    }

    pub(crate) fn aggregation_timezone(&self) -> &str {
        &self.aggregation_timezone
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CalendarDayInfo {
    pub date: NaiveDate,
    pub total_tokens: u64,
    pub active_sources: u32,
    pub cost: OverviewCost,
    pub has_partial_data: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CalendarReadModel {
    pub period: CalendarPeriod,
    pub days: Vec<CalendarDayInfo>,
    pub data_status: OverviewDataStatus, // Reusing data status for simplicity
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub(crate) enum CalendarQueryError {
    #[error("calendar start date must not be after end date")]
    InvalidPeriod,
    #[error("calendar aggregation timezone must not be empty")]
    EmptyAggregationTimezone,
    #[error("calendar storage failed")]
    Storage(#[from] CalendarStoreError),
}

pub(crate) struct CalendarQuery {
    store: Arc<dyn CalendarStore>,
}

impl CalendarQuery {
    pub(crate) fn new(store: Arc<dyn CalendarStore>) -> Self {
        Self { store }
    }

    pub(crate) fn get(
        &self,
        period: CalendarPeriod,
    ) -> Result<CalendarReadModel, CalendarQueryError> {
        let mut model = self.store.read_calendar(&period)?;
        // data_status should ideally be calculated based on latest refresh and sources
        // but for now we rely on the store to populate it correctly or we default it.
        // The store returns CalendarReadModel.
        model.period = period;
        Ok(model)
    }
}
