use serde::Serialize;
use tauri::State;

use crate::application::diagnostics::{
    DiagnosticComponent, DiagnosticComponentKind, DiagnosticsService, DiagnosticsStatus,
    HealthStatus,
};

use super::response::{IpcResponse, CONTRACT_VERSION};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DiagnosticsStatusResponse {
    status: &'static str,
    contract_version: u16,
    components: Vec<DiagnosticComponentResponse>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticComponentResponse {
    component: &'static str,
    status: &'static str,
    summary: String,
    details: Vec<String>,
}

#[tauri::command]
pub(super) fn diagnostics_get_status(
    service: State<'_, DiagnosticsService>,
) -> IpcResponse<DiagnosticsStatusResponse> {
    IpcResponse::success(service.status().into())
}

impl From<DiagnosticsStatus> for DiagnosticsStatusResponse {
    fn from(value: DiagnosticsStatus) -> Self {
        Self {
            status: health_status(value.status),
            contract_version: CONTRACT_VERSION,
            components: value.components.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<DiagnosticComponent> for DiagnosticComponentResponse {
    fn from(value: DiagnosticComponent) -> Self {
        Self {
            component: component_kind(value.component),
            status: health_status(value.status),
            summary: value.summary,
            details: value.details,
        }
    }
}

const fn component_kind(value: DiagnosticComponentKind) -> &'static str {
    match value {
        DiagnosticComponentKind::Database => "database",
        DiagnosticComponentKind::Settings => "settings",
        DiagnosticComponentKind::Sources => "sources",
        DiagnosticComponentKind::Collector => "collector",
        DiagnosticComponentKind::Runtime => "runtime",
    }
}

const fn health_status(value: HealthStatus) -> &'static str {
    match value {
        HealthStatus::Healthy => "healthy",
        HealthStatus::Degraded => "degraded",
        HealthStatus::Unavailable => "unavailable",
        HealthStatus::Unknown => "unknown",
    }
}
