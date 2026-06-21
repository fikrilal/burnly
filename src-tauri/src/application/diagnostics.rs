use std::sync::Arc;

use crate::application::ports::diagnostics_store::{
    DiagnosticsStore, DiagnosticsStoreError, SourceDiagnosticRecord,
};
use crate::application::ports::log_reveal::{
    LogRevealAvailability, LogRevealError, LogRevealOutcome, LogRevealPort,
};
use crate::application::ports::settings_store::{SettingsStore, SettingsStoreError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DiagnosticsStatus {
    pub status: HealthStatus,
    pub components: Vec<DiagnosticComponent>,
    pub logs: LogDiagnostics,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LogDiagnostics {
    pub status: LogRevealAvailability,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DiagnosticComponent {
    pub component: DiagnosticComponentKind,
    pub status: HealthStatus,
    pub summary: String,
    pub details: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DiagnosticComponentKind {
    Database,
    Settings,
    Sources,
    Collector,
    Runtime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum HealthStatus {
    Healthy,
    Unknown,
    Degraded,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeDiagnosticRecord {
    pub app_version: String,
    pub contract_version: u16,
    pub collector_initialized: bool,
}

pub(crate) struct DiagnosticsService {
    diagnostics_store: Arc<dyn DiagnosticsStore>,
    settings_store: Arc<dyn SettingsStore>,
    log_reveal: Arc<dyn LogRevealPort>,
    runtime: RuntimeDiagnosticRecord,
}

impl DiagnosticsService {
    pub(crate) fn new(
        diagnostics_store: Arc<dyn DiagnosticsStore>,
        settings_store: Arc<dyn SettingsStore>,
        log_reveal: Arc<dyn LogRevealPort>,
        runtime: RuntimeDiagnosticRecord,
    ) -> Self {
        Self {
            diagnostics_store,
            settings_store,
            log_reveal,
            runtime,
        }
    }

    pub(crate) fn status(&self) -> DiagnosticsStatus {
        let components = vec![
            self.database_status(),
            self.settings_status(),
            self.source_status(),
            self.collector_status(),
            self.runtime_status(),
        ];
        let status = aggregate_status(&components);
        DiagnosticsStatus {
            status,
            components,
            logs: self.log_status(),
        }
    }

    pub(crate) fn reveal_logs(&self) -> Result<LogRevealOutcome, LogRevealError> {
        self.log_reveal.reveal_logs()
    }

    fn database_status(&self) -> DiagnosticComponent {
        match self.diagnostics_store.database() {
            Ok(record) => DiagnosticComponent::healthy(
                DiagnosticComponentKind::Database,
                "Database is reachable.",
                vec![format!("Schema version {}", record.schema_version)],
            ),
            Err(error) => DiagnosticComponent::from_store_error(
                DiagnosticComponentKind::Database,
                error,
                "Database is unavailable.",
                "Database contains invalid diagnostic values.",
            ),
        }
    }

    fn settings_status(&self) -> DiagnosticComponent {
        match self.settings_store.get() {
            Ok(document) => DiagnosticComponent::healthy(
                DiagnosticComponentKind::Settings,
                "Settings are readable.",
                vec![
                    format!(
                        "Reporting timezone {}",
                        document.settings().reporting_timezone()
                    ),
                    format!("Settings revision {}", document.revision()),
                ],
            ),
            Err(SettingsStoreError::Unavailable) => DiagnosticComponent::unavailable(
                DiagnosticComponentKind::Settings,
                "Settings are unavailable.",
                vec!["Settings storage could not be read.".to_owned()],
            ),
            Err(SettingsStoreError::InvalidStoredValue) => DiagnosticComponent::degraded(
                DiagnosticComponentKind::Settings,
                "Settings contain invalid stored values.",
                vec!["Stored settings need recovery before updates can be trusted.".to_owned()],
            ),
            Err(SettingsStoreError::Conflict) => DiagnosticComponent::degraded(
                DiagnosticComponentKind::Settings,
                "Settings changed during diagnostics.",
                vec!["Retry diagnostics after the current settings update completes.".to_owned()],
            ),
        }
    }

    fn source_status(&self) -> DiagnosticComponent {
        match self.diagnostics_store.sources() {
            Ok(record) => source_component(record),
            Err(error) => DiagnosticComponent::from_store_error(
                DiagnosticComponentKind::Sources,
                error,
                "Source status is unavailable.",
                "Source records contain invalid diagnostic values.",
            ),
        }
    }

    fn collector_status(&self) -> DiagnosticComponent {
        if self.runtime.collector_initialized {
            DiagnosticComponent::healthy(
                DiagnosticComponentKind::Collector,
                "Collector runtime is initialized.",
                vec!["Collector health is available after startup initialization.".to_owned()],
            )
        } else {
            DiagnosticComponent::unknown(
                DiagnosticComponentKind::Collector,
                "Collector runtime has not reported status.",
                vec!["Collector status is unknown before initialization completes.".to_owned()],
            )
        }
    }

    fn runtime_status(&self) -> DiagnosticComponent {
        DiagnosticComponent::healthy(
            DiagnosticComponentKind::Runtime,
            "Runtime is initialized.",
            vec![
                format!("App version {}", self.runtime.app_version),
                format!("IPC contract version {}", self.runtime.contract_version),
            ],
        )
    }

    fn log_status(&self) -> LogDiagnostics {
        let capability = self.log_reveal.capability();
        let redactor = DiagnosticRedactor;
        LogDiagnostics {
            status: capability.status,
            label: redactor.redact(&capability.label),
        }
    }
}

impl DiagnosticComponent {
    fn healthy(
        component: DiagnosticComponentKind,
        summary: impl Into<String>,
        details: Vec<String>,
    ) -> Self {
        Self::new(component, HealthStatus::Healthy, summary, details)
    }

    fn unknown(
        component: DiagnosticComponentKind,
        summary: impl Into<String>,
        details: Vec<String>,
    ) -> Self {
        Self::new(component, HealthStatus::Unknown, summary, details)
    }

    fn degraded(
        component: DiagnosticComponentKind,
        summary: impl Into<String>,
        details: Vec<String>,
    ) -> Self {
        Self::new(component, HealthStatus::Degraded, summary, details)
    }

    fn unavailable(
        component: DiagnosticComponentKind,
        summary: impl Into<String>,
        details: Vec<String>,
    ) -> Self {
        Self::new(component, HealthStatus::Unavailable, summary, details)
    }

    fn from_store_error(
        component: DiagnosticComponentKind,
        error: DiagnosticsStoreError,
        unavailable_summary: &'static str,
        invalid_summary: &'static str,
    ) -> Self {
        match error {
            DiagnosticsStoreError::Unavailable => Self::unavailable(
                component,
                unavailable_summary,
                vec!["Storage could not be read.".to_owned()],
            ),
            DiagnosticsStoreError::InvalidStoredValue => Self::degraded(
                component,
                invalid_summary,
                vec!["Stored diagnostic counters are outside the supported range.".to_owned()],
            ),
        }
    }

    fn new(
        component: DiagnosticComponentKind,
        status: HealthStatus,
        summary: impl Into<String>,
        details: Vec<String>,
    ) -> Self {
        let redactor = DiagnosticRedactor;
        Self {
            component,
            status,
            summary: redactor.redact(&summary.into()),
            details: details
                .into_iter()
                .map(|detail| redactor.redact(&detail))
                .collect(),
        }
    }
}

fn source_component(record: SourceDiagnosticRecord) -> DiagnosticComponent {
    if record.configured_count == 0 {
        return DiagnosticComponent::unknown(
            DiagnosticComponentKind::Sources,
            "No sources are configured.",
            vec!["Configure a supported source before usage can be collected.".to_owned()],
        );
    }

    if record.enabled_count == 0 {
        return DiagnosticComponent::degraded(
            DiagnosticComponentKind::Sources,
            "Sources are configured but disabled.",
            vec![format!("Configured sources {}", record.configured_count)],
        );
    }

    DiagnosticComponent::healthy(
        DiagnosticComponentKind::Sources,
        "Sources are configured.",
        vec![
            format!("Detected sources {}", record.detected_count),
            format!("Configured sources {}", record.configured_count),
            format!("Enabled sources {}", record.enabled_count),
        ],
    )
}

fn aggregate_status(components: &[DiagnosticComponent]) -> HealthStatus {
    components
        .iter()
        .map(|component| component.status)
        .max()
        .unwrap_or(HealthStatus::Unknown)
}

pub(crate) struct DiagnosticRedactor;

impl DiagnosticRedactor {
    pub(crate) fn redact(&self, value: &str) -> String {
        let mut redacted = value
            .split_whitespace()
            .map(redact_token)
            .collect::<Vec<_>>()
            .join(" ");
        if redacted.len() > 240 {
            redacted.truncate(240);
            redacted.push('…');
        }
        redacted
    }
}

fn redact_token(token: &str) -> String {
    let trimmed = token.trim_matches(|character: char| {
        matches!(character, ',' | ';' | ')' | ']' | '}' | '"' | '\'')
    });
    if looks_like_path(trimmed) {
        return token.replace(trimmed, "[redacted-path]");
    }
    if looks_like_credential(trimmed) {
        return token.replace(trimmed, "[redacted-secret]");
    }
    if looks_like_session_id(trimmed) {
        return token.replace(trimmed, "[redacted-id]");
    }
    token.to_owned()
}

fn looks_like_path(value: &str) -> bool {
    value.starts_with('/')
        || value.starts_with("~/")
        || value.starts_with("\\\\")
        || (value.len() > 3
            && value.as_bytes()[1] == b':'
            && value.as_bytes()[2] == b'\\'
            && value.as_bytes()[0].is_ascii_alphabetic())
}

fn looks_like_credential(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.starts_with("sk-")
        || lower.starts_with("ghp_")
        || lower.starts_with("github_pat_")
        || lower.starts_with("xoxb-")
        || lower.contains("token=")
        || lower.contains("api_key=")
        || lower.contains("apikey=")
        || lower.contains("authorization:")
}

fn looks_like_session_id(value: &str) -> bool {
    if value.len() < 32 {
        return false;
    }
    let separators = value
        .chars()
        .filter(|character| matches!(character, '-' | '_'))
        .count();
    let identifier_chars = value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .count();
    identifier_chars >= 28 && separators <= 6
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::application::ports::diagnostics_store::DatabaseDiagnosticRecord;
    use crate::application::ports::log_reveal::LogRevealCapability;
    use crate::application::ports::settings_store::ProjectPathRetentionResult;
    use crate::domain::settings::{Settings, SettingsDocument};

    struct FakeDiagnosticsStore {
        database: Result<DatabaseDiagnosticRecord, DiagnosticsStoreError>,
        sources: Result<SourceDiagnosticRecord, DiagnosticsStoreError>,
    }

    impl DiagnosticsStore for FakeDiagnosticsStore {
        fn database(&self) -> Result<DatabaseDiagnosticRecord, DiagnosticsStoreError> {
            self.database.clone()
        }

        fn sources(&self) -> Result<SourceDiagnosticRecord, DiagnosticsStoreError> {
            self.sources.clone()
        }
    }

    struct FakeSettingsStore {
        result: Mutex<Result<SettingsDocument, SettingsStoreError>>,
    }

    impl SettingsStore for FakeSettingsStore {
        fn get(&self) -> Result<SettingsDocument, SettingsStoreError> {
            self.result.lock().expect("settings result").clone()
        }

        fn replace(
            &self,
            _expected_revision: i64,
            _settings: &Settings,
            _updated_at_ms: i64,
        ) -> Result<SettingsDocument, SettingsStoreError> {
            Err(SettingsStoreError::Unavailable)
        }

        fn replace_project_path_retention(
            &self,
            _expected_revision: i64,
            _retain_paths: bool,
            _updated_at_ms: i64,
        ) -> Result<ProjectPathRetentionResult, SettingsStoreError> {
            Err(SettingsStoreError::Unavailable)
        }
    }

    struct FakeLogReveal {
        capability: LogRevealCapability,
        outcome: Mutex<Result<LogRevealOutcome, LogRevealError>>,
    }

    impl LogRevealPort for FakeLogReveal {
        fn capability(&self) -> LogRevealCapability {
            self.capability.clone()
        }

        fn reveal_logs(&self) -> Result<LogRevealOutcome, LogRevealError> {
            *self.outcome.lock().expect("log reveal outcome")
        }
    }

    #[test]
    fn status_aggregates_component_health() {
        let service = service(
            Ok(DatabaseDiagnosticRecord { schema_version: 1 }),
            Ok(SourceDiagnosticRecord {
                detected_count: 0,
                configured_count: 1,
                enabled_count: 0,
            }),
            Ok(settings_document()),
            true,
        );

        let status = service.status();

        assert_eq!(status.status, HealthStatus::Degraded);
        assert_eq!(status.components.len(), 5);
        assert_eq!(status.logs.status, LogRevealAvailability::Available);
        assert_eq!(status.logs.label, "Burnly logs");
        assert!(status.components.iter().any(|component| {
            component.component == DiagnosticComponentKind::Sources
                && component.status == HealthStatus::Degraded
        }));
    }

    #[test]
    fn log_capability_is_redacted_and_reveal_outcome_is_delegated() {
        let service = DiagnosticsService::new(
            Arc::new(FakeDiagnosticsStore {
                database: Ok(DatabaseDiagnosticRecord { schema_version: 1 }),
                sources: Ok(SourceDiagnosticRecord {
                    detected_count: 0,
                    configured_count: 0,
                    enabled_count: 0,
                }),
            }),
            Arc::new(FakeSettingsStore {
                result: Mutex::new(Ok(settings_document())),
            }),
            Arc::new(FakeLogReveal {
                capability: LogRevealCapability {
                    status: LogRevealAvailability::Available,
                    label: "Burnly logs at /home/fikrilal/.config/burnly/logs".to_owned(),
                },
                outcome: Mutex::new(Ok(LogRevealOutcome::Revealed)),
            }),
            RuntimeDiagnosticRecord {
                app_version: "0.1.0".to_owned(),
                contract_version: 1,
                collector_initialized: true,
            },
        );

        let status = service.status();

        assert_eq!(status.logs.status, LogRevealAvailability::Available);
        assert!(!status.logs.label.contains("/home/fikrilal"));
        assert!(status.logs.label.contains("[redacted-path]"));
        assert_eq!(service.reveal_logs(), Ok(LogRevealOutcome::Revealed));
    }

    #[test]
    fn unavailable_database_dominates_over_unknown_components() {
        let service = service(
            Err(DiagnosticsStoreError::Unavailable),
            Ok(SourceDiagnosticRecord {
                detected_count: 0,
                configured_count: 0,
                enabled_count: 0,
            }),
            Ok(settings_document()),
            false,
        );

        assert_eq!(service.status().status, HealthStatus::Unavailable);
    }

    #[test]
    fn redactor_removes_sensitive_paths_credentials_and_full_ids() {
        let redactor = DiagnosticRedactor;
        let value = redactor.redact(
            "path /home/fikrilal/devs/private/project windows C:\\Users\\Dante\\secret token=sk-live-123456789 session 018f5f4d-7758-7bb2-9d9b-6d7f22c4a901 prompt write payment code",
        );

        assert!(!value.contains("/home/fikrilal"));
        assert!(!value.contains("C:\\Users"));
        assert!(!value.contains("sk-live"));
        assert!(!value.contains("018f5f4d-7758-7bb2-9d9b-6d7f22c4a901"));
        assert!(value.contains("[redacted-path]"));
        assert!(value.contains("[redacted-secret]"));
        assert!(value.contains("[redacted-id]"));
    }

    fn service(
        database: Result<DatabaseDiagnosticRecord, DiagnosticsStoreError>,
        sources: Result<SourceDiagnosticRecord, DiagnosticsStoreError>,
        settings: Result<SettingsDocument, SettingsStoreError>,
        collector_initialized: bool,
    ) -> DiagnosticsService {
        DiagnosticsService::new(
            Arc::new(FakeDiagnosticsStore { database, sources }),
            Arc::new(FakeSettingsStore {
                result: Mutex::new(settings),
            }),
            Arc::new(FakeLogReveal {
                capability: LogRevealCapability {
                    status: LogRevealAvailability::Available,
                    label: "Burnly logs".to_owned(),
                },
                outcome: Mutex::new(Ok(LogRevealOutcome::Revealed)),
            }),
            RuntimeDiagnosticRecord {
                app_version: "0.1.0".to_owned(),
                contract_version: 1,
                collector_initialized,
            },
        )
    }

    fn settings_document() -> SettingsDocument {
        SettingsDocument::new(
            Settings::new("UTC".to_owned(), false, 15, false, "quit", false, false)
                .expect("valid settings"),
            1,
        )
        .expect("valid document")
    }
}
