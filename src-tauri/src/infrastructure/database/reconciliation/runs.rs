//! Run lifecycle implementation: source resolution, refresh/import run
//! records, interrupted run recovery, and latest successful import lookup.

use chrono::NaiveDate;
use rusqlite::{params, OptionalExtension, Row};

use crate::application::collection::{CollectionProjection, CollectionScope};
use crate::application::ports::run_store::{RunStore, RunStoreError};
use crate::application::reconciliation::{
    ImportOutcome, ImportRunCompletion, ImportRunId, ImportRunLookup, ImportRunSpec,
    RefreshOutcome, RefreshRunCompletion, RefreshRunId, RefreshRunSpec, RefreshTrigger, RunError,
    SourceId, SuccessfulImportState,
};
use crate::domain::source::SourceKey;

use super::store::SqliteReconciliationStore;

pub(super) const INTERRUPTED_REFRESH_ERROR_CODE: &str = "refresh.interrupted";
const INTERRUPTED_REFRESH_ERROR_SUMMARY: &str =
    "The previous refresh was interrupted before it completed.";
pub(super) const INTERRUPTED_IMPORT_ERROR_CODE: &str = "import.interrupted";
const INTERRUPTED_IMPORT_ERROR_DETAIL: &str =
    "The previous import was interrupted before it completed.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct InterruptedRunRecovery {
    pub(crate) refresh_runs: usize,
    pub(crate) import_runs: usize,
}

impl SqliteReconciliationStore {
    pub(crate) fn recover_interrupted_runs(
        &self,
        finished_at_ms: i64,
    ) -> Result<InterruptedRunRecovery, RunStoreError> {
        let mut database = self.database.lock().map_err(|_| RunStoreError::Backend)?;
        let transaction = database
            .connection_mut()
            .transaction()
            .map_err(|_| RunStoreError::Backend)?;

        let import_runs = transaction
            .execute(
                "UPDATE import_runs
                SET status = 'failed',
                    finished_at_ms = CASE
                        WHEN started_at_ms > ?1 THEN started_at_ms
                        ELSE ?1
                    END,
                    error_code = ?2,
                    error_detail = ?3
                WHERE status = 'running'",
                params![
                    finished_at_ms,
                    INTERRUPTED_IMPORT_ERROR_CODE,
                    INTERRUPTED_IMPORT_ERROR_DETAIL,
                ],
            )
            .map_err(|_| RunStoreError::Backend)?;

        let refresh_runs = transaction
            .execute(
                "UPDATE refresh_runs
                SET status = 'failed',
                    finished_at_ms = CASE
                        WHEN started_at_ms IS NOT NULL AND started_at_ms > ?1 THEN started_at_ms
                        ELSE ?1
                    END,
                    error_code = ?2,
                    error_summary = ?3
                WHERE status IN ('queued', 'running', 'cancelling')",
                params![
                    finished_at_ms,
                    INTERRUPTED_REFRESH_ERROR_CODE,
                    INTERRUPTED_REFRESH_ERROR_SUMMARY,
                ],
            )
            .map_err(|_| RunStoreError::Backend)?;

        transaction.commit().map_err(|_| RunStoreError::Backend)?;

        Ok(InterruptedRunRecovery {
            refresh_runs,
            import_runs,
        })
    }
}

impl RunStore for SqliteReconciliationStore {
    fn resolve_source(&self, source: SourceKey, now_ms: i64) -> Result<SourceId, RunStoreError> {
        self.with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO sources (
                        source_key, display_name, enabled, detection_state,
                        created_at_ms, updated_at_ms
                    ) VALUES (?1, ?2, 1, 'unknown', ?3, ?3)
                    ON CONFLICT(source_key) DO NOTHING",
                    params![source.as_str(), source.as_str(), now_ms],
                )
                .map_err(|_| RunStoreError::Backend)?;

            let id: i64 = connection
                .query_row(
                    "SELECT id FROM sources WHERE source_key = ?1",
                    params![source.as_str()],
                    |row| row.get(0),
                )
                .map_err(|_| RunStoreError::Backend)?;

            Ok(SourceId::new(id))
        })
    }

    fn begin_refresh_run(
        &self,
        spec: RefreshRunSpec,
        now_ms: i64,
    ) -> Result<RefreshRunId, RunStoreError> {
        self.with_connection(|connection| {
            let result = connection.execute(
                "INSERT INTO refresh_runs (
                    job_key, trigger, status, started_at_ms,
                    requested_by_app_version, created_at_ms
                ) VALUES (?1, ?2, 'running', ?3, ?4, ?3)",
                params![
                    spec.job_key().as_str(),
                    refresh_trigger_value(spec.trigger()),
                    now_ms,
                    spec.requested_by_app_version(),
                ],
            );

            match result {
                Ok(_) => Ok(RefreshRunId::new(connection.last_insert_rowid())),
                Err(error) if is_unique_violation(&error) => Err(RunStoreError::DuplicateJobKey),
                Err(_) => Err(RunStoreError::Backend),
            }
        })
    }

    fn complete_refresh_run(
        &self,
        id: RefreshRunId,
        completion: RefreshRunCompletion,
    ) -> Result<(), RunStoreError> {
        let (error_code, error_summary) = error_fields(completion.error.as_ref());

        self.with_connection(|connection| {
            let changed = connection
                .execute(
                    "UPDATE refresh_runs
                    SET status = ?2, finished_at_ms = ?3, error_code = ?4, error_summary = ?5
                    WHERE id = ?1",
                    params![
                        id.value(),
                        refresh_outcome_value(completion.outcome),
                        completion.finished_at_ms,
                        error_code,
                        error_summary,
                    ],
                )
                .map_err(|_| RunStoreError::Backend)?;

            run_found(changed)
        })
    }

    fn begin_import_run(
        &self,
        spec: ImportRunSpec,
        started_at_ms: i64,
    ) -> Result<ImportRunId, RunStoreError> {
        let (scope_kind, scope_start, scope_end) = scope_fields(spec.scope());

        self.with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO import_runs (
                        refresh_run_id, source_id, collector_key, collector_version,
                        profile_version, projection, scope_kind, scope_start_date,
                        scope_end_date, aggregation_timezone, status,
                        records_seen, records_rejected, started_at_ms
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'running', 0, 0, ?11)",
                    params![
                        spec.refresh_run_id().value(),
                        spec.source_id().value(),
                        spec.collector_key(),
                        spec.collector_version(),
                        i64::from(spec.profile_version()),
                        projection_value(spec.projection()),
                        scope_kind,
                        scope_start,
                        scope_end,
                        spec.aggregation_timezone(),
                        started_at_ms,
                    ],
                )
                .map_err(|_| RunStoreError::Backend)?;

            Ok(ImportRunId::new(connection.last_insert_rowid()))
        })
    }

    fn complete_import_run(
        &self,
        id: ImportRunId,
        completion: ImportRunCompletion,
    ) -> Result<(), RunStoreError> {
        let (error_code, error_detail) = error_fields(completion.error.as_ref());

        self.with_connection(|connection| {
            let changed = connection
                .execute(
                    "UPDATE import_runs
                    SET status = ?2, records_seen = ?3, records_rejected = ?4,
                        finished_at_ms = ?5, error_code = ?6, error_detail = ?7
                    WHERE id = ?1",
                    params![
                        id.value(),
                        import_outcome_value(completion.outcome),
                        i64::from(completion.records_seen),
                        i64::from(completion.records_rejected),
                        completion.finished_at_ms,
                        error_code,
                        error_detail,
                    ],
                )
                .map_err(|_| RunStoreError::Backend)?;

            run_found(changed)
        })
    }

    fn latest_successful_import(
        &self,
        lookup: ImportRunLookup,
    ) -> Result<Option<SuccessfulImportState>, RunStoreError> {
        self.with_connection(|connection| {
            let projection = projection_value(lookup.projection());
            let row = match lookup.aggregation_timezone() {
                Some(timezone) => connection
                    .query_row(
                        "SELECT import_runs.scope_kind, import_runs.scope_start_date,
                            import_runs.scope_end_date, import_runs.finished_at_ms
                        FROM import_runs
                        INNER JOIN sources ON sources.id = import_runs.source_id
                        WHERE sources.source_key = ?1
                            AND import_runs.projection = ?2
                            AND import_runs.aggregation_timezone = ?3
                            AND (?4 IS NULL OR import_runs.collector_key = ?4)
                            AND (?5 IS NULL OR import_runs.profile_version = ?5)
                            AND import_runs.status = 'succeeded'
                        ORDER BY import_runs.finished_at_ms DESC, import_runs.id DESC
                        LIMIT 1",
                        params![
                            lookup.source().as_str(),
                            projection,
                            timezone,
                            lookup.collector_key(),
                            lookup.profile_version().map(i64::from),
                        ],
                        import_run_scope_row,
                    )
                    .optional()
                    .map_err(|_| RunStoreError::Backend)?,
                None => connection
                    .query_row(
                        "SELECT import_runs.scope_kind, import_runs.scope_start_date,
                            import_runs.scope_end_date, import_runs.finished_at_ms
                        FROM import_runs
                        INNER JOIN sources ON sources.id = import_runs.source_id
                        WHERE sources.source_key = ?1
                            AND import_runs.projection = ?2
                            AND (?3 IS NULL OR import_runs.collector_key = ?3)
                            AND (?4 IS NULL OR import_runs.profile_version = ?4)
                            AND import_runs.status = 'succeeded'
                        ORDER BY import_runs.finished_at_ms DESC, import_runs.id DESC
                        LIMIT 1",
                        params![
                            lookup.source().as_str(),
                            projection,
                            lookup.collector_key(),
                            lookup.profile_version().map(i64::from),
                        ],
                        import_run_scope_row,
                    )
                    .optional()
                    .map_err(|_| RunStoreError::Backend)?,
            };

            row.map(|(kind, start, end, finished_at_ms)| {
                import_state_from_row(
                    &lookup,
                    &kind,
                    start.as_deref(),
                    end.as_deref(),
                    finished_at_ms,
                )
            })
            .transpose()
        })
    }
}

fn error_fields(error: Option<&RunError>) -> (Option<&str>, Option<&str>) {
    match error {
        Some(error) => (Some(error.code()), Some(error.summary())),
        None => (None, None),
    }
}

fn scope_fields(scope: &CollectionScope) -> (&'static str, Option<String>, Option<String>) {
    match scope {
        CollectionScope::Full => ("full", None, None),
        CollectionScope::Incremental(incremental) => (
            "incremental",
            Some(incremental.start_date().to_string()),
            Some(incremental.end_date().to_string()),
        ),
    }
}

fn import_run_scope_row(
    row: &Row<'_>,
) -> rusqlite::Result<(String, Option<String>, Option<String>, i64)> {
    Ok((
        row.get::<_, String>(0)?,
        row.get::<_, Option<String>>(1)?,
        row.get::<_, Option<String>>(2)?,
        row.get::<_, i64>(3)?,
    ))
}

fn import_state_from_row(
    lookup: &ImportRunLookup,
    scope_kind: &str,
    scope_start_date: Option<&str>,
    scope_end_date: Option<&str>,
    finished_at_ms: i64,
) -> Result<SuccessfulImportState, RunStoreError> {
    let scope = match scope_kind {
        "full" => CollectionScope::Full,
        "incremental" => CollectionScope::incremental(
            parse_scope_date(scope_start_date)?,
            parse_scope_date(scope_end_date)?,
        )
        .map_err(|_| RunStoreError::Backend)?,
        _ => return Err(RunStoreError::Backend),
    };

    Ok(SuccessfulImportState::new(
        lookup.source(),
        lookup.projection(),
        scope,
        finished_at_ms,
    ))
}

fn parse_scope_date(value: Option<&str>) -> Result<NaiveDate, RunStoreError> {
    let value = value.ok_or(RunStoreError::Backend)?;
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| RunStoreError::Backend)
}

const fn refresh_trigger_value(trigger: RefreshTrigger) -> &'static str {
    match trigger {
        RefreshTrigger::Launch => "launch",
        RefreshTrigger::Manual => "manual",
        RefreshTrigger::Scheduled => "scheduled",
        RefreshTrigger::FileChange => "file_change",
        RefreshTrigger::Resume => "resume",
        RefreshTrigger::Reconcile => "reconcile",
    }
}

const fn refresh_outcome_value(outcome: RefreshOutcome) -> &'static str {
    match outcome {
        RefreshOutcome::Succeeded => "succeeded",
        RefreshOutcome::Partial => "partial",
        RefreshOutcome::Failed => "failed",
        RefreshOutcome::Cancelled => "cancelled",
    }
}

const fn import_outcome_value(outcome: ImportOutcome) -> &'static str {
    match outcome {
        ImportOutcome::Succeeded => "succeeded",
        ImportOutcome::Partial => "partial",
        ImportOutcome::Failed => "failed",
        ImportOutcome::Cancelled => "cancelled",
    }
}

const fn projection_value(projection: CollectionProjection) -> &'static str {
    match projection {
        CollectionProjection::Daily => "daily",
        CollectionProjection::Session => "session",
    }
}

fn is_unique_violation(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(failure, _)
            if failure.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE
    )
}

fn run_found(changed: usize) -> Result<(), RunStoreError> {
    if changed == 0 {
        Err(RunStoreError::RunNotFound)
    } else {
        Ok(())
    }
}
