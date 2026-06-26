use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::application::ports::overview_store::OverviewStoreError;
use crate::application::usage::{
    OverviewDataStatus, TraySummaryModelRow, TraySummaryPeriodMetric, TraySummaryQuery,
    TraySummaryQueryError, TraySummaryReadModel, TraySummaryTrend, TraySummaryTrendDirection,
};

use super::response::{ErrorCategory, FieldError, IpcError, IpcResponse};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TraySummaryRequest {
    reporting_timezone: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TraySummaryResponse {
    today: TraySummaryPeriodMetricResponse,
    week: TraySummaryPeriodMetricResponse,
    month: TraySummaryPeriodMetricResponse,
    models: Vec<TraySummaryModelResponse>,
    as_of: String,
    last_successful_refresh_at: Option<String>,
    data_status: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TraySummaryPeriodMetricResponse {
    start_date: String,
    end_date: String,
    total_tokens: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TraySummaryModelResponse {
    model_name: String,
    agent_label: String,
    total_tokens: String,
    trend: Option<TraySummaryTrendResponse>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TraySummaryTrendResponse {
    direction: &'static str,
    basis_points: u32,
}

#[tauri::command]
pub(super) fn usage_get_tray_summary(
    request: TraySummaryRequest,
    query: State<'_, TraySummaryQuery>,
) -> IpcResponse<TraySummaryResponse> {
    match query.get(request.reporting_timezone) {
        Ok(summary) => match TraySummaryResponse::try_from(summary) {
            Ok(response) => IpcResponse::success(response),
            Err(error) => IpcResponse::failure(storage_error(error)),
        },
        Err(error) => IpcResponse::failure(tray_summary_query_error(error)),
    }
}

fn tray_summary_query_error(error: TraySummaryQueryError) -> IpcError {
    match error {
        TraySummaryQueryError::EmptyAggregationTimezone => IpcError::new(
            "validation.empty_reporting_timezone",
            "A reporting timezone is required.",
            ErrorCategory::Validation,
            false,
        )
        .with_field_errors(vec![FieldError::new(
            "request.reportingTimezone",
            "validation.required",
            "Reporting timezone is required.",
        )]),
        TraySummaryQueryError::InvalidAggregationTimezone => IpcError::new(
            "validation.invalid_reporting_timezone",
            "The reporting timezone is invalid.",
            ErrorCategory::Validation,
            false,
        )
        .with_field_errors(vec![FieldError::new(
            "request.reportingTimezone",
            "validation.timezone",
            "Reporting timezone must be a valid IANA timezone.",
        )]),
        TraySummaryQueryError::InvalidTimestamp => IpcError::new(
            "usage.tray_summary_invalid_time",
            "Burnly could not determine the current reporting day.",
            ErrorCategory::Internal,
            true,
        ),
        TraySummaryQueryError::Storage(storage) => match storage {
            crate::application::ports::tray_summary_store::TraySummaryStoreError::Backend => {
                IpcError::new(
                    "usage.tray_summary_unavailable",
                    "Burnly could not read local tray summary data.",
                    ErrorCategory::Persistence,
                    true,
                )
            }
            crate::application::ports::tray_summary_store::TraySummaryStoreError::ValueOutOfRange => {
                IpcError::new(
                    "usage.tray_summary_inconsistent",
                    "Burnly found inconsistent local tray summary data.",
                    ErrorCategory::Persistence,
                    false,
                )
            }
        },
    }
}

fn storage_error(error: OverviewStoreError) -> IpcError {
    match error {
        OverviewStoreError::Backend => IpcError::new(
            "usage.overview_unavailable",
            "Burnly could not read local usage data.",
            ErrorCategory::Persistence,
            true,
        ),
        OverviewStoreError::ValueOutOfRange | OverviewStoreError::MixedCurrencies => IpcError::new(
            "usage.overview_inconsistent",
            "Burnly found inconsistent local usage data.",
            ErrorCategory::Persistence,
            false,
        ),
    }
}

impl TryFrom<TraySummaryReadModel> for TraySummaryResponse {
    type Error = OverviewStoreError;

    fn try_from(value: TraySummaryReadModel) -> Result<Self, Self::Error> {
        Ok(Self {
            today: value.today.into(),
            week: value.week.into(),
            month: value.month.into(),
            models: value.models.into_iter().map(Into::into).collect(),
            as_of: to_rfc3339(value.as_of_ms)?,
            last_successful_refresh_at: value
                .last_successful_refresh_at_ms
                .map(to_rfc3339)
                .transpose()?,
            data_status: data_status(value.data_status),
        })
    }
}

impl From<TraySummaryPeriodMetric> for TraySummaryPeriodMetricResponse {
    fn from(value: TraySummaryPeriodMetric) -> Self {
        Self {
            start_date: value.start_date.to_string(),
            end_date: value.end_date.to_string(),
            total_tokens: value.total_tokens.to_string(),
        }
    }
}

impl From<TraySummaryModelRow> for TraySummaryModelResponse {
    fn from(value: TraySummaryModelRow) -> Self {
        Self {
            model_name: value.model_name,
            agent_label: value.agent_label,
            total_tokens: value.total_tokens.to_string(),
            trend: value.trend.map(Into::into),
        }
    }
}

impl From<TraySummaryTrend> for TraySummaryTrendResponse {
    fn from(value: TraySummaryTrend) -> Self {
        Self {
            direction: tray_summary_trend_direction(value.direction),
            basis_points: value.basis_points,
        }
    }
}

const fn tray_summary_trend_direction(value: TraySummaryTrendDirection) -> &'static str {
    match value {
        TraySummaryTrendDirection::Increased => "increased",
        TraySummaryTrendDirection::Decreased => "decreased",
        TraySummaryTrendDirection::Flat => "flat",
    }
}

const fn data_status(value: OverviewDataStatus) -> &'static str {
    match value {
        OverviewDataStatus::Current => "current",
        OverviewDataStatus::Stale => "stale",
        OverviewDataStatus::Partial => "partial",
        OverviewDataStatus::Empty => "empty",
    }
}

fn to_rfc3339(epoch_ms: i64) -> Result<String, OverviewStoreError> {
    DateTime::<Utc>::from_timestamp_millis(epoch_ms)
        .map(|timestamp| timestamp.to_rfc3339_opts(SecondsFormat::Secs, true))
        .ok_or(OverviewStoreError::ValueOutOfRange)
}
