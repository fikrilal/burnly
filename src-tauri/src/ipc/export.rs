use serde::{Deserialize, Serialize};
use tauri::{Manager, State};

use crate::application::export::{
    ConfirmedExportRequest, ExportError, ExportOutcome, ExportPreview, ExportRequest, ExportService,
};
use crate::application::ports::export_store::ExportDataset;

use super::response::{ErrorCategory, IpcError, IpcResponse};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ExportPreviewRequest {
    start_date: String,
    end_date: String,
    datasets: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ConfirmedExportCommandRequest {
    request: ExportPreviewRequest,
    preview_token: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ExportPreviewResponse {
    start_date: String,
    end_date: String,
    format: &'static str,
    datasets: Vec<ExportDatasetPreviewResponse>,
    total_rows: String,
    estimated_bytes: String,
    privacy_notes: Vec<String>,
    preview_token: String,
    can_export: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportDatasetPreviewResponse {
    dataset: &'static str,
    rows: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ExportResponse {
    status: &'static str,
    rows: String,
    message: &'static str,
}

#[tauri::command]
pub(super) fn history_get_export_preview(
    request: ExportPreviewRequest,
    service: State<'_, ExportService>,
) -> IpcResponse<ExportPreviewResponse> {
    match export_request(request).and_then(|request| service.preview(request)) {
        Ok(preview) => IpcResponse::success(preview.into()),
        Err(error) => IpcResponse::failure(export_error(error)),
    }
}

#[tauri::command]
pub(super) async fn history_export<R: tauri::Runtime>(
    request: ConfirmedExportCommandRequest,
    app: tauri::AppHandle<R>,
) -> IpcResponse<ExportResponse> {
    let confirmed = match export_request(request.request) {
        Ok(export_request) => ConfirmedExportRequest {
            request: export_request,
            preview_token: request.preview_token,
        },
        Err(error) => return IpcResponse::failure(export_error(error)),
    };
    let service = app.state::<ExportService>().inner().clone();
    match tauri::async_runtime::spawn_blocking(move || service.export(confirmed)).await {
        Ok(Ok(outcome)) => IpcResponse::success(outcome.into()),
        Ok(Err(error)) => IpcResponse::failure(export_error(error)),
        Err(_) => IpcResponse::failure(export_error(ExportError::WriteFailed)),
    }
}

fn export_request(value: ExportPreviewRequest) -> Result<ExportRequest, ExportError> {
    let datasets = value
        .datasets
        .into_iter()
        .map(|dataset| match dataset.as_str() {
            "daily_usage" => Ok(ExportDataset::DailyUsage),
            "sessions" => Ok(ExportDataset::Sessions),
            _ => Err(ExportError::NoDatasets),
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ExportRequest {
        start_date: value.start_date,
        end_date: value.end_date,
        datasets,
    })
}

impl From<ExportPreview> for ExportPreviewResponse {
    fn from(value: ExportPreview) -> Self {
        Self {
            start_date: value.start_date,
            end_date: value.end_date,
            format: "csv",
            datasets: value
                .datasets
                .into_iter()
                .map(|dataset| ExportDatasetPreviewResponse {
                    dataset: dataset_name(dataset.dataset),
                    rows: dataset.rows.to_string(),
                })
                .collect(),
            total_rows: value.total_rows.to_string(),
            estimated_bytes: value.estimated_bytes.to_string(),
            privacy_notes: value.privacy_notes,
            preview_token: value.preview_token,
            can_export: value.can_export,
        }
    }
}

impl From<ExportOutcome> for ExportResponse {
    fn from(value: ExportOutcome) -> Self {
        match value {
            ExportOutcome::Exported { rows } => Self {
                status: "exported",
                rows: rows.to_string(),
                message: "CSV export saved.",
            },
            ExportOutcome::Cancelled => Self {
                status: "cancelled",
                rows: "0".to_owned(),
                message: "Export cancelled. No file was written.",
            },
        }
    }
}

const fn dataset_name(value: ExportDataset) -> &'static str {
    match value {
        ExportDataset::DailyUsage => "daily_usage",
        ExportDataset::Sessions => "sessions",
    }
}

fn export_error(error: ExportError) -> IpcError {
    match error {
        ExportError::InvalidDateRange => IpcError::new(
            "export.invalid_date_range",
            "Choose a valid export date range.",
            ErrorCategory::Validation,
            false,
        ),
        ExportError::NoDatasets | ExportError::DuplicateDataset => IpcError::new(
            "export.invalid_datasets",
            "Choose at least one unique export dataset.",
            ErrorCategory::Validation,
            false,
        ),
        ExportError::StalePreview => IpcError::new(
            "export.preview_stale",
            "Local data changed. Preview the export again.",
            ErrorCategory::Conflict,
            true,
        ),
        ExportError::TooLarge => IpcError::new(
            "export.too_large",
            "Narrow the date range before exporting.",
            ErrorCategory::Validation,
            false,
        ),
        ExportError::Unavailable => IpcError::new(
            "export.data_unavailable",
            "Burnly could not read export data.",
            ErrorCategory::Persistence,
            true,
        ),
        ExportError::InvalidStoredValue => IpcError::new(
            "export.invalid_data",
            "Local usage contains unsupported export values.",
            ErrorCategory::Persistence,
            false,
        ),
        ExportError::DestinationUnavailable => IpcError::new(
            "export.destination_unavailable",
            "The selected export destination is unavailable.",
            ErrorCategory::Platform,
            true,
        ),
        ExportError::WriteFailed => IpcError::new(
            "export.write_failed",
            "Burnly could not write the export file.",
            ErrorCategory::Platform,
            true,
        ),
    }
}
