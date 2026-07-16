use serde::{Deserialize, Serialize};
use tauri::{Emitter, State};

use crate::application::settings::{RuntimeSettingError, SettingsError, SettingsService};
use crate::domain::settings::{Settings, SettingsDocument, SettingsValidationError};

use super::events::{names as event_names, DataInvalidatedEvent, SettingsChangedEvent};
use super::response::{ErrorCategory, FieldError, IpcError, IpcResponse};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SettingsResponse {
    launch_at_login: bool,
    close_behavior: &'static str,
    revision: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct UpdateSettingsRequest {
    expected_revision: i64,
    launch_at_login: bool,
    close_behavior: String,
}

#[tauri::command]
pub(super) fn settings_get(service: State<'_, SettingsService>) -> IpcResponse<SettingsResponse> {
    match service.get() {
        Ok(settings) => IpcResponse::success(settings.into()),
        Err(error) => IpcResponse::failure(settings_error(error)),
    }
}

#[tauri::command]
pub(super) fn settings_update<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    service: State<'_, SettingsService>,
    request: UpdateSettingsRequest,
) -> IpcResponse<SettingsResponse> {
    let settings = match Settings::new(request.launch_at_login, &request.close_behavior) {
        Ok(settings) => settings,
        Err(error) => {
            return IpcResponse::failure(validation_error(error));
        }
    };

    match service.update(request.expected_revision, settings) {
        Ok(updated) => {
            let response = SettingsResponse::from(updated);
            let _ = app.emit(
                event_names::SETTINGS_CHANGED,
                SettingsChangedEvent {
                    revision: response.revision,
                },
            );
            let _ = app.emit(
                event_names::DATA_INVALIDATED,
                DataInvalidatedEvent { scope: "budgets" },
            );
            IpcResponse::success(response)
        }
        Err(error) => IpcResponse::failure(settings_error(error)),
    }
}

impl From<SettingsDocument> for SettingsResponse {
    fn from(value: SettingsDocument) -> Self {
        let revision = value.revision();
        let settings = value.settings();
        Self {
            launch_at_login: settings.launch_at_login(),
            close_behavior: settings.close_behavior().as_str(),
            revision,
        }
    }
}

fn settings_error(error: SettingsError) -> IpcError {
    match error {
        SettingsError::Validation(error) => validation_error(error),
        SettingsError::Conflict => IpcError::new(
            "settings.revision_conflict",
            "Settings changed since this screen was loaded.",
            ErrorCategory::Conflict,
            true,
        ),
        SettingsError::StorageUnavailable | SettingsError::InvalidStoredValue => IpcError::new(
            "settings.storage_unavailable",
            "Burnly could not access local settings.",
            ErrorCategory::Persistence,
            true,
        ),
        SettingsError::Runtime(error) => runtime_error(error),
    }
}

fn validation_error(error: SettingsValidationError) -> IpcError {
    let field = match error {
        SettingsValidationError::CloseBehavior => FieldError::new(
            "closeBehavior",
            "settings.invalid_close_behavior",
            "Close behavior must be hide or quit.",
        ),
        SettingsValidationError::Revision => FieldError::new(
            "expectedRevision",
            "settings.invalid_revision",
            "Settings revision must be positive.",
        ),
    };
    IpcError::new(
        "settings.validation_failed",
        "Some settings are invalid.",
        ErrorCategory::Validation,
        false,
    )
    .with_field_errors(vec![field])
}

fn runtime_error(error: RuntimeSettingError) -> IpcError {
    match error {
        RuntimeSettingError::LaunchAtLoginUnavailable => IpcError::new(
            "settings.launch_at_login_unavailable",
            "Launch at login is not available in this build.",
            ErrorCategory::Unavailable,
            false,
        ),
        RuntimeSettingError::LaunchAtLoginApplyFailed => IpcError::new(
            "settings.launch_at_login_apply_failed",
            "Burnly could not update the system launch-at-login setting.",
            ErrorCategory::Platform,
            true,
        ),
    }
}
