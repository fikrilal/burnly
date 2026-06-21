use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HistoryDeletionSnapshot {
    pub daily_usage: u64,
    pub daily_model_usage: u64,
    pub sessions: u64,
    pub session_model_usage: u64,
    pub refresh_runs: u64,
    pub import_runs: u64,
    pub projects: u64,
    pub source_models: u64,
    pub notification_records: u64,
    pub source_count: u64,
    pub earliest_date: Option<String>,
    pub latest_date: Option<String>,
    pub active_refresh: bool,
}

impl HistoryDeletionSnapshot {
    pub(crate) fn total_records(&self) -> u64 {
        self.daily_usage
            .saturating_add(self.daily_model_usage)
            .saturating_add(self.sessions)
            .saturating_add(self.session_model_usage)
            .saturating_add(self.refresh_runs)
            .saturating_add(self.import_runs)
            .saturating_add(self.projects)
            .saturating_add(self.source_models)
            .saturating_add(self.notification_records)
    }
}

pub(crate) trait HistoryDeletionStore: Send + Sync {
    fn preview(&self) -> Result<HistoryDeletionSnapshot, HistoryDeletionStoreError>;
    fn delete(
        &self,
        expected: &HistoryDeletionSnapshot,
    ) -> Result<HistoryDeletionSnapshot, HistoryDeletionStoreError>;
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HistoryDeletionStoreError {
    #[error("history deletion storage is unavailable")]
    Unavailable,
    #[error("history deletion values are invalid")]
    InvalidStoredValue,
    #[error("history deletion preview is stale")]
    StalePreview,
    #[error("history deletion is blocked by an active refresh")]
    ActiveRefresh,
}
