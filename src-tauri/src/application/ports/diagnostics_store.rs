#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DatabaseDiagnosticRecord {
    pub schema_version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceDiagnosticRecord {
    pub detected_count: u32,
    pub configured_count: u32,
    pub enabled_count: u32,
}

pub(crate) trait DiagnosticsStore: Send + Sync {
    fn database(&self) -> Result<DatabaseDiagnosticRecord, DiagnosticsStoreError>;
    fn sources(&self) -> Result<SourceDiagnosticRecord, DiagnosticsStoreError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DiagnosticsStoreError {
    Unavailable,
    InvalidStoredValue,
}
