use std::sync::Arc;

use crate::application::ports::clock::Clock;
use crate::application::ports::settings_store::{SettingsStore, SettingsStoreError};
use crate::domain::settings::{Settings, SettingsDocument, SettingsValidationError};

pub(crate) trait SettingsRuntime: Send + Sync {
    fn validate(&self, current: &Settings, proposed: &Settings) -> Result<(), RuntimeSettingError>;
    fn prepare_update(
        &self,
        current: &Settings,
        proposed: &Settings,
    ) -> Result<(), RuntimeSettingError>;
    fn rollback_update(&self, current: &Settings) -> Result<(), RuntimeSettingError>;
    fn commit_update(&self, settings: &Settings);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeSettingError {
    LaunchAtLoginUnavailable,
    LaunchAtLoginApplyFailed,
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
        if current.revision() != expected_revision {
            return Err(SettingsError::Conflict);
        }
        self.runtime
            .validate(current.settings(), &proposed)
            .map_err(SettingsError::Runtime)?;
        self.runtime
            .prepare_update(current.settings(), &proposed)
            .map_err(SettingsError::Runtime)?;

        match self
            .store
            .replace(expected_revision, &proposed, self.clock.now_epoch_ms())
        {
            Ok(updated) => {
                self.runtime.commit_update(updated.settings());
                Ok(updated)
            }
            Err(error) => {
                let _ = self.runtime.rollback_update(current.settings());
                Err(SettingsError::from_store(error))
            }
        }
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
        fail_replace: bool,
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
            if self.fail_replace {
                return Err(SettingsStoreError::Unavailable);
            }
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
        prepared: Mutex<Vec<Settings>>,
        committed: Mutex<Vec<Settings>>,
        rolled_back: Mutex<Vec<Settings>>,
        fail_prepare: bool,
    }

    impl SettingsRuntime for RecordingRuntime {
        fn validate(
            &self,
            _current: &Settings,
            _proposed: &Settings,
        ) -> Result<(), RuntimeSettingError> {
            Ok(())
        }

        fn prepare_update(
            &self,
            _current: &Settings,
            proposed: &Settings,
        ) -> Result<(), RuntimeSettingError> {
            if self.fail_prepare {
                return Err(RuntimeSettingError::LaunchAtLoginApplyFailed);
            }
            self.prepared
                .lock()
                .expect("runtime lock")
                .push(proposed.clone());
            Ok(())
        }

        fn rollback_update(&self, current: &Settings) -> Result<(), RuntimeSettingError> {
            self.rolled_back
                .lock()
                .expect("runtime lock")
                .push(current.clone());
            Ok(())
        }

        fn commit_update(&self, settings: &Settings) {
            self.committed
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
                fail_replace: false,
            }),
            runtime.clone(),
            Arc::new(FixedClock),
        );

        let updated = service.update(1, settings("hide")).expect("update");

        assert_eq!(updated.revision(), 2);
        assert_eq!(
            runtime.prepared.lock().expect("runtime lock").as_slice(),
            &[settings("hide")]
        );
        assert_eq!(
            runtime.committed.lock().expect("runtime lock").as_slice(),
            &[settings("hide")]
        );
        assert!(runtime.rolled_back.lock().expect("runtime lock").is_empty());
    }

    #[test]
    fn stale_revision_does_not_apply_runtime_settings() {
        let runtime = Arc::new(RecordingRuntime::default());
        let service = SettingsService::new(
            Arc::new(MemoryStore {
                document: Mutex::new(SettingsDocument::new(settings("quit"), 2).expect("document")),
                fail_replace: false,
            }),
            runtime.clone(),
            Arc::new(FixedClock),
        );

        assert_eq!(
            service.update(1, settings("hide")),
            Err(SettingsError::Conflict)
        );
        assert!(runtime.prepared.lock().expect("runtime lock").is_empty());
        assert!(runtime.committed.lock().expect("runtime lock").is_empty());
        assert!(runtime.rolled_back.lock().expect("runtime lock").is_empty());
    }

    #[test]
    fn runtime_apply_failure_does_not_persist_settings() {
        let runtime = Arc::new(RecordingRuntime {
            fail_prepare: true,
            ..RecordingRuntime::default()
        });
        let store = Arc::new(MemoryStore {
            document: Mutex::new(SettingsDocument::new(settings("quit"), 1).expect("document")),
            fail_replace: false,
        });
        let service = SettingsService::new(store.clone(), runtime, Arc::new(FixedClock));

        assert_eq!(
            service.update(1, settings("hide")),
            Err(SettingsError::Runtime(
                RuntimeSettingError::LaunchAtLoginApplyFailed
            ))
        );
        assert_eq!(store.get().expect("settings").settings(), &settings("quit"));
    }

    #[test]
    fn persistence_failure_rolls_back_runtime_update() {
        let runtime = Arc::new(RecordingRuntime::default());
        let service = SettingsService::new(
            Arc::new(MemoryStore {
                document: Mutex::new(SettingsDocument::new(settings("quit"), 1).expect("document")),
                fail_replace: true,
            }),
            runtime.clone(),
            Arc::new(FixedClock),
        );

        assert_eq!(
            service.update(1, settings("hide")),
            Err(SettingsError::StorageUnavailable)
        );
        assert_eq!(
            runtime.prepared.lock().expect("runtime lock").as_slice(),
            &[settings("hide")]
        );
        assert_eq!(
            runtime.rolled_back.lock().expect("runtime lock").as_slice(),
            &[settings("quit")]
        );
        assert!(runtime.committed.lock().expect("runtime lock").is_empty());
    }
}
