use crate::domain::settings::{Settings, SettingsDocument};

pub(crate) trait SettingsStore: Send + Sync {
    fn get(&self) -> Result<SettingsDocument, SettingsStoreError>;

    fn replace(
        &self,
        expected_revision: i64,
        settings: &Settings,
        updated_at_ms: i64,
    ) -> Result<SettingsDocument, SettingsStoreError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SettingsStoreError {
    Conflict,
    Unavailable,
    InvalidStoredValue,
}
