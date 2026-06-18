use std::sync::Arc;

use crate::application::ports::budget_store::{BudgetStore, BudgetStoreError};
use crate::application::ports::clock::Clock;
use crate::domain::budget::{Budget, BudgetDefinition, BudgetId, BudgetValidationError};

pub(crate) struct BudgetService {
    store: Arc<dyn BudgetStore>,
    clock: Arc<dyn Clock>,
}

impl BudgetService {
    pub(crate) fn new(store: Arc<dyn BudgetStore>, clock: Arc<dyn Clock>) -> Self {
        Self { store, clock }
    }

    pub(crate) fn create(&self, definition: BudgetDefinition) -> Result<Budget, BudgetError> {
        self.store
            .create(&definition, self.clock.now_epoch_ms())
            .map_err(BudgetError::from_store)
    }

    pub(crate) fn get(&self, id: BudgetId) -> Result<Budget, BudgetError> {
        self.store.get(id).map_err(BudgetError::from_store)
    }

    pub(crate) fn list(&self) -> Result<Vec<Budget>, BudgetError> {
        self.store.list().map_err(BudgetError::from_store)
    }

    pub(crate) fn update(
        &self,
        id: BudgetId,
        expected_revision: i64,
        definition: BudgetDefinition,
    ) -> Result<Budget, BudgetError> {
        validate_revision(expected_revision)?;
        self.store
            .replace(
                id,
                expected_revision,
                &definition,
                self.clock.now_epoch_ms(),
            )
            .map_err(BudgetError::from_store)
    }

    pub(crate) fn enable(
        &self,
        id: BudgetId,
        expected_revision: i64,
    ) -> Result<Budget, BudgetError> {
        self.set_enabled(id, expected_revision, true)
    }

    pub(crate) fn disable(
        &self,
        id: BudgetId,
        expected_revision: i64,
    ) -> Result<Budget, BudgetError> {
        self.set_enabled(id, expected_revision, false)
    }

    pub(crate) fn delete(&self, id: BudgetId, expected_revision: i64) -> Result<(), BudgetError> {
        validate_revision(expected_revision)?;
        self.store
            .delete(id, expected_revision)
            .map_err(BudgetError::from_store)
    }

    fn set_enabled(
        &self,
        id: BudgetId,
        expected_revision: i64,
        enabled: bool,
    ) -> Result<Budget, BudgetError> {
        validate_revision(expected_revision)?;
        self.store
            .set_enabled(id, expected_revision, enabled, self.clock.now_epoch_ms())
            .map_err(BudgetError::from_store)
    }
}

fn validate_revision(revision: i64) -> Result<(), BudgetError> {
    if revision <= 0 {
        return Err(BudgetError::Validation(BudgetValidationError::Revision));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BudgetError {
    Validation(BudgetValidationError),
    NotFound,
    Conflict,
    UnknownSource,
    StorageUnavailable,
    InvalidStoredValue,
}

impl BudgetError {
    fn from_store(error: BudgetStoreError) -> Self {
        match error {
            BudgetStoreError::NotFound => Self::NotFound,
            BudgetStoreError::Conflict => Self::Conflict,
            BudgetStoreError::UnknownSource => Self::UnknownSource,
            BudgetStoreError::Unavailable => Self::StorageUnavailable,
            BudgetStoreError::InvalidStoredValue => Self::InvalidStoredValue,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::domain::budget::{BudgetLimit, BudgetPeriod, BudgetScope};

    struct FixedClock;

    impl Clock for FixedClock {
        fn now_epoch_ms(&self) -> i64 {
            200
        }
    }

    #[derive(Default)]
    struct RecordingStore {
        enabled: Mutex<Vec<(i64, bool, i64)>>,
    }

    impl BudgetStore for RecordingStore {
        fn create(
            &self,
            definition: &BudgetDefinition,
            _now_epoch_ms: i64,
        ) -> Result<Budget, BudgetStoreError> {
            budget(1, definition.clone())
        }

        fn get(&self, _id: BudgetId) -> Result<Budget, BudgetStoreError> {
            budget(1, definition())
        }

        fn list(&self) -> Result<Vec<Budget>, BudgetStoreError> {
            Ok(vec![budget(1, definition())?])
        }

        fn replace(
            &self,
            _id: BudgetId,
            expected_revision: i64,
            definition: &BudgetDefinition,
            _now_epoch_ms: i64,
        ) -> Result<Budget, BudgetStoreError> {
            budget(expected_revision + 1, definition.clone())
        }

        fn set_enabled(
            &self,
            _id: BudgetId,
            expected_revision: i64,
            enabled: bool,
            now_epoch_ms: i64,
        ) -> Result<Budget, BudgetStoreError> {
            self.enabled.lock().expect("enabled lock").push((
                expected_revision,
                enabled,
                now_epoch_ms,
            ));
            budget(expected_revision + 1, definition().with_enabled(enabled))
        }

        fn delete(&self, _id: BudgetId, _expected_revision: i64) -> Result<(), BudgetStoreError> {
            Ok(())
        }
    }

    fn definition() -> BudgetDefinition {
        BudgetDefinition::new(
            "Daily tokens",
            BudgetLimit::tokens(10_000).expect("limit"),
            BudgetPeriod::Daily,
            BudgetScope::Global,
            true,
            vec![],
        )
        .expect("definition")
    }

    fn budget(revision: i64, definition: BudgetDefinition) -> Result<Budget, BudgetStoreError> {
        Budget::new(BudgetId::new(1).expect("budget ID"), revision, definition)
            .map_err(|_| BudgetStoreError::InvalidStoredValue)
    }

    #[test]
    fn enable_and_disable_are_explicit_revision_checked_commands() {
        let store = Arc::new(RecordingStore::default());
        let service = BudgetService::new(store.clone(), Arc::new(FixedClock));
        let id = BudgetId::new(1).expect("budget ID");

        assert!(service
            .enable(id, 1)
            .expect("enable")
            .definition()
            .enabled());
        assert!(!service
            .disable(id, 2)
            .expect("disable")
            .definition()
            .enabled());
        assert_eq!(
            store.enabled.lock().expect("enabled lock").as_slice(),
            &[(1, true, 200), (2, false, 200)]
        );
    }

    #[test]
    fn invalid_revision_is_rejected_before_store_mutation() {
        let store = Arc::new(RecordingStore::default());
        let service = BudgetService::new(store.clone(), Arc::new(FixedClock));

        assert_eq!(
            service.delete(BudgetId::new(1).expect("budget ID"), 0),
            Err(BudgetError::Validation(BudgetValidationError::Revision))
        );
        assert!(store.enabled.lock().expect("enabled lock").is_empty());
    }
}
