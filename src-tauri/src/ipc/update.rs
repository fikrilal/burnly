use chrono::{DateTime, SecondsFormat, Utc};
use serde::Serialize;
use tauri::State;

use crate::application::update::{
    update_status_label, UpdateRuntimeError, UpdateService, UpdateSnapshot,
};

use super::response::{ErrorCategory, IpcError, IpcResponse};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct UpdateStatusResponse {
    status: &'static str,
    available_version: Option<String>,
    downloaded_version: Option<String>,
    last_checked_at: Option<String>,
    error: Option<UpdateErrorResponse>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateErrorResponse {
    code: &'static str,
    retryable: bool,
}

#[tauri::command]
pub(super) fn update_get_state(
    service: State<'_, UpdateService>,
) -> IpcResponse<UpdateStatusResponse> {
    IpcResponse::success(service.status().into())
}

#[tauri::command]
pub(super) fn update_check(service: State<'_, UpdateService>) -> IpcResponse<UpdateStatusResponse> {
    update_command_result(service.check())
}

#[tauri::command]
pub(super) fn update_download(
    service: State<'_, UpdateService>,
) -> IpcResponse<UpdateStatusResponse> {
    update_command_result(service.download())
}

#[tauri::command]
pub(super) fn update_restart(
    service: State<'_, UpdateService>,
) -> IpcResponse<UpdateStatusResponse> {
    update_command_result(service.restart())
}

fn update_command_result(
    result: Result<UpdateSnapshot, UpdateRuntimeError>,
) -> IpcResponse<UpdateStatusResponse> {
    match result {
        Ok(snapshot) => IpcResponse::success(snapshot.into()),
        Err(error) => IpcResponse::failure(update_error(error)),
    }
}

impl From<UpdateSnapshot> for UpdateStatusResponse {
    fn from(value: UpdateSnapshot) -> Self {
        Self {
            status: update_status_label(value.status),
            available_version: value.available_version,
            downloaded_version: value.downloaded_version,
            last_checked_at: value.last_checked_at_ms.map(to_rfc3339),
            error: value.error.map(|error| UpdateErrorResponse {
                code: error.code,
                retryable: error.retryable,
            }),
        }
    }
}

fn to_rfc3339(epoch_ms: i64) -> String {
    DateTime::<Utc>::from_timestamp_millis(epoch_ms)
        .expect("stored update timestamp must be representable")
        .to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn update_error(error: UpdateRuntimeError) -> IpcError {
    let category = match error {
        UpdateRuntimeError::Unavailable => ErrorCategory::Unavailable,
        UpdateRuntimeError::InvalidState => ErrorCategory::Conflict,
        UpdateRuntimeError::Network
        | UpdateRuntimeError::Signature
        | UpdateRuntimeError::Install
        | UpdateRuntimeError::Internal => ErrorCategory::Update,
    };
    IpcError::new(
        error.code(),
        update_error_message(&error),
        category,
        error.retryable(),
    )
}

fn update_error_message(error: &UpdateRuntimeError) -> &'static str {
    match error {
        UpdateRuntimeError::Unavailable => "Burnly updates are not available in this build.",
        UpdateRuntimeError::InvalidState => "Burnly cannot run that update operation right now.",
        UpdateRuntimeError::Network => "Burnly could not reach the update feed.",
        UpdateRuntimeError::Signature => "Burnly could not verify the update signature.",
        UpdateRuntimeError::Install => "Burnly could not install the downloaded update.",
        UpdateRuntimeError::Internal => "Burnly could not complete the update operation.",
    }
}
