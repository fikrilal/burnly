use crate::domain::budget::{Budget, BudgetDefinition, BudgetId};

pub(crate) trait BudgetStore: Send + Sync {
    fn create(
        &self,
        definition: &BudgetDefinition,
        now_epoch_ms: i64,
    ) -> Result<Budget, BudgetStoreError>;

    fn get(&self, id: BudgetId) -> Result<Budget, BudgetStoreError>;

    fn list(&self) -> Result<Vec<Budget>, BudgetStoreError>;

    fn replace(
        &self,
        id: BudgetId,
        expected_revision: i64,
        definition: &BudgetDefinition,
        now_epoch_ms: i64,
    ) -> Result<Budget, BudgetStoreError>;

    fn set_enabled(
        &self,
        id: BudgetId,
        expected_revision: i64,
        enabled: bool,
        now_epoch_ms: i64,
    ) -> Result<Budget, BudgetStoreError>;

    fn delete(&self, id: BudgetId, expected_revision: i64) -> Result<(), BudgetStoreError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BudgetStoreError {
    NotFound,
    Conflict,
    UnknownSource,
    Unavailable,
    InvalidStoredValue,
}
