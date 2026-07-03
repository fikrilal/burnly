//! Application-owned diagnostic event model.
//!
//! Diagnostic events are local, redacted breadcrumbs. They intentionally carry
//! stable codes and bounded context instead of raw external payloads.

#![allow(
    dead_code,
    reason = "The diagnostics event vocabulary is introduced before every area is wired"
)]

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::Serialize;
use thiserror::Error;

use crate::application::ports::diagnostics_report_store::DiagnosticsReportStore;

const MAX_CODE_LEN: usize = 128;
const MAX_SUMMARY_LEN: usize = 240;
const MAX_CONTEXT_JSON_LEN: usize = 4_096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DiagnosticArea {
    Refresh,
    Collector,
    TraySummary,
    Settings,
    Update,
    LaunchAtLogin,
    Database,
    Runtime,
}

impl DiagnosticArea {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Refresh => "refresh",
            Self::Collector => "collector",
            Self::TraySummary => "tray_summary",
            Self::Settings => "settings",
            Self::Update => "update",
            Self::LaunchAtLogin => "launch_at_login",
            Self::Database => "database",
            Self::Runtime => "runtime",
        }
    }

    pub(crate) fn from_storage(value: &str) -> Option<Self> {
        match value {
            "refresh" => Some(Self::Refresh),
            "collector" => Some(Self::Collector),
            "tray_summary" => Some(Self::TraySummary),
            "settings" => Some(Self::Settings),
            "update" => Some(Self::Update),
            "launch_at_login" => Some(Self::LaunchAtLogin),
            "database" => Some(Self::Database),
            "runtime" => Some(Self::Runtime),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DiagnosticsHealthStatus {
    Ok,
    Warning,
    Error,
}

impl DiagnosticsHealthStatus {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiagnosticsHealthReason {
    pub code: String,
    pub message: String,
}

impl DiagnosticsHealthReason {
    pub(crate) fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiagnosticsHealth {
    pub status: DiagnosticsHealthStatus,
    pub reasons: Vec<DiagnosticsHealthReason>,
    pub generated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DiagnosticsReportRequest {
    pub generated_at_ms: i64,
    pub app_version: String,
    pub platform: String,
    pub arch: String,
    pub debug: bool,
    pub timezone: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiagnosticsReport {
    pub schema_version: u16,
    pub generated_at: String,
    pub app: DiagnosticsAppReport,
    pub environment: DiagnosticsEnvironmentReport,
    pub health: DiagnosticsHealth,
    pub database: DiagnosticsDatabaseReport,
    pub refresh: DiagnosticsRefreshReport,
    pub imports: DiagnosticsImportsReport,
    pub sources: DiagnosticsSourcesReport,
    pub usage_integrity: DiagnosticsUsageIntegrityReport,
    pub diagnostic_events: Vec<DiagnosticsEventReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiagnosticsAppReport {
    pub version: String,
    pub platform: String,
    pub arch: String,
    pub debug: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiagnosticsEnvironmentReport {
    pub timezone: String,
    pub locale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiagnosticsDatabaseReport {
    pub schema_version: i64,
    pub tables_present: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiagnosticsRefreshReport {
    pub latest_runs: Vec<DiagnosticsRefreshRunReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiagnosticsRefreshRunReport {
    pub id: String,
    pub trigger: String,
    pub status: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub requested_by_app_version: String,
    pub error: Option<DiagnosticsRunErrorReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiagnosticsImportsReport {
    pub latest_runs: Vec<DiagnosticsImportRunReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiagnosticsImportRunReport {
    pub id: String,
    pub refresh_run_id: String,
    pub source_id: String,
    pub collector_key: String,
    pub collector_version: String,
    pub profile_version: i64,
    pub projection: String,
    pub scope_kind: String,
    pub scope_start_date: Option<String>,
    pub scope_end_date: Option<String>,
    pub status: String,
    pub records_seen: String,
    pub records_rejected: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub error: Option<DiagnosticsRunErrorReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiagnosticsRunErrorReport {
    pub code: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiagnosticsSourcesReport {
    pub recent: Vec<DiagnosticsSourceReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiagnosticsSourceReport {
    pub source_id: String,
    pub status: String,
    pub latest_import_status: Option<String>,
    pub latest_projection: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiagnosticsUsageIntegrityReport {
    pub today_daily_usage_rows: i64,
    pub today_daily_model_usage_rows: i64,
    pub today_daily_usage_token_sum: String,
    pub today_daily_model_usage_token_sum: String,
    pub orphan_daily_model_rows: i64,
    pub model_rows_without_total_tokens: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiagnosticsEventReport {
    pub id: String,
    pub area: String,
    pub severity: String,
    pub code: String,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<BTreeMap<String, String>>,
    pub created_at: String,
}

#[derive(Debug, Error)]
pub(crate) enum DiagnosticsReportError {
    #[error("failed to read diagnostics report data")]
    Store,
}

pub(crate) struct DiagnosticsService {
    store: Arc<dyn DiagnosticsReportStore>,
    app_version: String,
    timezone: String,
}

impl DiagnosticsService {
    pub(crate) fn new(
        store: Arc<dyn DiagnosticsReportStore>,
        app_version: String,
        timezone: String,
    ) -> Self {
        Self {
            store,
            app_version,
            timezone,
        }
    }

    pub(crate) fn health(&self) -> Result<DiagnosticsHealth, DiagnosticsReportError> {
        Ok(self.report()?.health)
    }

    pub(crate) fn report(&self) -> Result<DiagnosticsReport, DiagnosticsReportError> {
        self.store.report(DiagnosticsReportRequest {
            generated_at_ms: chrono::Utc::now().timestamp_millis(),
            app_version: self.app_version.clone(),
            platform: std::env::consts::OS.to_owned(),
            arch: std::env::consts::ARCH.to_owned(),
            debug: cfg!(debug_assertions),
            timezone: self.timezone.clone(),
        })
    }
}

impl DiagnosticSeverity {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }

    pub(crate) fn from_storage(value: &str) -> Option<Self> {
        match value {
            "info" => Some(Self::Info),
            "warning" => Some(Self::Warning),
            "error" => Some(Self::Error),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DiagnosticCode(String);

impl DiagnosticCode {
    pub(crate) fn new(value: impl Into<String>) -> Result<Self, DiagnosticValidationError> {
        let value = value.into();
        validate_bounded_text(&value, MAX_CODE_LEN, DiagnosticValidationError::Code)?;
        Ok(Self(value))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DiagnosticSummary(String);

impl DiagnosticSummary {
    pub(crate) fn new(value: impl Into<String>) -> Result<Self, DiagnosticValidationError> {
        let value = value.into();
        validate_bounded_text(&value, MAX_SUMMARY_LEN, DiagnosticValidationError::Summary)?;
        Ok(Self(value))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DiagnosticContext(String);

impl DiagnosticContext {
    pub(crate) fn new(value: impl Into<String>) -> Result<Self, DiagnosticValidationError> {
        let value = value.into();
        if value.trim().is_empty() || value.len() > MAX_CONTEXT_JSON_LEN {
            return Err(DiagnosticValidationError::Context);
        }
        Ok(Self(value))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DiagnosticEvent {
    pub area: DiagnosticArea,
    pub severity: DiagnosticSeverity,
    pub code: DiagnosticCode,
    pub summary: DiagnosticSummary,
    pub context: Option<DiagnosticContext>,
    pub created_at_ms: i64,
}

impl DiagnosticEvent {
    pub(crate) fn new(
        area: DiagnosticArea,
        severity: DiagnosticSeverity,
        code: DiagnosticCode,
        summary: DiagnosticSummary,
        context: Option<DiagnosticContext>,
        created_at_ms: i64,
    ) -> Result<Self, DiagnosticValidationError> {
        if created_at_ms < 0 {
            return Err(DiagnosticValidationError::CreatedAt);
        }

        Ok(Self {
            area,
            severity,
            code,
            summary,
            context,
            created_at_ms,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StoredDiagnosticEvent {
    pub id: i64,
    pub event: DiagnosticEvent,
}

impl StoredDiagnosticEvent {
    pub(crate) fn new(id: i64, event: DiagnosticEvent) -> Result<Self, DiagnosticValidationError> {
        if id <= 0 {
            return Err(DiagnosticValidationError::Id);
        }
        Ok(Self { id, event })
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DiagnosticValidationError {
    #[error("diagnostic event id must be positive")]
    Id,
    #[error("diagnostic event code must be non-empty and bounded")]
    Code,
    #[error("diagnostic event summary must be non-empty and bounded")]
    Summary,
    #[error("diagnostic event context must be a bounded JSON object")]
    Context,
    #[error("diagnostic event created_at_ms must be non-negative")]
    CreatedAt,
}

fn validate_bounded_text(
    value: &str,
    max_len: usize,
    error: DiagnosticValidationError,
) -> Result<(), DiagnosticValidationError> {
    if value.trim().is_empty() || value.len() > max_len {
        return Err(error);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_area_and_severity_have_stable_storage_values() {
        assert_eq!(DiagnosticArea::Refresh.as_str(), "refresh");
        assert_eq!(
            DiagnosticArea::from_storage("refresh"),
            Some(DiagnosticArea::Refresh)
        );
        assert_eq!(DiagnosticSeverity::Warning.as_str(), "warning");
        assert_eq!(
            DiagnosticSeverity::from_storage("warning"),
            Some(DiagnosticSeverity::Warning)
        );
        assert_eq!(DiagnosticArea::from_storage("unknown"), None);
        assert_eq!(DiagnosticSeverity::from_storage("fatal"), None);
    }

    #[test]
    fn diagnostic_event_validates_bounded_safe_fields() {
        assert_eq!(
            DiagnosticCode::new(" ").expect_err("blank code"),
            DiagnosticValidationError::Code
        );
        assert_eq!(
            DiagnosticSummary::new(" ").expect_err("blank summary"),
            DiagnosticValidationError::Summary
        );
        assert_eq!(
            DiagnosticContext::new(" ").expect_err("blank context"),
            DiagnosticValidationError::Context
        );

        let context = DiagnosticContext::new(r#"{"source":"antigravity","status":"failed"}"#)
            .expect("context");
        let event = DiagnosticEvent::new(
            DiagnosticArea::Collector,
            DiagnosticSeverity::Warning,
            DiagnosticCode::new("collector.source_failed").expect("code"),
            DiagnosticSummary::new("A source failed during collection.").expect("summary"),
            Some(context),
            100,
        )
        .expect("event");

        assert_eq!(event.area, DiagnosticArea::Collector);
        assert_eq!(event.created_at_ms, 100);
    }
}
