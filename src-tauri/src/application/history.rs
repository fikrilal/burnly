use std::sync::Arc;

use thiserror::Error;

use crate::application::diagnostics::DiagnosticRedactor;
use crate::application::ports::clock::Clock;
use crate::application::ports::history_store::{
    HistoryQuery, HistoryStore, HistoryStoreError, StoredHistoryPage, StoredImportRun,
    StoredRefreshRun,
};

const DEFAULT_LIMIT: usize = 10;
const MAX_LIMIT: usize = 50;
const STALE_AFTER_MS: i64 = 60 * 60 * 1_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HistoryRequest {
    pub cursor: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HistoryPage {
    pub items: Vec<RefreshHistoryItem>,
    pub next_cursor: Option<String>,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RefreshHistoryItem {
    pub trigger: HistoryTrigger,
    pub status: HistoryStatus,
    pub summary: String,
    pub started_at_ms: i64,
    pub finished_at_ms: Option<i64>,
    pub import_count: u32,
    pub records_seen: u64,
    pub records_rejected: u64,
    pub failure: Option<HistoryFailure>,
    pub imports: Vec<ImportHistoryItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ImportHistoryItem {
    pub source: String,
    pub projection: HistoryProjection,
    pub scope: HistoryScope,
    pub status: HistoryStatus,
    pub started_at_ms: i64,
    pub finished_at_ms: Option<i64>,
    pub records_seen: u64,
    pub records_rejected: u64,
    pub failure: Option<HistoryFailure>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HistoryFailure {
    pub category: FailureCategory,
    pub retryable: bool,
    pub summary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HistoryTrigger {
    Launch,
    Manual,
    Scheduled,
    FileChange,
    Resume,
    Reconcile,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HistoryProjection {
    Daily,
    Session,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HistoryScope {
    Full,
    Incremental,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HistoryStatus {
    Queued,
    Running,
    Stale,
    Succeeded,
    Partial,
    Failed,
    Cancelled,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FailureCategory {
    Collector,
    Reconciliation,
    Persistence,
    Cancelled,
    Unknown,
}

pub(crate) struct HistoryService {
    store: Arc<dyn HistoryStore>,
    clock: Arc<dyn Clock>,
}

impl HistoryService {
    pub(crate) fn new(store: Arc<dyn HistoryStore>, clock: Arc<dyn Clock>) -> Self {
        Self { store, clock }
    }

    pub(crate) fn history(&self, request: HistoryRequest) -> Result<HistoryPage, HistoryError> {
        let limit = request.limit.unwrap_or(DEFAULT_LIMIT);
        if !(1..=MAX_LIMIT).contains(&limit) {
            return Err(HistoryError::InvalidLimit);
        }
        let before_refresh_id = request
            .cursor
            .as_deref()
            .map(str::parse::<i64>)
            .transpose()
            .map_err(|_| HistoryError::InvalidCursor)?;
        if before_refresh_id.is_some_and(|cursor| cursor <= 0) {
            return Err(HistoryError::InvalidCursor);
        }

        let stored = self.store.history(HistoryQuery {
            before_refresh_id,
            limit,
        })?;
        self.map_page(stored, limit)
    }

    fn map_page(
        &self,
        stored: StoredHistoryPage,
        limit: usize,
    ) -> Result<HistoryPage, HistoryError> {
        let next_cursor = stored
            .has_more
            .then(|| {
                stored
                    .refreshes
                    .last()
                    .map(|refresh| refresh.id.to_string())
            })
            .flatten();
        let now_ms = self.clock.now_epoch_ms();
        let items = stored
            .refreshes
            .into_iter()
            .map(|refresh| map_refresh(refresh, now_ms))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(HistoryPage {
            items,
            next_cursor,
            limit,
        })
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HistoryError {
    #[error("history limit must be between 1 and 50")]
    InvalidLimit,
    #[error("history cursor is invalid")]
    InvalidCursor,
    #[error("history storage is unavailable")]
    Unavailable,
    #[error("history contains invalid stored values")]
    InvalidStoredValue,
}

impl From<HistoryStoreError> for HistoryError {
    fn from(value: HistoryStoreError) -> Self {
        match value {
            HistoryStoreError::Unavailable => Self::Unavailable,
            HistoryStoreError::InvalidStoredValue => Self::InvalidStoredValue,
        }
    }
}

fn map_refresh(value: StoredRefreshRun, now_ms: i64) -> Result<RefreshHistoryItem, HistoryError> {
    let imports = value
        .imports
        .into_iter()
        .map(|item| map_import(item, now_ms))
        .collect::<Result<Vec<_>, _>>()?;
    let records_seen = imports.iter().map(|item| item.records_seen).sum();
    let records_rejected = imports.iter().map(|item| item.records_rejected).sum();
    let import_count =
        u32::try_from(imports.len()).map_err(|_| HistoryError::InvalidStoredValue)?;
    let status = parse_status(
        &value.status,
        value.started_at_ms.or(Some(value.created_at_ms)),
        now_ms,
    )?;
    let started_at_ms = value.started_at_ms.unwrap_or(value.created_at_ms);
    let summary =
        format!("{import_count} imports; {records_seen} accepted; {records_rejected} rejected.");
    Ok(RefreshHistoryItem {
        trigger: parse_trigger(&value.trigger)?,
        status,
        summary,
        started_at_ms,
        finished_at_ms: value.finished_at_ms,
        import_count,
        records_seen,
        records_rejected,
        failure: map_failure(
            value.error_code.as_deref(),
            value.error_summary.as_deref(),
            status,
        ),
        imports,
    })
}

fn map_import(value: StoredImportRun, now_ms: i64) -> Result<ImportHistoryItem, HistoryError> {
    let status = parse_status(&value.status, Some(value.started_at_ms), now_ms)?;
    Ok(ImportHistoryItem {
        source: DiagnosticRedactor.redact(&value.source_name),
        projection: match value.projection.as_str() {
            "daily" => HistoryProjection::Daily,
            "session" => HistoryProjection::Session,
            _ => return Err(HistoryError::InvalidStoredValue),
        },
        scope: match value.scope.as_str() {
            "full" => HistoryScope::Full,
            "incremental" => HistoryScope::Incremental,
            _ => return Err(HistoryError::InvalidStoredValue),
        },
        status,
        started_at_ms: value.started_at_ms,
        finished_at_ms: value.finished_at_ms,
        records_seen: u64::try_from(value.records_seen)
            .map_err(|_| HistoryError::InvalidStoredValue)?,
        records_rejected: u64::try_from(value.records_rejected)
            .map_err(|_| HistoryError::InvalidStoredValue)?,
        failure: map_failure(
            value.error_code.as_deref(),
            value.error_detail.as_deref(),
            status,
        ),
    })
}

fn parse_trigger(value: &str) -> Result<HistoryTrigger, HistoryError> {
    match value {
        "launch" => Ok(HistoryTrigger::Launch),
        "manual" => Ok(HistoryTrigger::Manual),
        "scheduled" => Ok(HistoryTrigger::Scheduled),
        "file_change" => Ok(HistoryTrigger::FileChange),
        "resume" => Ok(HistoryTrigger::Resume),
        "reconcile" => Ok(HistoryTrigger::Reconcile),
        _ => Err(HistoryError::InvalidStoredValue),
    }
}

fn parse_status(
    value: &str,
    started_at_ms: Option<i64>,
    now_ms: i64,
) -> Result<HistoryStatus, HistoryError> {
    match value {
        "queued" => Ok(HistoryStatus::Queued),
        "running" | "cancelling"
            if started_at_ms
                .is_some_and(|started| now_ms.saturating_sub(started) >= STALE_AFTER_MS) =>
        {
            Ok(HistoryStatus::Stale)
        }
        "running" | "cancelling" => Ok(HistoryStatus::Running),
        "succeeded" => Ok(HistoryStatus::Succeeded),
        "partial" => Ok(HistoryStatus::Partial),
        "failed" => Ok(HistoryStatus::Failed),
        "cancelled" => Ok(HistoryStatus::Cancelled),
        _ => Err(HistoryError::InvalidStoredValue),
    }
}

fn map_failure(
    code: Option<&str>,
    detail: Option<&str>,
    status: HistoryStatus,
) -> Option<HistoryFailure> {
    if !matches!(
        status,
        HistoryStatus::Failed
            | HistoryStatus::Partial
            | HistoryStatus::Cancelled
            | HistoryStatus::Stale
    ) {
        return None;
    }
    let category = match code.unwrap_or_default() {
        value if value.starts_with("collector.") => FailureCategory::Collector,
        value if value.starts_with("refresh.reconciliation") => FailureCategory::Reconciliation,
        value if value.starts_with("database.") || value.starts_with("persistence.") => {
            FailureCategory::Persistence
        }
        value if value.contains("cancel") => FailureCategory::Cancelled,
        _ if status == HistoryStatus::Cancelled => FailureCategory::Cancelled,
        _ => FailureCategory::Unknown,
    };
    let retryable = matches!(
        category,
        FailureCategory::Collector | FailureCategory::Persistence | FailureCategory::Unknown
    ) || status == HistoryStatus::Stale;
    let fallback = match status {
        HistoryStatus::Stale => "Run did not reach a terminal state.",
        HistoryStatus::Cancelled => "Run was cancelled.",
        _ => "Run did not complete successfully.",
    };
    Some(HistoryFailure {
        category,
        retryable,
        summary: DiagnosticRedactor.redact(detail.unwrap_or(fallback)),
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    struct FakeClock(i64);
    impl Clock for FakeClock {
        fn now_epoch_ms(&self) -> i64 {
            self.0
        }
    }

    struct FakeStore(Mutex<Result<StoredHistoryPage, HistoryStoreError>>);
    impl HistoryStore for FakeStore {
        fn history(&self, _query: HistoryQuery) -> Result<StoredHistoryPage, HistoryStoreError> {
            self.0.lock().expect("lock store").clone()
        }
    }

    #[test]
    fn maps_safe_counts_failures_stale_state_and_cursor() {
        let service = HistoryService::new(
            Arc::new(FakeStore(Mutex::new(Ok(StoredHistoryPage {
                refreshes: vec![StoredRefreshRun {
                    id: 9,
                    trigger: "manual".to_owned(),
                    status: "running".to_owned(),
                    started_at_ms: Some(1_000),
                    finished_at_ms: None,
                    created_at_ms: 1_000,
                    error_code: None,
                    error_summary: None,
                    imports: vec![StoredImportRun {
                        source_name: "/home/dante/private".to_owned(),
                        projection: "daily".to_owned(),
                        scope: "full".to_owned(),
                        status: "partial".to_owned(),
                        records_seen: 12,
                        records_rejected: 2,
                        started_at_ms: 1_000,
                        finished_at_ms: Some(2_000),
                        error_code: Some("collector.timeout".to_owned()),
                        error_detail: Some(
                            "failed at /home/dante/private with sk-secret".to_owned(),
                        ),
                    }],
                }],
                has_more: true,
            })))),
            Arc::new(FakeClock(1_000 + STALE_AFTER_MS)),
        );

        let page = service
            .history(HistoryRequest {
                cursor: None,
                limit: Some(10),
            })
            .expect("history");
        assert_eq!(page.next_cursor.as_deref(), Some("9"));
        assert_eq!(page.items[0].status, HistoryStatus::Stale);
        assert_eq!(page.items[0].records_seen, 12);
        assert_eq!(page.items[0].imports[0].source, "[redacted-path]");
        assert_eq!(
            page.items[0].imports[0]
                .failure
                .as_ref()
                .expect("failure")
                .summary,
            "failed at [redacted-path] with [redacted-secret]"
        );
    }

    #[test]
    fn rejects_unbounded_limits_and_invalid_cursors() {
        let service = HistoryService::new(
            Arc::new(FakeStore(Mutex::new(Ok(StoredHistoryPage {
                refreshes: Vec::new(),
                has_more: false,
            })))),
            Arc::new(FakeClock(0)),
        );
        assert_eq!(
            service.history(HistoryRequest {
                cursor: None,
                limit: Some(51)
            }),
            Err(HistoryError::InvalidLimit)
        );
        assert_eq!(
            service.history(HistoryRequest {
                cursor: Some("invalid".to_owned()),
                limit: None
            }),
            Err(HistoryError::InvalidCursor)
        );
    }
}
