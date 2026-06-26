use chrono::DateTime;
use serde::Serialize;
use std::sync::Arc;
use tauri::{Emitter, Manager, State};

use crate::application::bootstrap::{
    AppBootstrap, AppCapabilities, BootstrapError, BootstrapErrorKind, BootstrapService,
    Capability, CapabilityStatus, DatabaseState, ExportFormat, FeatureSummary,
    NativeNotificationCapability, Readiness, RefreshState, RefreshStatus, SourceStatus,
    SourceSummary,
};
use crate::application::ports::notification::NotificationPermission;
use crate::application::ports::window_actions::WindowActions;
use crate::application::reconciliation::RefreshTrigger;
use crate::application::refresh::{
    RefreshCoordinator, RefreshEventSink, RefreshSnapshot, RefreshStatus as RefreshLifecycleStatus,
};

use super::response::{ErrorCategory, IpcError, IpcResponse, CONTRACT_VERSION};
use super::settings::SettingsResponse;

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
    settings: SettingsResponse,
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
    native_notifications: NativeNotificationCapabilityResponse,
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
struct NativeNotificationCapabilityResponse {
    supported: bool,
    status: &'static str,
    permission: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticCapabilitiesResponse {
    desktop_evidence: bool,
}

#[tauri::command]
pub(super) fn app_get_bootstrap<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> IpcResponse<AppBootstrapResponse> {
    let Some(service) = app.try_state::<BootstrapService>() else {
        return IpcResponse::failure(IpcError::new(
            "bootstrap.storage_unavailable",
            "Burnly could not read local application state.",
            ErrorCategory::Unavailable,
            true,
        ));
    };
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct HideTrayPanelResponse {
    status: &'static str,
}

#[tauri::command]
pub(super) fn app_hide_tray_panel(
    window_actions: State<'_, Arc<dyn WindowActions>>,
) -> IpcResponse<HideTrayPanelResponse> {
    match window_actions.hide_tray_panel() {
        Ok(()) => IpcResponse::success(HideTrayPanelResponse { status: "hidden" }),
        Err(_) => IpcResponse::failure(IpcError::new(
            "app.hide_tray_panel_failed",
            "Burnly could not hide the tray panel.",
            ErrorCategory::Platform,
            true,
        )),
    }
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

impl From<NativeNotificationCapability> for NativeNotificationCapabilityResponse {
    fn from(value: NativeNotificationCapability) -> Self {
        Self {
            supported: value.supported,
            status: capability_status_label(value.status),
            permission: notification_permission_label(value.permission),
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
        CapabilityStatus::Available => "available",
        CapabilityStatus::NotImplemented => "not_implemented",
        CapabilityStatus::Unavailable => "unavailable",
    }
}

fn notification_permission_label(value: NotificationPermission) -> &'static str {
    match value {
        NotificationPermission::Granted => "granted",
        NotificationPermission::Denied => "denied",
        NotificationPermission::Prompt => "prompt",
        NotificationPermission::Unknown => "unknown",
    }
}

fn export_format_label(value: ExportFormat) -> String {
    match value {
        ExportFormat::Csv => "csv".to_owned(),
    }
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RefreshStatusResponse {
    status: &'static str,
    job_id: Option<String>,
    trigger: Option<&'static str>,
    last_successful_refresh_at: Option<String>,
}

#[tauri::command]
pub(super) fn refresh_get_state(
    coordinator: State<'_, RefreshCoordinator>,
) -> IpcResponse<RefreshStatusResponse> {
    IpcResponse::success(coordinator.snapshot().into())
}

#[tauri::command]
pub(super) fn refresh_request(
    coordinator: State<'_, RefreshCoordinator>,
) -> IpcResponse<RefreshStatusResponse> {
    let snapshot = coordinator.request_refresh(RefreshTrigger::Manual);
    IpcResponse::success(snapshot.into())
}

#[tauri::command]
pub(super) fn refresh_cancel(
    coordinator: State<'_, RefreshCoordinator>,
) -> IpcResponse<RefreshStatusResponse> {
    IpcResponse::success(coordinator.cancel().into())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RefreshProgressEvent {
    status: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DataInvalidatedEvent {
    scope: &'static str,
}

/// Publishes refresh notifications. Events carry only hints; the frontend must
/// re-query authoritative state after `data-invalidated`.
struct TauriRefreshEventSink<R: tauri::Runtime> {
    app: tauri::AppHandle<R>,
}

impl<R: tauri::Runtime> RefreshEventSink for TauriRefreshEventSink<R> {
    fn publish(&self, snapshot: RefreshSnapshot, usage_changed: bool) {
        let _ = self.app.emit(
            "burnly://v1/refresh-progress",
            RefreshProgressEvent {
                status: refresh_lifecycle_value(snapshot.status),
            },
        );

        if usage_changed {
            let _ = self.app.emit(
                "burnly://v1/data-invalidated",
                DataInvalidatedEvent { scope: "usage" },
            );
        }
    }
}

pub(crate) fn refresh_event_sink<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Arc<dyn RefreshEventSink> {
    Arc::new(TauriRefreshEventSink { app })
}

impl From<RefreshSnapshot> for RefreshStatusResponse {
    fn from(value: RefreshSnapshot) -> Self {
        Self {
            status: refresh_lifecycle_value(value.status),
            job_id: value.job_id,
            trigger: value.trigger.map(refresh_trigger_value),
            last_successful_refresh_at: value.last_successful_refresh_at_ms.map(to_rfc3339),
        }
    }
}

const fn refresh_lifecycle_value(status: RefreshLifecycleStatus) -> &'static str {
    match status {
        RefreshLifecycleStatus::Idle => "idle",
        RefreshLifecycleStatus::Queued => "queued",
        RefreshLifecycleStatus::Running => "running",
        RefreshLifecycleStatus::Cancelling => "cancelling",
        RefreshLifecycleStatus::Succeeded => "succeeded",
        RefreshLifecycleStatus::Partial => "partial",
        RefreshLifecycleStatus::Failed => "failed",
    }
}

const fn refresh_trigger_value(trigger: RefreshTrigger) -> &'static str {
    match trigger {
        RefreshTrigger::Launch => "launch",
        RefreshTrigger::Manual => "manual",
        RefreshTrigger::Scheduled => "scheduled",
        RefreshTrigger::FileChange => "file_change",
        RefreshTrigger::Resume => "resume",
        RefreshTrigger::Reconcile => "reconcile",
    }
}

fn to_rfc3339(epoch_ms: i64) -> String {
    DateTime::from_timestamp_millis(epoch_ms)
        .map(|timestamp| timestamp.to_rfc3339())
        .unwrap_or_default()
}
