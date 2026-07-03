//! Application-owned diagnostic event model.
//!
//! Diagnostic events are local, redacted breadcrumbs. They intentionally carry
//! stable codes and bounded context instead of raw external payloads.

#![allow(
    dead_code,
    reason = "The diagnostics event vocabulary is introduced before every area is wired"
)]

use thiserror::Error;

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
        validate_bounded_text(&value, MAX_CODE_LEN, DiagnosticValidationError::InvalidCode)?;
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
        validate_bounded_text(
            &value,
            MAX_SUMMARY_LEN,
            DiagnosticValidationError::InvalidSummary,
        )?;
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
            return Err(DiagnosticValidationError::InvalidContext);
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
            return Err(DiagnosticValidationError::InvalidCreatedAt);
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
            return Err(DiagnosticValidationError::InvalidId);
        }
        Ok(Self { id, event })
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DiagnosticValidationError {
    #[error("diagnostic event id must be positive")]
    InvalidId,
    #[error("diagnostic event code must be non-empty and bounded")]
    InvalidCode,
    #[error("diagnostic event summary must be non-empty and bounded")]
    InvalidSummary,
    #[error("diagnostic event context must be a bounded JSON object")]
    InvalidContext,
    #[error("diagnostic event created_at_ms must be non-negative")]
    InvalidCreatedAt,
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
            DiagnosticValidationError::InvalidCode
        );
        assert_eq!(
            DiagnosticSummary::new(" ").expect_err("blank summary"),
            DiagnosticValidationError::InvalidSummary
        );
        assert_eq!(
            DiagnosticContext::new(" ").expect_err("blank context"),
            DiagnosticValidationError::InvalidContext
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
