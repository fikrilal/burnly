use serde::{Deserialize, Serialize};
use tauri::{Emitter, State};

use crate::application::history_deletion::{
    ConfirmedHistoryDeletion, HistoryDeletionError, HistoryDeletionPreview, HistoryDeletionResult,
    HistoryDeletionService,
};
use crate::application::ports::history_deletion_store::HistoryDeletionSnapshot;

use super::response::{ErrorCategory, IpcError, IpcResponse};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DeleteHistoryRequest {
    preview_token: String,
    confirmation: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DeleteHistoryPreviewResponse {
    scope: String,
    earliest_date: Option<String>,
    latest_date: Option<String>,
    source_count: String,
    counts: DeleteHistoryCountsResponse,
    total_records: String,
    preserved: Vec<String>,
    preview_token: String,
    can_delete: bool,
    active_refresh: bool,
    confirmation_text: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DeleteHistoryCountsResponse {
    daily_usage: String,
    daily_model_usage: String,
    sessions: String,
    session_model_usage: String,
    refresh_runs: String,
    import_runs: String,
    projects: String,
    source_models: String,
    notification_records: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DeleteHistoryResponse {
    deleted_records: String,
    message: &'static str,
}

#[tauri::command]
pub(super) fn history_get_delete_preview(
    service: State<'_, HistoryDeletionService>,
) -> IpcResponse<DeleteHistoryPreviewResponse> {
    match service.preview() {
        Ok(preview) => IpcResponse::success(preview.into()),
        Err(error) => IpcResponse::failure(deletion_error(error)),
    }
}

#[tauri::command]
pub(super) fn history_delete<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    service: State<'_, HistoryDeletionService>,
    request: DeleteHistoryRequest,
) -> IpcResponse<DeleteHistoryResponse> {
    match service.delete(ConfirmedHistoryDeletion {
        preview_token: request.preview_token,
        confirmation: request.confirmation,
    }) {
        Ok(result) => {
            let _ = app.emit(
                "burnly://v1/data-invalidated",
                serde_json::json!({ "scope": "history_deleted" }),
            );
            IpcResponse::success(result.into())
        }
        Err(error) => IpcResponse::failure(deletion_error(error)),
    }
}

impl From<HistoryDeletionPreview> for DeleteHistoryPreviewResponse {
    fn from(value: HistoryDeletionPreview) -> Self {
        let active_refresh = value.snapshot.active_refresh;
        let earliest_date = value.snapshot.earliest_date.clone();
        let latest_date = value.snapshot.latest_date.clone();
        let source_count = value.snapshot.source_count.to_string();
        Self {
            scope: value.scope,
            earliest_date,
            latest_date,
            source_count,
            counts: value.snapshot.into(),
            total_records: value.total_records.to_string(),
            preserved: value.preserved,
            preview_token: value.preview_token,
            can_delete: value.can_delete,
            active_refresh,
            confirmation_text: value.confirmation_text,
        }
    }
}

impl From<HistoryDeletionSnapshot> for DeleteHistoryCountsResponse {
    fn from(value: HistoryDeletionSnapshot) -> Self {
        Self {
            daily_usage: value.daily_usage.to_string(),
            daily_model_usage: value.daily_model_usage.to_string(),
            sessions: value.sessions.to_string(),
            session_model_usage: value.session_model_usage.to_string(),
            refresh_runs: value.refresh_runs.to_string(),
            import_runs: value.import_runs.to_string(),
            projects: value.projects.to_string(),
            source_models: value.source_models.to_string(),
            notification_records: value.notification_records.to_string(),
        }
    }
}

impl From<HistoryDeletionResult> for DeleteHistoryResponse {
    fn from(value: HistoryDeletionResult) -> Self {
        Self {
            deleted_records: value.deleted_records.to_string(),
            message: "Local imported history deleted.",
        }
    }
}

fn deletion_error(error: HistoryDeletionError) -> IpcError {
    match error {
        HistoryDeletionError::ConfirmationRequired => IpcError::new(
            "history_delete.confirmation_required",
            "Type the required confirmation text before deleting history.",
            ErrorCategory::Validation,
            false,
        ),
        HistoryDeletionError::StalePreview => IpcError::new(
            "history_delete.preview_stale",
            "Local history changed. Preview deletion again.",
            ErrorCategory::Conflict,
            true,
        ),
        HistoryDeletionError::ActiveRefresh => IpcError::new(
            "history_delete.refresh_active",
            "Wait for the active refresh to finish before deleting history.",
            ErrorCategory::Conflict,
            true,
        ),
        HistoryDeletionError::Unavailable => IpcError::new(
            "history_delete.unavailable",
            "Burnly could not delete local history.",
            ErrorCategory::Persistence,
            true,
        ),
        HistoryDeletionError::InvalidStoredValue => IpcError::new(
            "history_delete.invalid_data",
            "Local history contains unsupported values.",
            ErrorCategory::Persistence,
            false,
        ),
    }
}
