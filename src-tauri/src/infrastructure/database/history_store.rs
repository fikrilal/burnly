use std::sync::Mutex;

use rusqlite::{params, Connection};

use crate::application::ports::history_store::{
    HistoryQuery, HistoryStore, HistoryStoreError, StoredHistoryPage, StoredImportRun,
    StoredRefreshRun,
};

use super::Database;

pub(crate) struct SqliteHistoryStore {
    database: Mutex<Database>,
}

impl SqliteHistoryStore {
    pub(crate) fn new(database: Database) -> Self {
        Self {
            database: Mutex::new(database),
        }
    }
}

impl HistoryStore for SqliteHistoryStore {
    fn history(&self, query: HistoryQuery) -> Result<StoredHistoryPage, HistoryStoreError> {
        let database = self
            .database
            .lock()
            .map_err(|_| HistoryStoreError::Unavailable)?;
        read_history(database.connection(), query)
    }
}

fn read_history(
    connection: &Connection,
    query: HistoryQuery,
) -> Result<StoredHistoryPage, HistoryStoreError> {
    let fetch_limit = i64::try_from(query.limit.saturating_add(1))
        .map_err(|_| HistoryStoreError::InvalidStoredValue)?;
    let mut statement = connection
        .prepare(
            "SELECT id, trigger, status, started_at_ms, finished_at_ms, created_at_ms,
                error_code, error_summary
         FROM refresh_runs
         WHERE (?1 IS NULL OR id < ?1)
         ORDER BY id DESC
         LIMIT ?2",
        )
        .map_err(|_| HistoryStoreError::Unavailable)?;
    let rows = statement
        .query_map(params![query.before_refresh_id, fetch_limit], |row| {
            Ok(StoredRefreshRun {
                id: row.get(0)?,
                trigger: row.get(1)?,
                status: row.get(2)?,
                started_at_ms: row.get(3)?,
                finished_at_ms: row.get(4)?,
                created_at_ms: row.get(5)?,
                error_code: row.get(6)?,
                error_summary: row.get(7)?,
                imports: Vec::new(),
            })
        })
        .map_err(|_| HistoryStoreError::Unavailable)?;
    let mut refreshes = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| HistoryStoreError::InvalidStoredValue)?;
    let has_more = refreshes.len() > query.limit;
    refreshes.truncate(query.limit);
    for refresh in &mut refreshes {
        refresh.imports = read_imports(connection, refresh.id)?;
    }
    Ok(StoredHistoryPage {
        refreshes,
        has_more,
    })
}

fn read_imports(
    connection: &Connection,
    refresh_id: i64,
) -> Result<Vec<StoredImportRun>, HistoryStoreError> {
    let mut statement = connection
        .prepare(
            "SELECT sources.display_name, import_runs.projection, import_runs.scope_kind,
                import_runs.status, import_runs.records_seen, import_runs.records_rejected,
                import_runs.started_at_ms, import_runs.finished_at_ms,
                import_runs.error_code, import_runs.error_detail
         FROM import_runs
         INNER JOIN sources ON sources.id = import_runs.source_id
         WHERE import_runs.refresh_run_id = ?1
         ORDER BY import_runs.id",
        )
        .map_err(|_| HistoryStoreError::Unavailable)?;
    let rows = statement
        .query_map([refresh_id], |row| {
            Ok(StoredImportRun {
                source_name: row.get(0)?,
                projection: row.get(1)?,
                scope: row.get(2)?,
                status: row.get(3)?,
                records_seen: row.get(4)?,
                records_rejected: row.get(5)?,
                started_at_ms: row.get(6)?,
                finished_at_ms: row.get(7)?,
                error_code: row.get(8)?,
                error_detail: row.get(9)?,
            })
        })
        .map_err(|_| HistoryStoreError::Unavailable)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|_| HistoryStoreError::InvalidStoredValue)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::database::test_database::TestDatabase;

    #[test]
    fn returns_bounded_newest_first_pages_with_imports() {
        let mut test_database = TestDatabase::open();
        test_database
            .database_mut()
            .migrate_to_latest()
            .expect("migrate database");
        let connection = test_database.database().connection();
        connection.execute(
            "INSERT INTO sources (id, source_key, display_name, enabled, detection_state, created_at_ms, updated_at_ms)
             VALUES (1, 'claude-code', 'Claude Code', 1, 'available', 1, 1)", [],
        ).expect("insert source");
        for id in 1..=3 {
            connection.execute(
                "INSERT INTO refresh_runs (id, job_key, trigger, status, started_at_ms, finished_at_ms, requested_by_app_version, created_at_ms)
                 VALUES (?1, ?2, 'manual', 'succeeded', ?3, ?4, '0.1.0', ?3)",
                params![id, format!("job-{id}"), id * 100, id * 100 + 10],
            ).expect("insert refresh");
            connection.execute(
                "INSERT INTO import_runs (refresh_run_id, source_id, collector_key, collector_version, profile_version, projection, scope_kind, aggregation_timezone, status, records_seen, records_rejected, started_at_ms, finished_at_ms)
                 VALUES (?1, 1, 'ccusage', '1.0.0', 1, 'daily', 'full', 'UTC', 'succeeded', ?2, 0, ?3, ?4)",
                params![id, id, id * 100, id * 100 + 10],
            ).expect("insert import");
        }
        let store =
            SqliteHistoryStore::new(Database::open(test_database.path()).expect("reopen database"));
        let first = store
            .history(HistoryQuery {
                before_refresh_id: None,
                limit: 2,
            })
            .expect("first page");
        assert_eq!(
            first.refreshes.iter().map(|row| row.id).collect::<Vec<_>>(),
            vec![3, 2]
        );
        assert!(first.has_more);
        assert_eq!(first.refreshes[0].imports[0].source_name, "Claude Code");
        let second = store
            .history(HistoryQuery {
                before_refresh_id: Some(2),
                limit: 2,
            })
            .expect("second page");
        assert_eq!(
            second
                .refreshes
                .iter()
                .map(|row| row.id)
                .collect::<Vec<_>>(),
            vec![1]
        );
        assert!(!second.has_more);
    }
}
