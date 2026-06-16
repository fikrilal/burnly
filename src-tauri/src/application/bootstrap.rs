use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AppBootstrap {
    pub app_version: String,
    pub contract_version: u16,
    pub database: DatabaseState,
    pub settings: SettingsState,
    pub features: FeatureSummary,
    pub sources: SourceSummary,
    pub refresh: RefreshState,
    pub onboarding_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DatabaseState {
    pub status: Readiness,
    pub schema_version: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Readiness {
    Ready,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SettingsState {
    pub reporting_timezone: String,
    pub background_refresh_enabled: bool,
    pub refresh_interval_minutes: i64,
    pub launch_at_login: bool,
    pub close_behavior: String,
    pub notifications_enabled: bool,
    pub store_project_paths: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FeatureSummary {
    pub usage_overview: bool,
    pub collector_refresh: bool,
    pub budgets: bool,
    pub settings: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceSummary {
    pub status: SourceStatus,
    pub detected_count: u16,
    pub configured_count: u16,
    pub enabled_count: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceStatus {
    NotConfigured,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RefreshState {
    pub status: RefreshStatus,
    pub current_job_id: Option<String>,
    pub last_successful_refresh_at: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RefreshStatus {
    Idle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AppCapabilities {
    pub tray: Capability,
    pub launch_at_login: Capability,
    pub native_notifications: Capability,
    pub updates: Capability,
    pub export_formats: Vec<ExportFormat>,
    pub diagnostics: DiagnosticCapabilities,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Capability {
    pub supported: bool,
    pub status: CapabilityStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CapabilityStatus {
    NotImplemented,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExportFormat {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DiagnosticCapabilities {
    pub desktop_evidence: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BootstrapStorage {
    pub reporting_timezone: String,
    pub background_refresh_enabled: bool,
    pub refresh_interval_minutes: i64,
    pub launch_at_login: bool,
    pub close_behavior: String,
    pub notifications_enabled: bool,
    pub store_project_paths: bool,
    pub schema_version: i64,
}

pub(crate) trait BootstrapStore: Send + Sync {
    fn read_bootstrap_storage(&self) -> Result<BootstrapStorage, BootstrapError>;
    fn update_settings(&self, settings: &SettingsState) -> Result<(), BootstrapError>;
}

pub(crate) struct BootstrapService {
    app_version: &'static str,
    contract_version: u16,
    store: Box<dyn BootstrapStore>,
}

impl BootstrapService {
    pub(crate) fn new(
        app_version: &'static str,
        contract_version: u16,
        store: impl BootstrapStore + 'static,
    ) -> Self {
        Self {
            app_version,
            contract_version,
            store: Box::new(store),
        }
    }

    pub(crate) fn bootstrap(&self) -> Result<AppBootstrap, BootstrapError> {
        let storage = self.store.read_bootstrap_storage()?;

        Ok(AppBootstrap {
            app_version: self.app_version.to_owned(),
            contract_version: self.contract_version,
            database: DatabaseState {
                status: Readiness::Ready,
                schema_version: storage.schema_version,
            },
            settings: SettingsState {
                reporting_timezone: storage.reporting_timezone,
                background_refresh_enabled: storage.background_refresh_enabled,
                refresh_interval_minutes: storage.refresh_interval_minutes,
                launch_at_login: storage.launch_at_login,
                close_behavior: storage.close_behavior,
                notifications_enabled: storage.notifications_enabled,
                store_project_paths: storage.store_project_paths,
            },
            features: FeatureSummary {
                usage_overview: false,
                collector_refresh: false,
                budgets: false,
                settings: false,
            },
            sources: SourceSummary {
                status: SourceStatus::NotConfigured,
                detected_count: 0,
                configured_count: 0,
                enabled_count: 0,
            },
            refresh: RefreshState {
                status: RefreshStatus::Idle,
                current_job_id: None,
                last_successful_refresh_at: None,
            },
            onboarding_complete: false,
        })
    }

    pub(crate) fn capabilities(&self) -> AppCapabilities {
        let unavailable = Capability {
            supported: false,
            status: CapabilityStatus::NotImplemented,
        };

        AppCapabilities {
            tray: unavailable.clone(),
            launch_at_login: unavailable.clone(),
            native_notifications: unavailable.clone(),
            updates: unavailable,
            export_formats: Vec::new(),
            diagnostics: DiagnosticCapabilities {
                desktop_evidence: true,
            },
        }
    }

    pub(crate) fn update_settings(&self, settings: SettingsState) -> Result<(), BootstrapError> {
        self.store.update_settings(&settings)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BootstrapErrorKind {
    StorageUnavailable,
}

#[derive(Debug, Error)]
#[error("bootstrap storage is unavailable")]
pub(crate) struct BootstrapError {
    kind: BootstrapErrorKind,
}

impl BootstrapError {
    pub(crate) fn storage_unavailable() -> Self {
        Self {
            kind: BootstrapErrorKind::StorageUnavailable,
        }
    }

    pub(crate) fn kind(&self) -> BootstrapErrorKind {
        self.kind
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FixedStore {
        storage: BootstrapStorage,
    }

    impl BootstrapStore for FixedStore {
        fn read_bootstrap_storage(&self) -> Result<BootstrapStorage, BootstrapError> {
            Ok(self.storage.clone())
        }

        fn update_settings(&self, _settings: &SettingsState) -> Result<(), BootstrapError> {
            Ok(())
        }
    }

    #[test]
    fn builds_bootstrap_from_persisted_storage_and_explicit_unimplemented_states() {
        let service = BootstrapService::new(
            "0.1.0",
            1,
            FixedStore {
                storage: BootstrapStorage {
                    reporting_timezone: "Asia/Jakarta".to_owned(),
                    background_refresh_enabled: false,
                    refresh_interval_minutes: 15,
                    launch_at_login: false,
                    close_behavior: "quit".to_owned(),
                    notifications_enabled: false,
                    store_project_paths: false,
                    schema_version: 1,
                },
            },
        );

        let bootstrap = service.bootstrap().expect("bootstrap state");

        assert_eq!(bootstrap.app_version, "0.1.0");
        assert_eq!(bootstrap.contract_version, 1);
        assert_eq!(bootstrap.database.status, Readiness::Ready);
        assert_eq!(bootstrap.database.schema_version, 1);
        assert_eq!(bootstrap.settings.reporting_timezone, "Asia/Jakarta");
        assert_eq!(bootstrap.sources.status, SourceStatus::NotConfigured);
        assert_eq!(bootstrap.refresh.status, RefreshStatus::Idle);
        assert!(!bootstrap.onboarding_complete);
        assert!(!bootstrap.features.collector_refresh);
    }

    #[test]
    fn exposes_truthful_build_capabilities_without_platform_names() {
        let service = BootstrapService::new(
            "0.1.0",
            1,
            FixedStore {
                storage: BootstrapStorage {
                    reporting_timezone: "UTC".to_owned(),
                    background_refresh_enabled: false,
                    refresh_interval_minutes: 15,
                    launch_at_login: false,
                    close_behavior: "quit".to_owned(),
                    notifications_enabled: false,
                    store_project_paths: false,
                    schema_version: 1,
                },
            },
        );

        let capabilities = service.capabilities();

        assert!(!capabilities.tray.supported);
        assert_eq!(capabilities.tray.status, CapabilityStatus::NotImplemented);
        assert!(capabilities.export_formats.is_empty());
        assert!(capabilities.diagnostics.desktop_evidence);
    }
}
