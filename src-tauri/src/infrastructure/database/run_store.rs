//! SQLite implementation of the run lifecycle store.
//!
//! Owns the SQL for source resolution and refresh/import run records. Run rows
//! store only stable codes and bounded summaries; raw collector output, paths,
//! and session identifiers never reach this table set.

#![allow(
    dead_code,
    reason = "The SQLite run store is constructed by the Phase 4E refresh coordinator wiring"
)]

use std::sync::Mutex;

use rusqlite::{params, Connection};

use crate::application::collection::{CollectionProjection, CollectionScope};
use crate::application::ports::run_store::{RunStore, RunStoreError};
use crate::application::reconciliation::{
    ImportOutcome, ImportRunCompletion, ImportRunId, ImportRunSpec, RefreshOutcome,
    RefreshRunCompletion, RefreshRunId, RefreshRunSpec, RefreshTrigger, RunError, SourceId,
};
use crate::domain::source::SourceKey;

use super::Database;

pub(crate) struct SqliteRunStore {
    database: Mutex<Database>,
}

impl SqliteRunStore {
    pub(crate) fn new(database: Database) -> Self {
        Self {
            database: Mutex::new(database),
        }
    }

    fn with_connection<T>(
        &self,
        operation: impl FnOnce(&Connection) -> Result<T, RunStoreError>,
    ) -> Result<T, RunStoreError> {
        let database = self.database.lock().map_err(|_| RunStoreError::Backend)?;
        operation(database.connection())
    }
}

impl RunStore for SqliteRunStore {
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
}

fn run_found(changed: usize) -> Result<(), RunStoreError> {
    if changed == 0 {
        Err(RunStoreError::RunNotFound)
    } else {
        Ok(())
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

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::*;
    use crate::application::reconciliation::{ImportCollector, JobKey};

    fn migrated_store() -> (tempfile::TempDir, SqliteRunStore) {
        let directory = tempfile::TempDir::new().expect("create temporary directory");
        let database_path = directory.path().join("burnly.sqlite3");
        let mut database = Database::open(&database_path).expect("open database");
        database.migrate_to_latest().expect("migrate database");

        (directory, SqliteRunStore::new(database))
    }

    fn refresh_spec(job_key: &str) -> RefreshRunSpec {
        RefreshRunSpec::new(
            JobKey::new(job_key).expect("job key"),
            RefreshTrigger::Manual,
            "0.1.0",
        )
        .expect("refresh spec")
    }

    fn daily_import_spec(refresh_run_id: RefreshRunId, source_id: SourceId) -> ImportRunSpec {
        ImportRunSpec::new(
            refresh_run_id,
            source_id,
            ImportCollector::new("ccusage", "20.0.11", 1).expect("collector"),
            CollectionProjection::Daily,
            CollectionScope::Full,
            Some("Asia/Jakarta".to_owned()),
        )
        .expect("import spec")
    }

    #[test]
    fn resolves_source_get_or_create_is_idempotent() {
        let (_directory, store) = migrated_store();

        let first = store
            .resolve_source(SourceKey::ClaudeCode, 100)
            .expect("first resolve");
        let second = store
            .resolve_source(SourceKey::ClaudeCode, 200)
            .expect("second resolve");

        assert_eq!(first, second);
    }

    #[test]
    fn refresh_run_lifecycle_reaches_a_terminal_status() {
        let (_directory, store) = migrated_store();

        let refresh_run_id = store
            .begin_refresh_run(refresh_spec("refresh-1"), 100)
            .expect("begin refresh run");

        store
            .complete_refresh_run(
                refresh_run_id,
                RefreshRunCompletion {
                    outcome: RefreshOutcome::Succeeded,
                    finished_at_ms: 200,
                    error: None,
                },
            )
            .expect("complete refresh run");
    }

    #[test]
    fn import_run_lifecycle_records_counts_and_redacted_error() {
        let (_directory, store) = migrated_store();

        let refresh_run_id = store
            .begin_refresh_run(refresh_spec("refresh-1"), 100)
            .expect("begin refresh run");
        let source_id = store
            .resolve_source(SourceKey::ClaudeCode, 100)
            .expect("resolve source");

        let import_run_id = store
            .begin_import_run(daily_import_spec(refresh_run_id, source_id), 110)
            .expect("begin import run");

        store
            .complete_import_run(
                import_run_id,
                ImportRunCompletion {
                    outcome: ImportOutcome::Partial,
                    records_seen: 12,
                    records_rejected: 3,
                    finished_at_ms: 180,
                    error: Some(
                        RunError::new("collector.partial", "some records were rejected")
                            .expect("run error"),
                    ),
                },
            )
            .expect("complete import run");
    }

    #[test]
    fn incremental_import_run_persists_scope_dates() {
        let (_directory, store) = migrated_store();

        let refresh_run_id = store
            .begin_refresh_run(refresh_spec("refresh-1"), 100)
            .expect("begin refresh run");
        let source_id = store
            .resolve_source(SourceKey::ClaudeCode, 100)
            .expect("resolve source");
        let scope = CollectionScope::incremental(
            NaiveDate::from_ymd_opt(2026, 6, 1).expect("start"),
            NaiveDate::from_ymd_opt(2026, 6, 14).expect("end"),
        )
        .expect("incremental scope");
        let spec = ImportRunSpec::new(
            refresh_run_id,
            source_id,
            ImportCollector::new("ccusage", "20.0.11", 1).expect("collector"),
            CollectionProjection::Daily,
            scope,
            Some("UTC".to_owned()),
        )
        .expect("incremental import spec");

        store
            .begin_import_run(spec, 110)
            .expect("begin incremental import run");
    }

    #[test]
    fn duplicate_job_key_is_rejected() {
        let (_directory, store) = migrated_store();

        store
            .begin_refresh_run(refresh_spec("refresh-1"), 100)
            .expect("first refresh run");
        let error = store
            .begin_refresh_run(refresh_spec("refresh-1"), 150)
            .expect_err("duplicate job key");

        assert_eq!(error, RunStoreError::DuplicateJobKey);
    }

    #[test]
    fn completing_a_missing_run_reports_not_found() {
        let (_directory, store) = migrated_store();

        let error = store
            .complete_refresh_run(
                RefreshRunId::new(999),
                RefreshRunCompletion {
                    outcome: RefreshOutcome::Failed,
                    finished_at_ms: 200,
                    error: None,
                },
            )
            .expect_err("missing refresh run");

        assert_eq!(error, RunStoreError::RunNotFound);
    }
}
