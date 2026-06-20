use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ExportDataset {
    DailyUsage,
    Sessions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExportScope {
    pub start_date: String,
    pub end_date: String,
    pub datasets: Vec<ExportDataset>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExportCounts {
    pub daily_usage: u64,
    pub sessions: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExportRow {
    pub dataset: ExportDataset,
    pub occurred_at: ExportOccurrence,
    pub source: String,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_creation_tokens: Option<u64>,
    pub cache_read_tokens: Option<u64>,
    pub total_tokens: u64,
    pub cost_amount_micros: Option<u64>,
    pub cost_currency: Option<String>,
    pub cost_status: String,
    pub data_quality: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExportOccurrence {
    Date(String),
    TimestampMs(i64),
}

pub(crate) trait ExportStore: Send + Sync {
    fn counts(&self, scope: &ExportScope) -> Result<ExportCounts, ExportStoreError>;
    fn rows(&self, scope: &ExportScope) -> Result<Vec<ExportRow>, ExportStoreError>;
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExportStoreError {
    #[error("export storage is unavailable")]
    Unavailable,
    #[error("export storage contains invalid values")]
    InvalidStoredValue,
}
