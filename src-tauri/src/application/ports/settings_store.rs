use crate::domain::settings::{Settings, SettingsDocument};

pub(crate) trait SettingsStore: Send + Sync {
    fn get(&self) -> Result<SettingsDocument, SettingsStoreError>;

    fn replace(
        &self,
        expected_revision: i64,
        settings: &Settings,
        updated_at_ms: i64,
    ) -> Result<SettingsDocument, SettingsStoreError>;

    fn replace_project_path_retention(
        &self,
        expected_revision: i64,
        retain_paths: bool,
        updated_at_ms: i64,
    ) -> Result<ProjectPathRetentionResult, SettingsStoreError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProjectPathRetentionResult {
    pub settings: SettingsDocument,
    pub cleared_paths: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SettingsStoreError {
    Conflict,
    Unavailable,
    InvalidStoredValue,
}
