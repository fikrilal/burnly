//! Application-owned read port for budget usage aggregation.

use thiserror::Error;

use crate::application::budget_evaluation::{BudgetUsageAggregate, BudgetUsageRequest};

pub(crate) trait BudgetUsageStore: Send + Sync {
    fn aggregate_budget_usage(
        &self,
        request: &BudgetUsageRequest,
    ) -> Result<BudgetUsageAggregate, BudgetUsageStoreError>;
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BudgetUsageStoreError {
    #[error("a budget usage value exceeded the supported integer range")]
    ValueOutOfRange,
    #[error("budget usage cost contains multiple currencies")]
    MixedCurrencies,
    #[error("the budget usage store backend failed")]
    Backend,
}
