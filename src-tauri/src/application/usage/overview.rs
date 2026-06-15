use std::sync::Arc;

use chrono::NaiveDate;
use thiserror::Error;

use crate::application::ports::clock::Clock;
use crate::application::ports::overview_store::{OverviewStore, OverviewStoreError};
use crate::domain::source::SourceKey;
use crate::domain::usage::CurrencyCode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OverviewPeriod {
    start_date: NaiveDate,
    end_date: NaiveDate,
    aggregation_timezone: String,
}

impl OverviewPeriod {
    pub(crate) fn new(
        start_date: NaiveDate,
        end_date: NaiveDate,
        aggregation_timezone: impl Into<String>,
    ) -> Result<Self, OverviewQueryError> {
        if start_date > end_date {
            return Err(OverviewQueryError::InvalidPeriod);
        }
        let aggregation_timezone = aggregation_timezone.into();
        if aggregation_timezone.trim().is_empty() {
            return Err(OverviewQueryError::EmptyAggregationTimezone);
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CostCompleteness {
    Complete,
    Partial,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CostValuation {
    Available,
    Estimated,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OverviewCost {
    pub amount_micros: Option<u64>,
    pub currency: Option<CurrencyCode>,
    pub valuation: CostValuation,
    pub completeness: CostCompleteness,
    pub unavailable_days: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OverviewSource {
    pub source: SourceKey,
    pub total_tokens: u64,
    pub active_days: u32,
    pub cost: OverviewCost,
    pub has_partial_data: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PersistedRefreshStatus {
    Succeeded,
    Partial,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OverviewDataStatus {
    Current,
    Stale,
    Partial,
    Empty,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OverviewStoreResult {
    pub total_tokens: u64,
    pub active_days: u32,
    pub cost: OverviewCost,
    pub sources: Vec<OverviewSource>,
    pub has_partial_data: bool,
    pub latest_refresh_status: Option<PersistedRefreshStatus>,
    pub last_successful_refresh_at_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OverviewReadModel {
    pub period: OverviewPeriod,
    pub total_tokens: u64,
    pub active_days: u32,
    pub cost: OverviewCost,
    pub sources: Vec<OverviewSource>,
    pub as_of_ms: i64,
    pub last_successful_refresh_at_ms: Option<i64>,
    pub data_status: OverviewDataStatus,
}

pub(crate) struct OverviewQuery {
    store: Arc<dyn OverviewStore>,
    clock: Arc<dyn Clock>,
}

impl OverviewQuery {
    pub(crate) fn new(store: Arc<dyn OverviewStore>, clock: Arc<dyn Clock>) -> Self {
        Self { store, clock }
    }

    pub(crate) fn get(
        &self,
        period: OverviewPeriod,
    ) -> Result<OverviewReadModel, OverviewQueryError> {
        let result = self.store.read_overview(&period)?;
        let data_status = data_status(&result);

        Ok(OverviewReadModel {
            period,
            total_tokens: result.total_tokens,
            active_days: result.active_days,
            cost: result.cost,
            sources: result.sources,
            as_of_ms: self.clock.now_epoch_ms(),
            last_successful_refresh_at_ms: result.last_successful_refresh_at_ms,
            data_status,
        })
    }
}

fn data_status(result: &OverviewStoreResult) -> OverviewDataStatus {
    if result.sources.is_empty() {
        return OverviewDataStatus::Empty;
    }
    if result.has_partial_data
        || matches!(
            result.latest_refresh_status,
            Some(PersistedRefreshStatus::Partial)
        )
    {
        return OverviewDataStatus::Partial;
    }
    if matches!(
        result.latest_refresh_status,
        Some(PersistedRefreshStatus::Failed | PersistedRefreshStatus::Cancelled)
    ) {
        return OverviewDataStatus::Stale;
    }
    OverviewDataStatus::Current
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub(crate) enum OverviewQueryError {
    #[error("overview start date must not be after end date")]
    InvalidPeriod,
    #[error("overview aggregation timezone must not be empty")]
    EmptyAggregationTimezone,
    #[error("overview storage failed")]
    Storage(#[from] OverviewStoreError),
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    struct FixedClock;

    impl Clock for FixedClock {
        fn now_epoch_ms(&self) -> i64 {
            1_000
        }
    }

    struct FakeStore(Mutex<OverviewStoreResult>);

    impl OverviewStore for FakeStore {
        fn read_overview(
            &self,
            _period: &OverviewPeriod,
        ) -> Result<OverviewStoreResult, OverviewStoreError> {
            Ok(self.0.lock().expect("store lock").clone())
        }
    }

    struct FailingStore;

    impl OverviewStore for FailingStore {
        fn read_overview(
            &self,
            _period: &OverviewPeriod,
        ) -> Result<OverviewStoreResult, OverviewStoreError> {
            Err(OverviewStoreError::Backend)
        }
    }

    #[test]
    fn validates_period_and_timezone() {
        let start = NaiveDate::from_ymd_opt(2026, 6, 15).expect("date");
        let end = NaiveDate::from_ymd_opt(2026, 6, 14).expect("date");

        assert_eq!(
            OverviewPeriod::new(start, end, "UTC"),
            Err(OverviewQueryError::InvalidPeriod)
        );
        assert_eq!(
            OverviewPeriod::new(end, start, " "),
            Err(OverviewQueryError::EmptyAggregationTimezone)
        );
    }

    #[test]
    fn derives_empty_partial_stale_and_current_status() {
        for (result, expected) in [
            (result(Vec::new(), false, None), OverviewDataStatus::Empty),
            (
                result(
                    vec![source()],
                    true,
                    Some(PersistedRefreshStatus::Succeeded),
                ),
                OverviewDataStatus::Partial,
            ),
            (
                result(vec![source()], false, Some(PersistedRefreshStatus::Failed)),
                OverviewDataStatus::Stale,
            ),
            (
                result(
                    vec![source()],
                    false,
                    Some(PersistedRefreshStatus::Succeeded),
                ),
                OverviewDataStatus::Current,
            ),
        ] {
            let query = OverviewQuery::new(
                Arc::new(FakeStore(Mutex::new(result))),
                Arc::new(FixedClock),
            );
            let model = query.get(period()).expect("overview");
            assert_eq!(model.data_status, expected);
            assert_eq!(model.as_of_ms, 1_000);
        }
    }

    #[test]
    fn preserves_the_stable_storage_failure_category() {
        let query = OverviewQuery::new(Arc::new(FailingStore), Arc::new(FixedClock));

        assert_eq!(
            query.get(period()),
            Err(OverviewQueryError::Storage(OverviewStoreError::Backend))
        );
    }

    fn period() -> OverviewPeriod {
        OverviewPeriod::new(
            NaiveDate::from_ymd_opt(2026, 6, 1).expect("start"),
            NaiveDate::from_ymd_opt(2026, 6, 15).expect("end"),
            "UTC",
        )
        .expect("period")
    }

    fn result(
        sources: Vec<OverviewSource>,
        has_partial_data: bool,
        latest_refresh_status: Option<PersistedRefreshStatus>,
    ) -> OverviewStoreResult {
        OverviewStoreResult {
            total_tokens: 0,
            active_days: 0,
            cost: unavailable_cost(),
            sources,
            has_partial_data,
            latest_refresh_status,
            last_successful_refresh_at_ms: None,
        }
    }

    fn source() -> OverviewSource {
        OverviewSource {
            source: SourceKey::ClaudeCode,
            total_tokens: 0,
            active_days: 0,
            cost: unavailable_cost(),
            has_partial_data: false,
        }
    }

    fn unavailable_cost() -> OverviewCost {
        OverviewCost {
            amount_micros: None,
            currency: None,
            valuation: CostValuation::Unavailable,
            completeness: CostCompleteness::Unavailable,
            unavailable_days: 0,
        }
    }
}
