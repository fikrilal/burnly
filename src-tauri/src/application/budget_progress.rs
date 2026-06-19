use std::sync::Arc;

use chrono::{DateTime, Utc};

use crate::application::budget_evaluation::{
    BudgetCostCompleteness, BudgetEvaluationError, BudgetEvaluationService, BudgetProgress,
    BudgetProgressValue,
};
use crate::application::ports::budget_store::{BudgetStore, BudgetStoreError};
use crate::application::ports::budget_usage_store::BudgetUsageStore;
use crate::application::ports::clock::Clock;
use crate::application::ports::settings_store::{SettingsStore, SettingsStoreError};
use crate::domain::budget::BudgetPeriod;

pub(crate) struct BudgetProgressQuery {
    budget_store: Arc<dyn BudgetStore>,
    evaluator: BudgetEvaluationService,
    settings_store: Arc<dyn SettingsStore>,
    clock: Arc<dyn Clock>,
}

impl BudgetProgressQuery {
    pub(crate) fn new(
        budget_store: Arc<dyn BudgetStore>,
        usage_store: Arc<dyn BudgetUsageStore>,
        settings_store: Arc<dyn SettingsStore>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        let evaluator = BudgetEvaluationService::new(budget_store.clone(), usage_store);
        Self {
            budget_store,
            evaluator,
            settings_store,
            clock,
        }
    }

    pub(crate) fn current(&self) -> Result<CurrentBudgetProgressReadModel, BudgetProgressError> {
        let settings = self
            .settings_store
            .get()
            .map_err(BudgetProgressError::from_settings)?;
        let budgets = self
            .budget_store
            .list()
            .map_err(BudgetProgressError::from_store)?;
        let configured_budget_count = budgets.len();
        let enabled_budget_count = budgets
            .iter()
            .filter(|budget| budget.definition().enabled())
            .count();
        let now_epoch_ms = self.clock.now_epoch_ms();
        let as_of = DateTime::<Utc>::from_timestamp_millis(now_epoch_ms)
            .ok_or(BudgetProgressError::InvalidTimestamp)?;

        if configured_budget_count == 0 {
            return Ok(CurrentBudgetProgressReadModel {
                status: BudgetProgressStatus::NoBudgets,
                reporting_timezone: settings.settings().reporting_timezone().to_owned(),
                as_of,
                configured_budget_count,
                enabled_budget_count,
                items: Vec::new(),
                tray_summary: None,
            });
        }

        if enabled_budget_count == 0 {
            return Ok(CurrentBudgetProgressReadModel {
                status: BudgetProgressStatus::AllDisabled,
                reporting_timezone: settings.settings().reporting_timezone().to_owned(),
                as_of,
                configured_budget_count,
                enabled_budget_count,
                items: Vec::new(),
                tray_summary: Some("Budgets disabled".to_owned()),
            });
        }

        let report = self
            .evaluator
            .evaluate(settings.settings().reporting_timezone(), now_epoch_ms)
            .map_err(BudgetProgressError::from_evaluation)?;
        let mut items = report
            .progress
            .into_iter()
            .map(CurrentBudgetProgressItem::from_progress)
            .collect::<Vec<_>>();
        items.sort_by(|left, right| {
            right
                .basis_points
                .unwrap_or(0)
                .cmp(&left.basis_points.unwrap_or(0))
                .then_with(|| left.budget_name.cmp(&right.budget_name))
        });
        let tray_summary = items.first().map(tray_summary_for_item);

        Ok(CurrentBudgetProgressReadModel {
            status: BudgetProgressStatus::Available,
            reporting_timezone: settings.settings().reporting_timezone().to_owned(),
            as_of,
            configured_budget_count,
            enabled_budget_count,
            items,
            tray_summary,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CurrentBudgetProgressReadModel {
    pub status: BudgetProgressStatus,
    pub reporting_timezone: String,
    pub as_of: DateTime<Utc>,
    pub configured_budget_count: usize,
    pub enabled_budget_count: usize,
    pub items: Vec<CurrentBudgetProgressItem>,
    pub tray_summary: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BudgetProgressStatus {
    NoBudgets,
    AllDisabled,
    Available,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CurrentBudgetProgressItem {
    pub budget_id: i64,
    pub budget_name: String,
    pub period: BudgetPeriod,
    pub period_start_date: String,
    pub period_end_date: String,
    pub metric: BudgetProgressMetric,
    pub state: BudgetProgressItemState,
    pub current: Option<u64>,
    pub limit: u64,
    pub currency: Option<String>,
    pub basis_points: Option<u64>,
    pub exceeded: bool,
    pub completeness: BudgetCostCompleteness,
    pub unavailable_days: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BudgetProgressMetric {
    Tokens,
    Cost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BudgetProgressItemState {
    Available,
    CostUnavailable,
}

impl CurrentBudgetProgressItem {
    fn from_progress(progress: BudgetProgress) -> Self {
        let (metric, state, current, limit, currency, basis_points, completeness, unavailable_days) =
            match progress.value {
                BudgetProgressValue::Tokens {
                    current,
                    limit,
                    basis_points,
                } => (
                    BudgetProgressMetric::Tokens,
                    BudgetProgressItemState::Available,
                    Some(current),
                    limit,
                    None,
                    Some(basis_points),
                    BudgetCostCompleteness::Complete,
                    0,
                ),
                BudgetProgressValue::Cost {
                    current_micros,
                    limit_micros,
                    currency,
                    basis_points,
                    completeness,
                    unavailable_days,
                } => (
                    BudgetProgressMetric::Cost,
                    if basis_points.is_some() {
                        BudgetProgressItemState::Available
                    } else {
                        BudgetProgressItemState::CostUnavailable
                    },
                    current_micros,
                    limit_micros,
                    Some(currency.as_str().to_owned()),
                    basis_points,
                    completeness,
                    unavailable_days,
                ),
            };

        Self {
            budget_id: progress.budget_id.value(),
            budget_name: progress.budget_name,
            period: progress.period.period,
            period_start_date: progress.period.start_date().to_string(),
            period_end_date: progress.period.end_date().to_string(),
            metric,
            state,
            current,
            limit,
            currency,
            basis_points,
            exceeded: basis_points.is_some_and(|value| value >= 10_000),
            completeness,
            unavailable_days,
        }
    }
}

fn tray_summary_for_item(item: &CurrentBudgetProgressItem) -> String {
    match item.state {
        BudgetProgressItemState::Available => {
            let percent = item.basis_points.unwrap_or(0) / 100;
            format!("Budget: {} {percent}%", item.budget_name)
        }
        BudgetProgressItemState::CostUnavailable => {
            format!("Budget: {} cost unavailable", item.budget_name)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BudgetProgressError {
    InvalidTimestamp,
    InvalidTimezone,
    StorageUnavailable,
    InvalidStoredValue,
    CurrencyMismatch,
}

impl BudgetProgressError {
    fn from_settings(error: SettingsStoreError) -> Self {
        match error {
            SettingsStoreError::Unavailable => Self::StorageUnavailable,
            SettingsStoreError::InvalidStoredValue => Self::InvalidStoredValue,
            SettingsStoreError::Conflict => Self::StorageUnavailable,
        }
    }

    fn from_store(error: BudgetStoreError) -> Self {
        match error {
            BudgetStoreError::Unavailable => Self::StorageUnavailable,
            BudgetStoreError::InvalidStoredValue => Self::InvalidStoredValue,
            BudgetStoreError::NotFound
            | BudgetStoreError::Conflict
            | BudgetStoreError::UnknownSource => Self::StorageUnavailable,
        }
    }

    fn from_evaluation(error: BudgetEvaluationError) -> Self {
        match error {
            BudgetEvaluationError::InvalidTimezone => Self::InvalidTimezone,
            BudgetEvaluationError::InvalidTimestamp => Self::InvalidTimestamp,
            BudgetEvaluationError::StorageUnavailable => Self::StorageUnavailable,
            BudgetEvaluationError::InvalidStoredValue => Self::InvalidStoredValue,
            BudgetEvaluationError::CurrencyMismatch => Self::CurrencyMismatch,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::application::budget_evaluation::{
        BudgetUsageAggregate, BudgetUsageCost, BudgetUsageRequest,
    };
    use crate::application::ports::budget_usage_store::BudgetUsageStoreError;
    use crate::application::ports::settings_store::ProjectPathRetentionResult;
    use crate::domain::budget::{
        Budget, BudgetDefinition, BudgetId, BudgetLimit, BudgetScope, BudgetThreshold,
    };
    use crate::domain::settings::{Settings, SettingsDocument};
    use crate::domain::usage::CurrencyCode;

    struct FixedClock;

    impl Clock for FixedClock {
        fn now_epoch_ms(&self) -> i64 {
            1_781_498_400_000
        }
    }

    #[derive(Default)]
    struct MemoryBudgetStore {
        budgets: Mutex<Vec<Budget>>,
    }

    impl BudgetStore for MemoryBudgetStore {
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

    struct FixedUsageStore(Mutex<BudgetUsageAggregate>);

    impl BudgetUsageStore for FixedUsageStore {
        fn aggregate_budget_usage(
            &self,
            _request: &BudgetUsageRequest,
        ) -> Result<BudgetUsageAggregate, BudgetUsageStoreError> {
            Ok(self.0.lock().expect("usage lock").clone())
        }
    }

    struct FixedSettingsStore;

    impl SettingsStore for FixedSettingsStore {
        fn get(&self) -> Result<SettingsDocument, SettingsStoreError> {
            Ok(SettingsDocument::new(
                Settings::new("UTC".to_owned(), false, 15, false, "quit", false, true)
                    .expect("settings"),
                1,
            )
            .expect("document"))
        }

        fn replace(
            &self,
            _expected_revision: i64,
            _settings: &Settings,
            _updated_at_ms: i64,
        ) -> Result<SettingsDocument, SettingsStoreError> {
            Err(SettingsStoreError::Unavailable)
        }

        fn replace_project_path_retention(
            &self,
            _expected_revision: i64,
            _retain_paths: bool,
            _updated_at_ms: i64,
        ) -> Result<ProjectPathRetentionResult, SettingsStoreError> {
            Err(SettingsStoreError::Unavailable)
        }
    }

    #[test]
    fn no_budget_and_all_disabled_states_are_explicit() {
        let budgets = Arc::new(MemoryBudgetStore::default());
        let query = query(budgets.clone(), usage(0, None));

        assert_eq!(
            query.current().expect("progress").status,
            BudgetProgressStatus::NoBudgets
        );

        budgets.budgets.lock().expect("budget lock").push(budget(
            1,
            BudgetLimit::tokens(1_000).expect("limit"),
            false,
        ));

        assert_eq!(
            query.current().expect("progress").status,
            BudgetProgressStatus::AllDisabled
        );
    }

    #[test]
    fn available_progress_is_sorted_by_usage_and_tray_summary_is_preformatted() {
        let budgets = Arc::new(MemoryBudgetStore::default());
        budgets.budgets.lock().expect("budget lock").push(budget(
            1,
            BudgetLimit::tokens(1_000).expect("limit"),
            true,
        ));
        let query = query(budgets, usage(1_250, None));

        let progress = query.current().expect("progress");

        assert_eq!(progress.status, BudgetProgressStatus::Available);
        assert_eq!(progress.items[0].basis_points, Some(12_500));
        assert!(progress.items[0].exceeded);
        assert_eq!(
            progress.tray_summary.as_deref(),
            Some("Budget: Budget 125%")
        );
    }

    #[test]
    fn unavailable_cost_budget_preserves_explicit_state() {
        let budgets = Arc::new(MemoryBudgetStore::default());
        budgets.budgets.lock().expect("budget lock").push(budget(
            1,
            BudgetLimit::cost_micros(10_000_000, CurrencyCode::new("USD").expect("currency"))
                .expect("limit"),
            true,
        ));
        let query = query(
            budgets,
            BudgetUsageAggregate {
                total_tokens: 0,
                active_days: 1,
                cost: BudgetUsageCost {
                    amount_micros: None,
                    currency: None,
                    completeness: BudgetCostCompleteness::Unavailable,
                    unavailable_days: 1,
                },
            },
        );

        let progress = query.current().expect("progress");

        assert_eq!(
            progress.items[0].state,
            BudgetProgressItemState::CostUnavailable
        );
        assert_eq!(progress.items[0].basis_points, None);
        assert_eq!(
            progress.tray_summary.as_deref(),
            Some("Budget: Budget cost unavailable")
        );
    }

    fn query(budgets: Arc<MemoryBudgetStore>, usage: BudgetUsageAggregate) -> BudgetProgressQuery {
        BudgetProgressQuery::new(
            budgets,
            Arc::new(FixedUsageStore(Mutex::new(usage))),
            Arc::new(FixedSettingsStore),
            Arc::new(FixedClock),
        )
    }

    fn usage(total_tokens: u64, cost_micros: Option<u64>) -> BudgetUsageAggregate {
        BudgetUsageAggregate {
            total_tokens,
            active_days: if total_tokens == 0 && cost_micros.is_none() {
                0
            } else {
                1
            },
            cost: BudgetUsageCost {
                amount_micros: cost_micros,
                currency: cost_micros.map(|_| CurrencyCode::new("USD").expect("currency")),
                completeness: if cost_micros.is_some() {
                    BudgetCostCompleteness::Complete
                } else {
                    BudgetCostCompleteness::Unavailable
                },
                unavailable_days: 0,
            },
        }
    }

    fn budget(id: i64, limit: BudgetLimit, enabled: bool) -> Budget {
        let definition = BudgetDefinition::new(
            "Budget",
            limit,
            BudgetPeriod::Monthly,
            BudgetScope::Global,
            enabled,
            vec![BudgetThreshold::new(8_000, true).expect("threshold")],
        )
        .expect("definition");
        Budget::new(BudgetId::new(id).expect("id"), 1, definition).expect("budget")
    }
}
