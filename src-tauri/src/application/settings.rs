use std::sync::Arc;

use crate::application::ports::clock::Clock;
use crate::application::ports::settings_store::{SettingsStore, SettingsStoreError};
use crate::domain::settings::{Settings, SettingsDocument, SettingsValidationError};

pub(crate) trait SettingsRuntime: Send + Sync {
    fn validate(&self, current: &Settings, proposed: &Settings) -> Result<(), RuntimeSettingError>;
    fn apply(&self, settings: &Settings);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeSettingError {
    LaunchAtLoginUnavailable,
}

pub(crate) struct SettingsService {
    store: Arc<dyn SettingsStore>,
    runtime: Arc<dyn SettingsRuntime>,
    clock: Arc<dyn Clock>,
}

impl SettingsService {
    pub(crate) fn new(
        store: Arc<dyn SettingsStore>,
        runtime: Arc<dyn SettingsRuntime>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            store,
            runtime,
            clock,
        }
    }

    pub(crate) fn get(&self) -> Result<SettingsDocument, SettingsError> {
        self.store.get().map_err(SettingsError::from_store)
    }

    pub(crate) fn update(
        &self,
        expected_revision: i64,
        proposed: Settings,
    ) -> Result<SettingsDocument, SettingsError> {
        if expected_revision <= 0 {
            return Err(SettingsError::Validation(SettingsValidationError::Revision));
        }

        let current = self.store.get().map_err(SettingsError::from_store)?;
        self.runtime
            .validate(current.settings(), &proposed)
            .map_err(SettingsError::Runtime)?;
        let updated = self
            .store
            .replace(expected_revision, &proposed, self.clock.now_epoch_ms())
            .map_err(SettingsError::from_store)?;
        self.runtime.apply(updated.settings());
        Ok(updated)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SettingsError {
    Validation(SettingsValidationError),
    Conflict,
    StorageUnavailable,
    InvalidStoredValue,
    Runtime(RuntimeSettingError),
}

impl SettingsError {
    fn from_store(error: SettingsStoreError) -> Self {
        match error {
            SettingsStoreError::Conflict => Self::Conflict,
            SettingsStoreError::Unavailable => Self::StorageUnavailable,
            SettingsStoreError::InvalidStoredValue => Self::InvalidStoredValue,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::domain::settings::Settings;

    struct FixedClock;

    impl Clock for FixedClock {
        fn now_epoch_ms(&self) -> i64 {
            200
        }
    }

    struct MemoryStore {
        document: Mutex<SettingsDocument>,
    }

    impl SettingsStore for MemoryStore {
        fn get(&self) -> Result<SettingsDocument, SettingsStoreError> {
            Ok(self.document.lock().expect("settings lock").clone())
        }

        fn replace(
            &self,
            expected_revision: i64,
            settings: &Settings,
            _updated_at_ms: i64,
        ) -> Result<SettingsDocument, SettingsStoreError> {
            let mut document = self.document.lock().expect("settings lock");
            if document.revision() != expected_revision {
                return Err(SettingsStoreError::Conflict);
            }
            *document = SettingsDocument::new(settings.clone(), expected_revision + 1)
                .expect("valid revision");
            Ok(document.clone())
        }
    }

    #[derive(Default)]
    struct RecordingRuntime {
        applied: Mutex<Vec<Settings>>,
    }

    impl SettingsRuntime for RecordingRuntime {
        fn validate(
            &self,
            _current: &Settings,
            _proposed: &Settings,
        ) -> Result<(), RuntimeSettingError> {
            Ok(())
        }

        fn apply(&self, settings: &Settings) {
            self.applied
                .lock()
                .expect("runtime lock")
                .push(settings.clone());
        }
    }

    fn settings(close_behavior: &str) -> Settings {
        Settings::new(false, close_behavior).expect("valid settings")
    }

    #[test]
    fn update_persists_then_applies_runtime_settings() {
        let runtime = Arc::new(RecordingRuntime::default());
        let service = SettingsService::new(
            Arc::new(MemoryStore {
                document: Mutex::new(SettingsDocument::new(settings("quit"), 1).expect("document")),
            }),
            runtime.clone(),
            Arc::new(FixedClock),
        );

        let updated = service.update(1, settings("hide")).expect("update");

        assert_eq!(updated.revision(), 2);
        assert_eq!(
            runtime.applied.lock().expect("runtime lock").as_slice(),
            &[settings("hide")]
        );
    }

    #[test]
    fn stale_revision_does_not_apply_runtime_settings() {
        let runtime = Arc::new(RecordingRuntime::default());
        let service = SettingsService::new(
            Arc::new(MemoryStore {
                document: Mutex::new(SettingsDocument::new(settings("quit"), 2).expect("document")),
            }),
            runtime.clone(),
            Arc::new(FixedClock),
        );

        assert_eq!(
            service.update(1, settings("hide")),
            Err(SettingsError::Conflict)
        );
        assert!(runtime.applied.lock().expect("runtime lock").is_empty());
    }
}
