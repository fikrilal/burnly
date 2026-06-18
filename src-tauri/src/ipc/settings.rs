use serde::{Deserialize, Serialize};
use tauri::{Emitter, State};

use crate::application::settings::{RuntimeSettingError, SettingsError, SettingsService};
use crate::domain::settings::{Settings, SettingsDocument, SettingsValidationError};

use super::response::{ErrorCategory, FieldError, IpcError, IpcResponse};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SettingsResponse {
    reporting_timezone: String,
    background_refresh_enabled: bool,
    refresh_interval_minutes: i64,
    launch_at_login: bool,
    close_behavior: &'static str,
    notifications_enabled: bool,
    store_project_paths: bool,
    revision: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct UpdateSettingsRequest {
    expected_revision: i64,
    reporting_timezone: String,
    background_refresh_enabled: bool,
    refresh_interval_minutes: i64,
    launch_at_login: bool,
    close_behavior: String,
    notifications_enabled: bool,
    store_project_paths: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct UpdateProjectPathRetentionRequest {
    expected_revision: i64,
    retain_paths: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ProjectPathRetentionResponse {
    settings: SettingsResponse,
    cleared_paths: u32,
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
    let settings = match Settings::new(
        request.reporting_timezone,
        request.background_refresh_enabled,
        request.refresh_interval_minutes,
        request.launch_at_login,
        &request.close_behavior,
        request.notifications_enabled,
        request.store_project_paths,
    ) {
        Ok(settings) => settings,
        Err(error) => {
            return IpcResponse::failure(validation_error(error));
        }
    };

    match service.update(request.expected_revision, settings) {
        Ok(updated) => {
            let _ = app.emit("burnly://v1/settings-changed", ());
            IpcResponse::success(updated.into())
        }
        Err(error) => IpcResponse::failure(settings_error(error)),
    }
}

#[tauri::command]
pub(super) fn settings_update_project_path_retention<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    service: State<'_, SettingsService>,
    request: UpdateProjectPathRetentionRequest,
) -> IpcResponse<ProjectPathRetentionResponse> {
    match service.update_project_path_retention(request.expected_revision, request.retain_paths) {
        Ok(result) => {
            let response = ProjectPathRetentionResponse {
                settings: result.settings.into(),
                cleared_paths: result.cleared_paths,
            };
            let _ = app.emit("burnly://v1/settings-changed", ());
            let _ = app.emit(
                "burnly://v1/data-invalidated",
                serde_json::json!({ "scope": "sessions" }),
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
            reporting_timezone: settings.reporting_timezone().to_owned(),
            background_refresh_enabled: settings.background_refresh_enabled(),
            refresh_interval_minutes: settings.refresh_interval_minutes(),
            launch_at_login: settings.launch_at_login(),
            close_behavior: settings.close_behavior().as_str(),
            notifications_enabled: settings.notifications_enabled(),
            store_project_paths: settings.store_project_paths(),
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
        SettingsValidationError::ReportingTimezone => FieldError::new(
            "reportingTimezone",
            "settings.invalid_timezone",
            "Enter a valid IANA timezone.",
        ),
        SettingsValidationError::RefreshInterval => FieldError::new(
            "refreshIntervalMinutes",
            "settings.invalid_refresh_interval",
            "Refresh interval must be between 5 and 1440 minutes.",
        ),
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
        RuntimeSettingError::NotificationsUnavailable => IpcError::new(
            "settings.notifications_unavailable",
            "Native notifications are not available in this build.",
            ErrorCategory::Unavailable,
            false,
        ),
        RuntimeSettingError::ProjectPathRetentionRequiresPrivacyFlow => IpcError::new(
            "settings.project_path_privacy_flow_required",
            "Project-path retention must be changed through the privacy confirmation flow.",
            ErrorCategory::Conflict,
            false,
        ),
    }
}
