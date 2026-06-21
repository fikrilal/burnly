use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::application::diagnostics::{
    DiagnosticComponent, DiagnosticComponentKind, DiagnosticsService, DiagnosticsStatus,
    HealthStatus, LogDiagnostics,
};
use crate::application::history::{
    FailureCategory, HistoryError, HistoryPage, HistoryProjection, HistoryRequest, HistoryScope,
    HistoryService, HistoryStatus, HistoryTrigger, ImportHistoryItem, RefreshHistoryItem,
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct HistoryCommandRequest {
    cursor: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct HistoryResponse {
    items: Vec<RefreshHistoryResponse>,
    next_cursor: Option<String>,
    limit: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RefreshHistoryResponse {
    trigger: &'static str,
    status: &'static str,
    summary: String,
    started_at: String,
    finished_at: Option<String>,
    import_count: u32,
    records_seen: String,
    records_rejected: String,
    failure: Option<HistoryFailureResponse>,
    imports: Vec<ImportHistoryResponse>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportHistoryResponse {
    source: String,
    projection: &'static str,
    scope: &'static str,
    status: &'static str,
    started_at: String,
    finished_at: Option<String>,
    records_seen: String,
    records_rejected: String,
    failure: Option<HistoryFailureResponse>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HistoryFailureResponse {
    category: &'static str,
    retryable: bool,
    summary: String,
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

#[tauri::command]
pub(super) fn diagnostics_get_history(
    request: HistoryCommandRequest,
    service: State<'_, HistoryService>,
) -> IpcResponse<HistoryResponse> {
    match service.history(HistoryRequest {
        cursor: request.cursor,
        limit: request.limit,
    }) {
        Ok(page) => match HistoryResponse::try_from(page) {
            Ok(response) => IpcResponse::success(response),
            Err(error) => IpcResponse::failure(history_error(error)),
        },
        Err(error) => IpcResponse::failure(history_error(error)),
    }
}

impl TryFrom<HistoryPage> for HistoryResponse {
    type Error = HistoryError;

    fn try_from(value: HistoryPage) -> Result<Self, Self::Error> {
        Ok(Self {
            items: value
                .items
                .into_iter()
                .map(RefreshHistoryResponse::try_from)
                .collect::<Result<_, _>>()?,
            next_cursor: value.next_cursor,
            limit: value.limit,
        })
    }
}

impl TryFrom<RefreshHistoryItem> for RefreshHistoryResponse {
    type Error = HistoryError;

    fn try_from(value: RefreshHistoryItem) -> Result<Self, Self::Error> {
        Ok(Self {
            trigger: history_trigger(value.trigger),
            status: history_status(value.status),
            summary: value.summary,
            started_at: timestamp(value.started_at_ms)?,
            finished_at: value.finished_at_ms.map(timestamp).transpose()?,
            import_count: value.import_count,
            records_seen: value.records_seen.to_string(),
            records_rejected: value.records_rejected.to_string(),
            failure: value.failure.map(Into::into),
            imports: value
                .imports
                .into_iter()
                .map(ImportHistoryResponse::try_from)
                .collect::<Result<_, _>>()?,
        })
    }
}

impl TryFrom<ImportHistoryItem> for ImportHistoryResponse {
    type Error = HistoryError;

    fn try_from(value: ImportHistoryItem) -> Result<Self, Self::Error> {
        Ok(Self {
            source: value.source,
            projection: history_projection(value.projection),
            scope: history_scope(value.scope),
            status: history_status(value.status),
            started_at: timestamp(value.started_at_ms)?,
            finished_at: value.finished_at_ms.map(timestamp).transpose()?,
            records_seen: value.records_seen.to_string(),
            records_rejected: value.records_rejected.to_string(),
            failure: value.failure.map(Into::into),
        })
    }
}

impl From<crate::application::history::HistoryFailure> for HistoryFailureResponse {
    fn from(value: crate::application::history::HistoryFailure) -> Self {
        Self {
            category: failure_category(value.category),
            retryable: value.retryable,
            summary: value.summary,
        }
    }
}

fn timestamp(value: i64) -> Result<String, HistoryError> {
    DateTime::<Utc>::from_timestamp_millis(value)
        .map(|timestamp| timestamp.to_rfc3339_opts(SecondsFormat::Millis, true))
        .ok_or(HistoryError::InvalidStoredValue)
}

const fn history_trigger(value: HistoryTrigger) -> &'static str {
    match value {
        HistoryTrigger::Launch => "launch",
        HistoryTrigger::Manual => "manual",
        HistoryTrigger::Scheduled => "scheduled",
        HistoryTrigger::FileChange => "file_change",
        HistoryTrigger::Resume => "resume",
        HistoryTrigger::Reconcile => "reconcile",
    }
}
const fn history_projection(value: HistoryProjection) -> &'static str {
    match value {
        HistoryProjection::Daily => "daily",
        HistoryProjection::Session => "session",
    }
}
const fn history_scope(value: HistoryScope) -> &'static str {
    match value {
        HistoryScope::Full => "full",
        HistoryScope::Incremental => "incremental",
    }
}
const fn history_status(value: HistoryStatus) -> &'static str {
    match value {
        HistoryStatus::Queued => "queued",
        HistoryStatus::Running => "running",
        HistoryStatus::Stale => "stale",
        HistoryStatus::Succeeded => "succeeded",
        HistoryStatus::Partial => "partial",
        HistoryStatus::Failed => "failed",
        HistoryStatus::Cancelled => "cancelled",
    }
}
const fn failure_category(value: FailureCategory) -> &'static str {
    match value {
        FailureCategory::Collector => "collector",
        FailureCategory::Reconciliation => "reconciliation",
        FailureCategory::Persistence => "persistence",
        FailureCategory::Cancelled => "cancelled",
        FailureCategory::Unknown => "unknown",
    }
}

fn history_error(error: HistoryError) -> IpcError {
    match error {
        HistoryError::InvalidLimit => IpcError::new(
            "diagnostics.history_invalid_limit",
            "History limit must be between 1 and 50.",
            ErrorCategory::Validation,
            false,
        ),
        HistoryError::InvalidCursor => IpcError::new(
            "diagnostics.history_invalid_cursor",
            "History cursor is invalid.",
            ErrorCategory::Validation,
            false,
        ),
        HistoryError::Unavailable => IpcError::new(
            "diagnostics.history_unavailable",
            "Burnly could not read local run history.",
            ErrorCategory::Persistence,
            true,
        ),
        HistoryError::InvalidStoredValue => IpcError::new(
            "diagnostics.history_invalid_data",
            "Local run history contains unsupported values.",
            ErrorCategory::Persistence,
            false,
        ),
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
