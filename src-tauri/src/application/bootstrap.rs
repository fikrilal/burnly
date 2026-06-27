use std::sync::{Arc, Mutex};

use thiserror::Error;

use crate::domain::settings::{CloseBehavior, Settings, SettingsDocument};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AppBootstrap {
    pub app_version: String,
    pub contract_version: u16,
    pub database: DatabaseState,
    pub settings: SettingsDocument,
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
    Available,
    NotImplemented,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExportFormat {
    Csv,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DiagnosticCapabilities {
    pub desktop_evidence: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BootstrapStorage {
    pub launch_at_login: bool,
    pub close_behavior: String,
    pub settings_revision: i64,
    pub schema_version: i64,
}

pub(crate) trait BootstrapStore: Send + Sync {
    fn read_bootstrap_storage(&self) -> Result<BootstrapStorage, BootstrapError>;
}

pub(crate) struct BootstrapService {
    app_version: &'static str,
    contract_version: u16,
    store: Box<dyn BootstrapStore>,
    runtime_capabilities: RuntimeCapabilities,
}

#[derive(Clone)]
pub(crate) struct RuntimeSettings {
    close_behavior: Arc<Mutex<CloseBehavior>>,
}

#[derive(Clone)]
pub(crate) struct RuntimeCapabilities {
    tray: Arc<Mutex<Capability>>,
}

impl RuntimeSettings {
    pub(crate) fn new(close_behavior: CloseBehavior) -> Self {
        Self {
            close_behavior: Arc::new(Mutex::new(close_behavior)),
        }
    }

    pub(crate) fn update(&self, settings: &Settings) {
        *self
            .close_behavior
            .lock()
            .expect("runtime settings lock is poisoned") = settings.close_behavior();
    }
}

impl BootstrapService {
    pub(crate) fn new(
        app_version: &'static str,
        contract_version: u16,
        store: impl BootstrapStore + 'static,
        runtime_capabilities: RuntimeCapabilities,
    ) -> Self {
        Self {
            app_version,
            contract_version,
            store: Box::new(store),
            runtime_capabilities,
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
            settings: SettingsDocument::new(
                Settings::new(storage.launch_at_login, &storage.close_behavior)
                    .map_err(|_| BootstrapError::storage_unavailable())?,
                storage.settings_revision,
            )
            .map_err(|_| BootstrapError::storage_unavailable())?,
            features: FeatureSummary {
                usage_overview: false,
                collector_refresh: false,
                budgets: true,
                settings: true,
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
            tray: self.runtime_capabilities.tray(),
            launch_at_login: unavailable.clone(),
            export_formats: vec![ExportFormat::Csv],
            diagnostics: DiagnosticCapabilities {
                desktop_evidence: true,
            },
        }
    }
}

impl RuntimeCapabilities {
    pub(crate) fn new(tray: Capability) -> Self {
        Self {
            tray: Arc::new(Mutex::new(tray)),
        }
    }

    pub(crate) fn tray_available() -> Capability {
        Capability {
            supported: true,
            status: CapabilityStatus::Available,
        }
    }

    pub(crate) fn tray_unavailable() -> Capability {
        Capability {
            supported: false,
            status: CapabilityStatus::Unavailable,
        }
    }

    pub(crate) fn tray(&self) -> Capability {
        self.tray
            .lock()
            .expect("runtime capabilities lock is poisoned")
            .clone()
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

    fn capabilities_without_tray() -> RuntimeCapabilities {
        RuntimeCapabilities::new(Capability {
            supported: false,
            status: CapabilityStatus::NotImplemented,
        })
    }

    struct FixedStore {
        storage: BootstrapStorage,
    }

    impl BootstrapStore for FixedStore {
        fn read_bootstrap_storage(&self) -> Result<BootstrapStorage, BootstrapError> {
            Ok(self.storage.clone())
        }
    }

    #[test]
    fn builds_bootstrap_from_persisted_storage_and_explicit_unimplemented_states() {
        let service = BootstrapService::new(
            "0.1.0",
            1,
            FixedStore {
                storage: BootstrapStorage {
                    launch_at_login: false,
                    close_behavior: "quit".to_owned(),
                    settings_revision: 1,
                    schema_version: 2,
                },
            },
            capabilities_without_tray(),
        );

        let bootstrap = service.bootstrap().expect("bootstrap state");

        assert_eq!(bootstrap.app_version, "0.1.0");
        assert_eq!(bootstrap.contract_version, 1);
        assert_eq!(bootstrap.database.status, Readiness::Ready);
        assert_eq!(bootstrap.database.schema_version, 2);
        assert!(!bootstrap.settings.settings().launch_at_login());
        assert_eq!(bootstrap.settings.revision(), 1);
        assert_eq!(bootstrap.sources.status, SourceStatus::NotConfigured);
        assert_eq!(bootstrap.refresh.status, RefreshStatus::Idle);
        assert!(!bootstrap.onboarding_complete);
        assert!(!bootstrap.features.collector_refresh);
        assert!(bootstrap.features.budgets);
    }

    #[test]
    fn exposes_truthful_build_capabilities_without_platform_names() {
        let service = BootstrapService::new(
            "0.1.0",
            1,
            FixedStore {
                storage: BootstrapStorage {
                    launch_at_login: false,
                    close_behavior: "quit".to_owned(),
                    settings_revision: 1,
                    schema_version: 2,
                },
            },
            RuntimeCapabilities::new(RuntimeCapabilities::tray_available()),
        );

        let capabilities = service.capabilities();

        assert!(capabilities.tray.supported);
        assert_eq!(capabilities.tray.status, CapabilityStatus::Available);
        assert_eq!(capabilities.export_formats, vec![ExportFormat::Csv]);
        assert!(capabilities.diagnostics.desktop_evidence);
    }
}
