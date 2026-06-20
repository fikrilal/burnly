use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HistoryQuery {
    pub before_refresh_id: Option<i64>,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StoredHistoryPage {
    pub refreshes: Vec<StoredRefreshRun>,
    pub has_more: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StoredRefreshRun {
    pub id: i64,
    pub trigger: String,
    pub status: String,
    pub started_at_ms: Option<i64>,
    pub finished_at_ms: Option<i64>,
    pub created_at_ms: i64,
    pub error_code: Option<String>,
    pub error_summary: Option<String>,
    pub imports: Vec<StoredImportRun>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StoredImportRun {
    pub source_name: String,
    pub projection: String,
    pub scope: String,
    pub status: String,
    pub records_seen: i64,
    pub records_rejected: i64,
    pub started_at_ms: i64,
    pub finished_at_ms: Option<i64>,
    pub error_code: Option<String>,
    pub error_detail: Option<String>,
}

pub(crate) trait HistoryStore: Send + Sync {
    fn history(&self, query: HistoryQuery) -> Result<StoredHistoryPage, HistoryStoreError>;
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HistoryStoreError {
    #[error("history storage is unavailable")]
    Unavailable,
    #[error("history contains invalid stored values")]
    InvalidStoredValue,
}
