use serde::Serialize;
use tauri::State;

use crate::application::diagnostics::{
    DiagnosticComponent, DiagnosticComponentKind, DiagnosticsService, DiagnosticsStatus,
    HealthStatus, LogDiagnostics,
};
use crate::application::ports::log_reveal::{
    LogRevealAvailability, LogRevealError, LogRevealOutcome,
};

use super::response::{ErrorCategory, IpcError, IpcResponse, CONTRACT_VERSION};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DiagnosticsStatusResponse {
    status: &'static str,
    contract_version: u16,
    components: Vec<DiagnosticComponentResponse>,
    logs: LogDiagnosticsResponse,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticComponentResponse {
    component: &'static str,
    status: &'static str,
    summary: String,
    details: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LogDiagnosticsResponse {
    status: &'static str,
    label: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RevealLogsResponse {
    status: &'static str,
    message: &'static str,
}

#[tauri::command]
pub(super) fn diagnostics_get_status(
    service: State<'_, DiagnosticsService>,
) -> IpcResponse<DiagnosticsStatusResponse> {
    IpcResponse::success(service.status().into())
}

#[tauri::command]
pub(super) fn diagnostics_reveal_logs(
    service: State<'_, DiagnosticsService>,
) -> IpcResponse<RevealLogsResponse> {
    match service.reveal_logs() {
        Ok(outcome) => IpcResponse::success(outcome.into()),
        Err(error) => IpcResponse::failure(log_reveal_error(error)),
    }
}

impl From<DiagnosticsStatus> for DiagnosticsStatusResponse {
    fn from(value: DiagnosticsStatus) -> Self {
        Self {
            status: health_status(value.status),
            contract_version: CONTRACT_VERSION,
            components: value.components.into_iter().map(Into::into).collect(),
            logs: value.logs.into(),
        }
    }
}

impl From<LogDiagnostics> for LogDiagnosticsResponse {
    fn from(value: LogDiagnostics) -> Self {
        Self {
            status: log_availability(value.status),
            label: value.label,
        }
    }
}

impl From<LogRevealOutcome> for RevealLogsResponse {
    fn from(value: LogRevealOutcome) -> Self {
        match value {
            LogRevealOutcome::Revealed => Self {
                status: "revealed",
                message: "Logs opened in the system file manager.",
            },
            LogRevealOutcome::Missing => Self {
                status: "missing",
                message: "No log folder exists yet.",
            },
            LogRevealOutcome::Unsupported => Self {
                status: "unsupported",
                message: "Opening logs is not supported on this platform.",
            },
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

const fn log_availability(value: LogRevealAvailability) -> &'static str {
    match value {
        LogRevealAvailability::Available => "available",
        LogRevealAvailability::Missing => "missing",
        LogRevealAvailability::Unsupported => "unsupported",
    }
}

fn log_reveal_error(error: LogRevealError) -> IpcError {
    match error {
        LogRevealError::Failed => IpcError::new(
            "diagnostics.logs_reveal_failed",
            "Burnly could not open the log location.",
            ErrorCategory::Platform,
            true,
        ),
    }
}
