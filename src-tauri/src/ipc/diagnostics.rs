use std::fs;

use chrono::{SecondsFormat, Utc};
use serde::Serialize;
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_dialog::DialogExt;

use crate::application::diagnostics::{DiagnosticsHealth, DiagnosticsService};

use super::response::{ErrorCategory, IpcError, IpcResponse};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DiagnosticsExportResponse {
    status: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DiagnosticsCopyResponse {
    status: &'static str,
}

#[tauri::command]
pub(super) async fn diagnostics_get_health<R: Runtime>(
    app: AppHandle<R>,
) -> IpcResponse<DiagnosticsHealth> {
    let Some(service) = app.try_state::<DiagnosticsService>() else {
        return IpcResponse::failure(report_error());
    };
    match service.health() {
        Ok(health) => IpcResponse::success(health),
        Err(_) => IpcResponse::failure(report_error()),
    }
}

#[tauri::command]
pub(super) async fn diagnostics_export_report<R: Runtime>(
    app: AppHandle<R>,
) -> IpcResponse<DiagnosticsExportResponse> {
    let Some(service) = app.try_state::<DiagnosticsService>() else {
        return IpcResponse::failure(report_error());
    };
    let report = match report_json(&service) {
        Ok(report) => report,
        Err(_) => return IpcResponse::failure(report_error()),
    };
    let file_name = diagnostics_file_name();
    let Some(file_path) = app
        .dialog()
        .file()
        .set_title("Export Burnly diagnostics")
        .set_file_name(file_name)
        .add_filter("JSON", &["json"])
        .blocking_save_file()
    else {
        return IpcResponse::success(DiagnosticsExportResponse {
            status: "cancelled",
        });
    };
    let path = match file_path.into_path() {
        Ok(path) => path,
        Err(_) => {
            return IpcResponse::failure(IpcError::new(
                "diagnostics.export_path_unavailable",
                "Burnly could not use the selected diagnostics file path.",
                ErrorCategory::Platform,
                true,
            ));
        }
    };

    match fs::write(path, report) {
        Ok(()) => IpcResponse::success(DiagnosticsExportResponse { status: "exported" }),
        Err(_) => IpcResponse::failure(IpcError::new(
            "diagnostics.export_failed",
            "Burnly could not export the diagnostics report.",
            ErrorCategory::Persistence,
            true,
        )),
    }
}

#[tauri::command]
pub(super) async fn diagnostics_copy_report<R: Runtime>(
    app: AppHandle<R>,
) -> IpcResponse<DiagnosticsCopyResponse> {
    let Some(service) = app.try_state::<DiagnosticsService>() else {
        return IpcResponse::failure(report_error());
    };
    let report = match report_json(&service) {
        Ok(report) => report,
        Err(_) => return IpcResponse::failure(report_error()),
    };

    match app.clipboard().write_text(report) {
        Ok(()) => IpcResponse::success(DiagnosticsCopyResponse { status: "copied" }),
        Err(_) => IpcResponse::failure(IpcError::new(
            "diagnostics.copy_failed",
            "Burnly could not copy the diagnostics report.",
            ErrorCategory::Platform,
            true,
        )),
    }
}

fn diagnostics_file_name() -> String {
    let timestamp = Utc::now()
        .to_rfc3339_opts(SecondsFormat::Secs, true)
        .replace([':', '.'], "-");
    format!("burnly-diagnostics-{timestamp}.json")
}

fn report_json(service: &DiagnosticsService) -> Result<String, ()> {
    service
        .report()
        .and_then(|report| {
            serde_json::to_string_pretty(&report)
                .map_err(|_| crate::application::diagnostics::DiagnosticsReportError::Store)
        })
        .map_err(|_| ())
}

fn report_error() -> IpcError {
    IpcError::new(
        "diagnostics.report_failed",
        "Burnly could not create the diagnostics report.",
        ErrorCategory::Persistence,
        true,
    )
}
