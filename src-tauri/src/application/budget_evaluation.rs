use std::str::FromStr;
use std::sync::Arc;

use chrono::{DateTime, Datelike, Days, NaiveDate, Utc};
use chrono_tz::Tz;
use thiserror::Error;

use crate::application::ports::budget_store::{BudgetStore, BudgetStoreError};
use crate::application::ports::budget_usage_store::{BudgetUsageStore, BudgetUsageStoreError};
use crate::domain::budget::{Budget, BudgetId, BudgetLimit, BudgetPeriod, BudgetScope};
use crate::domain::usage::CurrencyCode;

pub(crate) struct BudgetEvaluationService {
    budget_store: Arc<dyn BudgetStore>,
    usage_store: Arc<dyn BudgetUsageStore>,
}

impl BudgetEvaluationService {
    pub(crate) fn new(
        budget_store: Arc<dyn BudgetStore>,
        usage_store: Arc<dyn BudgetUsageStore>,
    ) -> Self {
        Self {
            budget_store,
            usage_store,
        }
    }

    pub(crate) fn evaluate(
        &self,
        aggregation_timezone: &str,
        now_epoch_ms: i64,
    ) -> Result<BudgetEvaluationReport, BudgetEvaluationError> {
        let budgets = self
            .budget_store
            .list()
            .map_err(BudgetEvaluationError::from)?;
        let mut progress = Vec::new();
        let mut decisions = Vec::new();

        for budget in budgets
            .into_iter()
            .filter(|budget| budget.definition().enabled())
        {
            let period = BudgetPeriodWindow::for_instant(
                budget.definition().period(),
                aggregation_timezone,
                now_epoch_ms,
            )?;
            let usage = self
                .usage_store
                .aggregate_budget_usage(&BudgetUsageRequest {
                    period: period.clone(),
                    scope: budget.definition().scope(),
                })?;
            let budget_progress = progress_for_budget(&budget, period, usage)?;
            decisions.extend(threshold_decisions(&budget_progress));
            progress.push(budget_progress);
        }

        Ok(BudgetEvaluationReport {
            progress,
            threshold_decisions: decisions,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BudgetEvaluationReport {
    pub progress: Vec<BudgetProgress>,
    pub threshold_decisions: Vec<BudgetThresholdDecision>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BudgetPeriodWindow {
    pub period: BudgetPeriod,
    start_date: NaiveDate,
    end_date: NaiveDate,
    aggregation_timezone: String,
}

impl BudgetPeriodWindow {
    pub(crate) fn new(
        period: BudgetPeriod,
        start_date: NaiveDate,
        end_date: NaiveDate,
        aggregation_timezone: impl Into<String>,
    ) -> Result<Self, BudgetEvaluationError> {
        let aggregation_timezone = aggregation_timezone.into();
        if start_date > end_date || aggregation_timezone.trim().is_empty() {
            return Err(BudgetEvaluationError::InvalidTimestamp);
        }
        Ok(Self {
            period,
            start_date,
            end_date,
            aggregation_timezone,
        })
    }

    pub(crate) fn for_instant(
        period: BudgetPeriod,
        aggregation_timezone: &str,
        now_epoch_ms: i64,
    ) -> Result<Self, BudgetEvaluationError> {
        let timezone = Tz::from_str(aggregation_timezone)
            .map_err(|_| BudgetEvaluationError::InvalidTimezone)?;
        let instant = DateTime::<Utc>::from_timestamp_millis(now_epoch_ms)
            .ok_or(BudgetEvaluationError::InvalidTimestamp)?;
        let local_date = instant.with_timezone(&timezone).date_naive();
        let (start_date, end_date) = period_bounds(period, local_date)?;

        Self::new(period, start_date, end_date, aggregation_timezone)
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
pub(crate) struct BudgetUsageRequest {
    pub period: BudgetPeriodWindow,
    pub scope: BudgetScope,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BudgetUsageAggregate {
    pub total_tokens: u64,
    pub active_days: u32,
    pub cost: BudgetUsageCost,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BudgetUsageCost {
    pub amount_micros: Option<u64>,
    pub currency: Option<CurrencyCode>,
    pub completeness: BudgetCostCompleteness,
    pub unavailable_days: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BudgetCostCompleteness {
    Complete,
    Partial,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BudgetProgress {
    pub budget_id: BudgetId,
    pub period: BudgetPeriodWindow,
    pub limit: BudgetLimit,
    pub value: BudgetProgressValue,
    pub thresholds: Vec<BudgetThresholdProgress>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BudgetProgressValue {
    Tokens {
        current: u64,
        limit: u64,
        basis_points: u64,
    },
    Cost {
        current_micros: Option<u64>,
        limit_micros: u64,
        currency: CurrencyCode,
        basis_points: Option<u64>,
        completeness: BudgetCostCompleteness,
        unavailable_days: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BudgetThresholdProgress {
    pub basis_points: u32,
    pub crossed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BudgetThresholdDecision {
    pub budget_id: BudgetId,
    pub period: BudgetPeriodWindow,
    pub threshold_basis_points: u32,
    pub observed_value: u64,
}

fn period_bounds(
    period: BudgetPeriod,
    local_date: NaiveDate,
) -> Result<(NaiveDate, NaiveDate), BudgetEvaluationError> {
    match period {
        BudgetPeriod::Daily => Ok((local_date, local_date)),
        BudgetPeriod::Weekly => {
            let start_date = local_date
                .checked_sub_days(Days::new(u64::from(
                    local_date.weekday().num_days_from_monday(),
                )))
                .ok_or(BudgetEvaluationError::InvalidTimestamp)?;
            let end_date = start_date
                .checked_add_days(Days::new(6))
                .ok_or(BudgetEvaluationError::InvalidTimestamp)?;
            Ok((start_date, end_date))
        }
        BudgetPeriod::Monthly => {
            let start_date = NaiveDate::from_ymd_opt(local_date.year(), local_date.month(), 1)
                .ok_or(BudgetEvaluationError::InvalidTimestamp)?;
            let next_month = if local_date.month() == 12 {
                NaiveDate::from_ymd_opt(local_date.year() + 1, 1, 1)
            } else {
                NaiveDate::from_ymd_opt(local_date.year(), local_date.month() + 1, 1)
            }
            .ok_or(BudgetEvaluationError::InvalidTimestamp)?;
            let end_date = next_month
                .checked_sub_days(Days::new(1))
                .ok_or(BudgetEvaluationError::InvalidTimestamp)?;
            Ok((start_date, end_date))
        }
    }
}

fn progress_for_budget(
    budget: &Budget,
    period: BudgetPeriodWindow,
    usage: BudgetUsageAggregate,
) -> Result<BudgetProgress, BudgetEvaluationError> {
    let value = match budget.definition().limit() {
        BudgetLimit::Tokens(limit) => BudgetProgressValue::Tokens {
            current: usage.total_tokens,
            limit: *limit,
            basis_points: ratio_basis_points(usage.total_tokens, *limit),
        },
        BudgetLimit::CostMicros {
            amount_micros,
            currency,
        } => cost_progress_value(&usage, *amount_micros, currency.clone())?,
    };
    let thresholds = budget
        .definition()
        .thresholds()
        .iter()
        .filter(|threshold| threshold.enabled())
        .map(|threshold| BudgetThresholdProgress {
            basis_points: threshold.basis_points(),
            crossed: threshold_crossed(&value, threshold.basis_points()),
        })
        .collect();

    Ok(BudgetProgress {
        budget_id: budget.id(),
        period,
        limit: budget.definition().limit().clone(),
        value,
        thresholds,
    })
}

fn cost_progress_value(
    usage: &BudgetUsageAggregate,
    limit_micros: u64,
    currency: CurrencyCode,
) -> Result<BudgetProgressValue, BudgetEvaluationError> {
    if usage.active_days == 0 {
        return Ok(BudgetProgressValue::Cost {
            current_micros: Some(0),
            limit_micros,
            currency,
            basis_points: Some(0),
            completeness: BudgetCostCompleteness::Complete,
            unavailable_days: 0,
        });
    }

    let Some(current_micros) = usage.cost.amount_micros else {
        return Ok(BudgetProgressValue::Cost {
            current_micros: None,
            limit_micros,
            currency,
            basis_points: None,
            completeness: usage.cost.completeness,
            unavailable_days: usage.cost.unavailable_days,
        });
    };
    if usage.cost.currency != Some(currency.clone()) {
        return Err(BudgetEvaluationError::CurrencyMismatch);
    }

    Ok(BudgetProgressValue::Cost {
        current_micros: Some(current_micros),
        limit_micros,
        currency,
        basis_points: Some(ratio_basis_points(current_micros, limit_micros)),
        completeness: usage.cost.completeness,
        unavailable_days: usage.cost.unavailable_days,
    })
}

fn threshold_decisions(progress: &BudgetProgress) -> Vec<BudgetThresholdDecision> {
    let Some(observed_value) = observed_value(&progress.value) else {
        return Vec::new();
    };

    progress
        .thresholds
        .iter()
        .filter(|threshold| threshold.crossed)
        .map(|threshold| BudgetThresholdDecision {
            budget_id: progress.budget_id,
            period: progress.period.clone(),
            threshold_basis_points: threshold.basis_points,
            observed_value,
        })
        .collect()
}

fn threshold_crossed(value: &BudgetProgressValue, threshold_basis_points: u32) -> bool {
    let Some((current, limit)) = current_and_limit(value) else {
        return false;
    };

    u128::from(current) * 10_000 >= u128::from(limit) * u128::from(threshold_basis_points)
}

fn current_and_limit(value: &BudgetProgressValue) -> Option<(u64, u64)> {
    match value {
        BudgetProgressValue::Tokens { current, limit, .. } => Some((*current, *limit)),
        BudgetProgressValue::Cost {
            current_micros,
            limit_micros,
            ..
        } => current_micros.map(|current| (current, *limit_micros)),
    }
}

fn observed_value(value: &BudgetProgressValue) -> Option<u64> {
    match value {
        BudgetProgressValue::Tokens { current, .. } => Some(*current),
        BudgetProgressValue::Cost { current_micros, .. } => *current_micros,
    }
}

fn ratio_basis_points(current: u64, limit: u64) -> u64 {
    let ratio = u128::from(current) * 10_000 / u128::from(limit);
    u64::try_from(ratio).unwrap_or(u64::MAX)
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BudgetEvaluationError {
    #[error("budget evaluation timezone is invalid")]
    InvalidTimezone,
    #[error("budget evaluation timestamp is invalid")]
    InvalidTimestamp,
    #[error("budget evaluation storage failed")]
    StorageUnavailable,
    #[error("budget evaluation found invalid stored values")]
    InvalidStoredValue,
    #[error("budget evaluation found mixed or mismatched currencies")]
    CurrencyMismatch,
}

impl From<BudgetStoreError> for BudgetEvaluationError {
    fn from(value: BudgetStoreError) -> Self {
        match value {
            BudgetStoreError::Unavailable => Self::StorageUnavailable,
            BudgetStoreError::InvalidStoredValue => Self::InvalidStoredValue,
            BudgetStoreError::NotFound
            | BudgetStoreError::Conflict
            | BudgetStoreError::UnknownSource => Self::StorageUnavailable,
        }
    }
}

impl From<BudgetUsageStoreError> for BudgetEvaluationError {
    fn from(value: BudgetUsageStoreError) -> Self {
        match value {
            BudgetUsageStoreError::ValueOutOfRange => Self::InvalidStoredValue,
            BudgetUsageStoreError::MixedCurrencies => Self::CurrencyMismatch,
            BudgetUsageStoreError::Backend => Self::StorageUnavailable,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::domain::budget::{BudgetDefinition, BudgetThreshold};

    #[derive(Default)]
    struct FakeBudgetStore {
        budgets: Mutex<Vec<Budget>>,
    }

    impl BudgetStore for FakeBudgetStore {
        fn create(
            &self,
            _definition: &BudgetDefinition,
            _now_epoch_ms: i64,
        ) -> Result<Budget, BudgetStoreError> {
            Err(BudgetStoreError::Unavailable)
        }

        fn get(&self, _id: BudgetId) -> Result<Budget, BudgetStoreError> {
            Err(BudgetStoreError::Unavailable)
        }

        fn list(&self) -> Result<Vec<Budget>, BudgetStoreError> {
            Ok(self.budgets.lock().expect("budget lock").clone())
        }

        fn replace(
            &self,
            _id: BudgetId,
            _expected_revision: i64,
            _definition: &BudgetDefinition,
            _now_epoch_ms: i64,
        ) -> Result<Budget, BudgetStoreError> {
            Err(BudgetStoreError::Unavailable)
        }

        fn set_enabled(
            &self,
            _id: BudgetId,
            _expected_revision: i64,
            _enabled: bool,
            _now_epoch_ms: i64,
        ) -> Result<Budget, BudgetStoreError> {
            Err(BudgetStoreError::Unavailable)
        }

        fn delete(&self, _id: BudgetId, _expected_revision: i64) -> Result<(), BudgetStoreError> {
            Err(BudgetStoreError::Unavailable)
        }
    }

    struct FakeUsageStore(Mutex<BudgetUsageAggregate>);

    impl BudgetUsageStore for FakeUsageStore {
        fn aggregate_budget_usage(
            &self,
            _request: &BudgetUsageRequest,
        ) -> Result<BudgetUsageAggregate, BudgetUsageStoreError> {
            Ok(self.0.lock().expect("usage lock").clone())
        }
    }

    #[test]
    fn period_windows_use_reporting_timezone_and_iso_weeks() {
        let sunday_utc = DateTime::parse_from_rfc3339("2026-06-14T20:00:00Z")
            .expect("instant")
            .timestamp_millis();

        let jakarta =
            BudgetPeriodWindow::for_instant(BudgetPeriod::Daily, "Asia/Jakarta", sunday_utc)
                .expect("jakarta day");
        assert_eq!(jakarta.start_date(), date(2026, 6, 15));
        assert_eq!(jakarta.end_date(), date(2026, 6, 15));

        let weekly = BudgetPeriodWindow::for_instant(BudgetPeriod::Weekly, "UTC", sunday_utc)
            .expect("weekly");
        assert_eq!(weekly.start_date(), date(2026, 6, 8));
        assert_eq!(weekly.end_date(), date(2026, 6, 14));
    }

    #[test]
    fn monthly_period_handles_dst_timezone_dates() {
        let instant = DateTime::parse_from_rfc3339("2026-03-08T07:30:00Z")
            .expect("instant")
            .timestamp_millis();

        let monthly =
            BudgetPeriodWindow::for_instant(BudgetPeriod::Monthly, "America/New_York", instant)
                .expect("monthly");

        assert_eq!(monthly.start_date(), date(2026, 3, 1));
        assert_eq!(monthly.end_date(), date(2026, 3, 31));
    }

    #[test]
    fn token_progress_can_exceed_one_hundred_percent_and_orders_thresholds() {
        let budget_store = Arc::new(FakeBudgetStore::default());
        budget_store
            .budgets
            .lock()
            .expect("budget lock")
            .push(budget(
                1,
                BudgetLimit::tokens(1_000).expect("limit"),
                BudgetPeriod::Monthly,
                BudgetScope::Global,
                true,
                &[8_000, 10_000, 15_000],
            ));
        let usage_store = Arc::new(FakeUsageStore(Mutex::new(aggregate(
            1_500,
            0,
            cost(None, None, BudgetCostCompleteness::Unavailable, 0),
        ))));
        let service = BudgetEvaluationService::new(budget_store, usage_store);

        let report = service.evaluate("UTC", 1_781_498_400_000).expect("report");

        assert_eq!(
            report.progress[0].value,
            BudgetProgressValue::Tokens {
                current: 1_500,
                limit: 1_000,
                basis_points: 15_000,
            }
        );
        assert_eq!(
            report
                .threshold_decisions
                .iter()
                .map(|decision| decision.threshold_basis_points)
                .collect::<Vec<_>>(),
            vec![8_000, 10_000, 15_000]
        );
    }

    #[test]
    fn cost_progress_preserves_partial_and_unavailable_semantics() {
        let currency = CurrencyCode::new("USD").expect("currency");
        let budget_store = Arc::new(FakeBudgetStore::default());
        budget_store
            .budgets
            .lock()
            .expect("budget lock")
            .push(budget(
                1,
                BudgetLimit::cost_micros(10_000_000, currency.clone()).expect("limit"),
                BudgetPeriod::Monthly,
                BudgetScope::Global,
                true,
                &[5_000],
            ));
        let usage_store = Arc::new(FakeUsageStore(Mutex::new(aggregate(
            0,
            3,
            cost(
                Some(6_000_000),
                Some(currency.clone()),
                BudgetCostCompleteness::Partial,
                1,
            ),
        ))));
        let service = BudgetEvaluationService::new(budget_store, usage_store.clone());

        let report = service.evaluate("UTC", 1_781_498_400_000).expect("report");

        assert_eq!(
            report.progress[0].value,
            BudgetProgressValue::Cost {
                current_micros: Some(6_000_000),
                limit_micros: 10_000_000,
                currency,
                basis_points: Some(6_000),
                completeness: BudgetCostCompleteness::Partial,
                unavailable_days: 1,
            }
        );
        assert_eq!(report.threshold_decisions.len(), 1);

        *usage_store.0.lock().expect("usage lock") = aggregate(
            0,
            1,
            cost(None, None, BudgetCostCompleteness::Unavailable, 1),
        );
        let report = service.evaluate("UTC", 1_781_498_400_000).expect("report");
        assert!(report.threshold_decisions.is_empty());
    }

    fn budget(
        id: i64,
        limit: BudgetLimit,
        period: BudgetPeriod,
        scope: BudgetScope,
        enabled: bool,
        thresholds: &[u32],
    ) -> Budget {
        let definition = BudgetDefinition::new(
            "Budget",
            limit,
            period,
            scope,
            enabled,
            thresholds
                .iter()
                .map(|threshold| BudgetThreshold::new(*threshold, true).expect("threshold"))
                .collect(),
        )
        .expect("definition");
        Budget::new(BudgetId::new(id).expect("id"), 1, definition).expect("budget")
    }

    fn aggregate(
        total_tokens: u64,
        active_days: u32,
        cost: BudgetUsageCost,
    ) -> BudgetUsageAggregate {
        BudgetUsageAggregate {
            total_tokens,
            active_days,
            cost,
        }
    }

    fn cost(
        amount_micros: Option<u64>,
        currency: Option<CurrencyCode>,
        completeness: BudgetCostCompleteness,
        unavailable_days: u32,
    ) -> BudgetUsageCost {
        BudgetUsageCost {
            amount_micros,
            currency,
            completeness,
            unavailable_days,
        }
    }

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("date")
    }
}
