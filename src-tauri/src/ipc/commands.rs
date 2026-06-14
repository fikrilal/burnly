use serde::Serialize;
use tauri::State;

use crate::application::bootstrap::{
    AppBootstrap, AppCapabilities, BootstrapError, BootstrapErrorKind, BootstrapService,
    Capability, CapabilityStatus, DatabaseState, ExportFormat, FeatureSummary, Readiness,
    RefreshState, RefreshStatus, SettingsState, SourceStatus, SourceSummary,
};

use super::response::{ErrorCategory, IpcError, IpcResponse, CONTRACT_VERSION};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ContractProbeResponse {
    status: &'static str,
    contract_version: u16,
}

#[tauri::command]
pub(super) fn __burnly_contract_probe() -> IpcResponse<ContractProbeResponse> {
    IpcResponse::success(ContractProbeResponse {
        status: "ok",
        contract_version: CONTRACT_VERSION,
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AppBootstrapResponse {
    app_version: String,
    contract_version: u16,
    database: DatabaseStateResponse,
    settings: SettingsStateResponse,
    features: FeatureSummaryResponse,
    sources: SourceSummaryResponse,
    refresh: RefreshStateResponse,
    onboarding_complete: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DatabaseStateResponse {
    status: &'static str,
    schema_version: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SettingsStateResponse {
    reporting_timezone: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FeatureSummaryResponse {
    usage_overview: bool,
    collector_refresh: bool,
    budgets: bool,
    settings: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceSummaryResponse {
    status: &'static str,
    detected_count: u16,
    configured_count: u16,
    enabled_count: u16,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RefreshStateResponse {
    status: &'static str,
    current_job_id: Option<String>,
    last_successful_refresh_at: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AppCapabilitiesResponse {
    tray: CapabilityResponse,
    launch_at_login: CapabilityResponse,
    native_notifications: CapabilityResponse,
    updates: CapabilityResponse,
    export_formats: Vec<String>,
    diagnostics: DiagnosticCapabilitiesResponse,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CapabilityResponse {
    supported: bool,
    status: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticCapabilitiesResponse {
    desktop_evidence: bool,
}

#[tauri::command]
pub(super) fn app_get_bootstrap(
    service: State<'_, BootstrapService>,
) -> IpcResponse<AppBootstrapResponse> {
    match service.bootstrap() {
        Ok(bootstrap) => IpcResponse::success(bootstrap.into()),
        Err(error) => IpcResponse::failure(bootstrap_error(error)),
    }
}

#[tauri::command]
pub(super) fn app_get_capabilities(
    service: State<'_, BootstrapService>,
) -> IpcResponse<AppCapabilitiesResponse> {
    IpcResponse::success(service.capabilities().into())
}

impl From<AppBootstrap> for AppBootstrapResponse {
    fn from(value: AppBootstrap) -> Self {
        Self {
            app_version: value.app_version,
            contract_version: value.contract_version,
            database: value.database.into(),
            settings: value.settings.into(),
            features: value.features.into(),
            sources: value.sources.into(),
            refresh: value.refresh.into(),
            onboarding_complete: value.onboarding_complete,
        }
    }
}

impl From<DatabaseState> for DatabaseStateResponse {
    fn from(value: DatabaseState) -> Self {
        Self {
            status: readiness_label(value.status),
            schema_version: value.schema_version,
        }
    }
}

impl From<SettingsState> for SettingsStateResponse {
    fn from(value: SettingsState) -> Self {
        Self {
            reporting_timezone: value.reporting_timezone,
        }
    }
}

impl From<FeatureSummary> for FeatureSummaryResponse {
    fn from(value: FeatureSummary) -> Self {
        Self {
            usage_overview: value.usage_overview,
            collector_refresh: value.collector_refresh,
            budgets: value.budgets,
            settings: value.settings,
        }
    }
}

impl From<SourceSummary> for SourceSummaryResponse {
    fn from(value: SourceSummary) -> Self {
        Self {
            status: source_status_label(value.status),
            detected_count: value.detected_count,
            configured_count: value.configured_count,
            enabled_count: value.enabled_count,
        }
    }
}

impl From<RefreshState> for RefreshStateResponse {
    fn from(value: RefreshState) -> Self {
        Self {
            status: refresh_status_label(value.status),
            current_job_id: value.current_job_id,
            last_successful_refresh_at: value.last_successful_refresh_at,
        }
    }
}

impl From<AppCapabilities> for AppCapabilitiesResponse {
    fn from(value: AppCapabilities) -> Self {
        Self {
            tray: value.tray.into(),
            launch_at_login: value.launch_at_login.into(),
            native_notifications: value.native_notifications.into(),
            updates: value.updates.into(),
            export_formats: value
                .export_formats
                .into_iter()
                .map(export_format_label)
                .collect(),
            diagnostics: DiagnosticCapabilitiesResponse {
                desktop_evidence: value.diagnostics.desktop_evidence,
            },
        }
    }
}

impl From<Capability> for CapabilityResponse {
    fn from(value: Capability) -> Self {
        Self {
            supported: value.supported,
            status: capability_status_label(value.status),
        }
    }
}

fn readiness_label(value: Readiness) -> &'static str {
    match value {
        Readiness::Ready => "ready",
    }
}

fn source_status_label(value: SourceStatus) -> &'static str {
    match value {
        SourceStatus::NotConfigured => "not_configured",
    }
}

fn refresh_status_label(value: RefreshStatus) -> &'static str {
    match value {
        RefreshStatus::Idle => "idle",
    }
}

fn capability_status_label(value: CapabilityStatus) -> &'static str {
    match value {
        CapabilityStatus::NotImplemented => "not_implemented",
    }
}

fn export_format_label(value: ExportFormat) -> String {
    match value {}
}

fn bootstrap_error(error: BootstrapError) -> IpcError {
    match error.kind() {
        BootstrapErrorKind::StorageUnavailable => IpcError::new(
            "bootstrap.storage_unavailable",
            "Burnly could not read local application state.",
            ErrorCategory::Persistence,
            true,
        ),
    }
}
