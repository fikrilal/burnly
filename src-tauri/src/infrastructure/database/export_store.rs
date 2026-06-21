use std::sync::Mutex;

use rusqlite::{params, Connection, Row};

use crate::application::ports::export_store::{
    ExportCounts, ExportDataset, ExportOccurrence, ExportRow, ExportScope, ExportStore,
    ExportStoreError,
};

use super::Database;

pub(crate) struct SqliteExportStore {
    database: Mutex<Database>,
}

impl SqliteExportStore {
    pub(crate) fn new(database: Database) -> Self {
        Self {
            database: Mutex::new(database),
        }
    }
}

impl ExportStore for SqliteExportStore {
    fn counts(&self, scope: &ExportScope) -> Result<ExportCounts, ExportStoreError> {
        let database = self
            .database
            .lock()
            .map_err(|_| ExportStoreError::Unavailable)?;
        let connection = database.connection();
        let daily_usage = if scope.datasets.contains(&ExportDataset::DailyUsage) {
            count_daily(connection, scope)?
        } else {
            0
        };
        let sessions = if scope.datasets.contains(&ExportDataset::Sessions) {
            count_sessions(connection, scope)?
        } else {
            0
        };
        Ok(ExportCounts {
            daily_usage,
            sessions,
        })
    }

    fn rows(&self, scope: &ExportScope) -> Result<Vec<ExportRow>, ExportStoreError> {
        let database = self
            .database
            .lock()
            .map_err(|_| ExportStoreError::Unavailable)?;
        let connection = database.connection();
        let mut rows = Vec::new();
        if scope.datasets.contains(&ExportDataset::DailyUsage) {
            rows.extend(read_daily(connection, scope)?);
        }
        if scope.datasets.contains(&ExportDataset::Sessions) {
            rows.extend(read_sessions(connection, scope)?);
        }
        Ok(rows)
    }
}

fn count_daily(connection: &Connection, scope: &ExportScope) -> Result<u64, ExportStoreError> {
    count(connection, "SELECT COUNT(*) FROM daily_usage WHERE usage_date BETWEEN ?1 AND ?2 AND record_state != 'removed'", scope)
}

fn count_sessions(connection: &Connection, scope: &ExportScope) -> Result<u64, ExportStoreError> {
    count(connection, "SELECT COUNT(*) FROM sessions WHERE date(first_activity_at_ms / 1000, 'unixepoch') BETWEEN ?1 AND ?2 AND record_state != 'removed'", scope)
}

fn count(connection: &Connection, sql: &str, scope: &ExportScope) -> Result<u64, ExportStoreError> {
    let value: i64 = connection
        .query_row(sql, params![scope.start_date, scope.end_date], |row| {
            row.get(0)
        })
        .map_err(|_| ExportStoreError::Unavailable)?;
    u64::try_from(value).map_err(|_| ExportStoreError::InvalidStoredValue)
}

fn read_daily(
    connection: &Connection,
    scope: &ExportScope,
) -> Result<Vec<ExportRow>, ExportStoreError> {
    let mut statement = connection
        .prepare(
            "SELECT daily_usage.usage_date, sources.display_name,
                daily_usage.input_tokens, daily_usage.output_tokens,
                daily_usage.cache_creation_tokens, daily_usage.cache_read_tokens,
                daily_usage.total_tokens, daily_usage.cost_amount_micros,
                daily_usage.cost_currency, daily_usage.cost_status, daily_usage.data_quality
         FROM daily_usage INNER JOIN sources ON sources.id = daily_usage.source_id
         WHERE daily_usage.usage_date BETWEEN ?1 AND ?2 AND daily_usage.record_state != 'removed'
         ORDER BY daily_usage.usage_date, daily_usage.id",
        )
        .map_err(|_| ExportStoreError::Unavailable)?;
    let mapped = statement
        .query_map(params![scope.start_date, scope.end_date], |row| {
            map_row(
                row,
                ExportDataset::DailyUsage,
                ExportOccurrence::Date(row.get(0)?),
                1,
            )
        })
        .map_err(|_| ExportStoreError::Unavailable)?;
    mapped
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| ExportStoreError::InvalidStoredValue)
        .and_then(validate_rows)
}

fn read_sessions(
    connection: &Connection,
    scope: &ExportScope,
) -> Result<Vec<ExportRow>, ExportStoreError> {
    let mut statement = connection
        .prepare(
            "SELECT sessions.first_activity_at_ms, sources.display_name,
                sessions.input_tokens, sessions.output_tokens,
                sessions.cache_creation_tokens, sessions.cache_read_tokens,
                sessions.total_tokens, sessions.cost_amount_micros,
                sessions.cost_currency, sessions.cost_status, sessions.data_quality
         FROM sessions INNER JOIN sources ON sources.id = sessions.source_id
         WHERE date(sessions.first_activity_at_ms / 1000, 'unixepoch') BETWEEN ?1 AND ?2
               AND sessions.record_state != 'removed'
         ORDER BY sessions.first_activity_at_ms, sessions.id",
        )
        .map_err(|_| ExportStoreError::Unavailable)?;
    let mapped = statement
        .query_map(params![scope.start_date, scope.end_date], |row| {
            map_row(
                row,
                ExportDataset::Sessions,
                ExportOccurrence::TimestampMs(row.get(0)?),
                1,
            )
        })
        .map_err(|_| ExportStoreError::Unavailable)?;
    mapped
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| ExportStoreError::InvalidStoredValue)
        .and_then(validate_rows)
}

fn map_row(
    row: &Row<'_>,
    dataset: ExportDataset,
    occurred_at: ExportOccurrence,
    offset: usize,
) -> rusqlite::Result<ExportRow> {
    Ok(ExportRow {
        dataset,
        occurred_at,
        source: row.get(offset)?,
        input_tokens: optional_u64(row.get(offset + 1)?)?,
        output_tokens: optional_u64(row.get(offset + 2)?)?,
        cache_creation_tokens: optional_u64(row.get(offset + 3)?)?,
        cache_read_tokens: optional_u64(row.get(offset + 4)?)?,
        total_tokens: required_u64(row.get(offset + 5)?)?,
        cost_amount_micros: optional_u64(row.get(offset + 6)?)?,
        cost_currency: row.get(offset + 7)?,
        cost_status: row.get(offset + 8)?,
        data_quality: row.get(offset + 9)?,
    })
}

fn optional_u64(value: Option<i64>) -> rusqlite::Result<Option<u64>> {
    value
        .map(|value| {
            u64::try_from(value)
                .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))
        })
        .transpose()
}

fn required_u64(value: i64) -> rusqlite::Result<u64> {
    u64::try_from(value).map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))
}

fn validate_rows(rows: Vec<ExportRow>) -> Result<Vec<ExportRow>, ExportStoreError> {
    if rows
        .iter()
        .any(|row| row.source.trim().is_empty() || row.data_quality.trim().is_empty())
    {
        Err(ExportStoreError::InvalidStoredValue)
    } else {
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::database::test_database::TestDatabase;

    #[test]
    fn previews_and_reads_only_selected_in_range_non_removed_rows() {
        let mut test_database = TestDatabase::open();
        test_database
            .database_mut()
            .migrate_to_latest()
            .expect("migrate database");
        let connection = test_database.database().connection();
        connection.execute_batch(
            "INSERT INTO sources (id, source_key, display_name, enabled, detection_state, created_at_ms, updated_at_ms)
             VALUES (1, 'claude-code', 'Claude Code', 1, 'available', 0, 0);
             INSERT INTO refresh_runs (id, job_key, trigger, status, started_at_ms, finished_at_ms, requested_by_app_version, created_at_ms)
             VALUES (1, 'job-1', 'manual', 'succeeded', 0, 1, '0.1.0', 0);
             INSERT INTO import_runs (id, refresh_run_id, source_id, collector_key, collector_version, profile_version, projection, scope_kind, aggregation_timezone, status, records_seen, records_rejected, started_at_ms, finished_at_ms)
             VALUES (1, 1, 1, 'ccusage', '1.0.0', 1, 'daily', 'full', 'UTC', 'succeeded', 1, 0, 0, 1);
             INSERT INTO daily_usage (source_id, source_key, identity_version, usage_date, aggregation_timezone, total_tokens, cost_kind, cost_status, data_quality, record_state, absence_count, first_seen_at_ms, last_seen_at_ms, removed_at_ms, latest_import_id)
             VALUES (1, 'day-active', 1, '2026-06-10', 'UTC', 12, 'unknown', 'unavailable', 'complete', 'active', 0, 1, 1, NULL, 1),
                    (1, 'day-removed', 1, '2026-06-11', 'UTC', 99, 'unknown', 'unavailable', 'complete', 'removed', 2, 1, 2, 2, 1);
             INSERT INTO sessions (source_id, source_key, identity_version, source_session_id, first_activity_at_ms, last_activity_at_ms, total_tokens, cost_kind, cost_status, data_quality, record_state, absence_count, first_seen_at_ms, last_seen_at_ms, latest_import_id)
             VALUES (1, 'session-active', 1, 'sensitive-session-id', 1781136000000, 1781136000000, 20, 'unknown', 'unavailable', 'complete', 'active', 0, 1, 1, 1);",
        ).expect("seed export rows");
        let store =
            SqliteExportStore::new(Database::open(test_database.path()).expect("reopen database"));
        let scope = ExportScope {
            start_date: "2026-06-01".to_owned(),
            end_date: "2026-06-30".to_owned(),
            datasets: vec![ExportDataset::DailyUsage, ExportDataset::Sessions],
        };

        assert_eq!(
            store.counts(&scope).expect("counts"),
            ExportCounts {
                daily_usage: 1,
                sessions: 1
            }
        );
        let rows = store.rows(&scope).expect("rows");
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|row| row.total_tokens != 99));
        assert!(rows.iter().all(|row| !row.source.contains("session-id")));
    }
}
