use std::sync::Mutex;

use rusqlite::{Connection, TransactionBehavior};

use crate::application::ports::history_deletion_store::{
    HistoryDeletionSnapshot, HistoryDeletionStore, HistoryDeletionStoreError,
};

use super::Database;

pub(crate) struct SqliteHistoryDeletionStore {
    database: Mutex<Database>,
}

impl SqliteHistoryDeletionStore {
    pub(crate) fn new(database: Database) -> Self {
        Self {
            database: Mutex::new(database),
        }
    }
}

impl HistoryDeletionStore for SqliteHistoryDeletionStore {
    fn preview(&self) -> Result<HistoryDeletionSnapshot, HistoryDeletionStoreError> {
        let database = self
            .database
            .lock()
            .map_err(|_| HistoryDeletionStoreError::Unavailable)?;
        read_snapshot(database.connection())
    }

    fn delete(
        &self,
        expected: &HistoryDeletionSnapshot,
    ) -> Result<HistoryDeletionSnapshot, HistoryDeletionStoreError> {
        let mut database = self
            .database
            .lock()
            .map_err(|_| HistoryDeletionStoreError::Unavailable)?;
        let transaction = database
            .connection_mut()
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| HistoryDeletionStoreError::Unavailable)?;
        let current = read_snapshot(&transaction)?;
        if &current != expected {
            return Err(HistoryDeletionStoreError::StalePreview);
        }
        if current.active_refresh {
            return Err(HistoryDeletionStoreError::ActiveRefresh);
        }

        transaction
            .execute("DELETE FROM budget_notification_state", [])
            .map_err(|_| HistoryDeletionStoreError::Unavailable)?;
        transaction
            .execute("DELETE FROM daily_usage", [])
            .map_err(|_| HistoryDeletionStoreError::Unavailable)?;
        transaction
            .execute("DELETE FROM sessions", [])
            .map_err(|_| HistoryDeletionStoreError::Unavailable)?;
        transaction
            .execute("DELETE FROM refresh_runs", [])
            .map_err(|_| HistoryDeletionStoreError::Unavailable)?;
        transaction
            .execute("DELETE FROM projects", [])
            .map_err(|_| HistoryDeletionStoreError::Unavailable)?;
        transaction
            .execute("DELETE FROM source_models", [])
            .map_err(|_| HistoryDeletionStoreError::Unavailable)?;
        transaction
            .commit()
            .map_err(|_| HistoryDeletionStoreError::Unavailable)?;
        Ok(current)
    }
}

fn read_snapshot(
    connection: &Connection,
) -> Result<HistoryDeletionSnapshot, HistoryDeletionStoreError> {
    let row = connection
        .query_row(
            "SELECT
            (SELECT COUNT(*) FROM daily_usage),
            (SELECT COUNT(*) FROM daily_model_usage),
            (SELECT COUNT(*) FROM sessions),
            (SELECT COUNT(*) FROM session_model_usage),
            (SELECT COUNT(*) FROM refresh_runs),
            (SELECT COUNT(*) FROM import_runs),
            (SELECT COUNT(*) FROM projects),
            (SELECT COUNT(*) FROM source_models),
            (SELECT COUNT(*) FROM budget_notification_state),
            (SELECT COUNT(DISTINCT source_id) FROM (
                SELECT source_id FROM daily_usage UNION ALL SELECT source_id FROM sessions
                UNION ALL SELECT source_id FROM import_runs UNION ALL SELECT source_id FROM projects
                UNION ALL SELECT source_id FROM source_models
            )),
            (SELECT MIN(activity_date) FROM (
                SELECT usage_date AS activity_date FROM daily_usage
                UNION ALL SELECT date(first_activity_at_ms / 1000, 'unixepoch') FROM sessions
            )),
            (SELECT MAX(activity_date) FROM (
                SELECT usage_date AS activity_date FROM daily_usage
                UNION ALL SELECT date(first_activity_at_ms / 1000, 'unixepoch') FROM sessions
            )),
            EXISTS(SELECT 1 FROM refresh_runs WHERE status IN ('queued', 'running', 'cancelling'))",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, i64>(8)?,
                    row.get::<_, i64>(9)?,
                    row.get::<_, Option<String>>(10)?,
                    row.get::<_, Option<String>>(11)?,
                    row.get::<_, bool>(12)?,
                ))
            },
        )
        .map_err(|_| HistoryDeletionStoreError::Unavailable)?;
    Ok(HistoryDeletionSnapshot {
        daily_usage: count(row.0)?,
        daily_model_usage: count(row.1)?,
        sessions: count(row.2)?,
        session_model_usage: count(row.3)?,
        refresh_runs: count(row.4)?,
        import_runs: count(row.5)?,
        projects: count(row.6)?,
        source_models: count(row.7)?,
        notification_records: count(row.8)?,
        source_count: count(row.9)?,
        earliest_date: row.10,
        latest_date: row.11,
        active_refresh: row.12,
    })
}

fn count(value: i64) -> Result<u64, HistoryDeletionStoreError> {
    u64::try_from(value).map_err(|_| HistoryDeletionStoreError::InvalidStoredValue)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::database::test_database::TestDatabase;

    #[test]
    fn deletes_imported_history_and_preserves_owned_configuration() {
        let test_database = seeded_database();
        let store = SqliteHistoryDeletionStore::new(
            Database::open(test_database.path()).expect("reopen database"),
        );
        let preview = store.preview().expect("preview");
        assert_eq!(preview.daily_usage, 1);
        assert_eq!(preview.sessions, 1);
        assert_eq!(preview.notification_records, 1);
        assert_eq!(preview.earliest_date.as_deref(), Some("2026-06-10"));

        let deleted = store.delete(&preview).expect("delete history");
        assert_eq!(deleted, preview);
        let after = store.preview().expect("preview after delete");
        assert_eq!(after.total_records(), 0);

        let database = Database::open(test_database.path()).expect("inspect database");
        let connection = database.connection();
        for table in ["sources", "app_settings", "budgets", "budget_thresholds"] {
            let count: i64 = connection
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .expect("count preserved table");
            assert_eq!(count, 1, "{table} must be preserved");
        }
    }

    #[test]
    fn rolls_back_every_delete_when_a_later_step_fails() {
        let test_database = seeded_database();
        test_database
            .database()
            .connection()
            .execute_batch(
                "CREATE TRIGGER fail_project_delete BEFORE DELETE ON projects
             BEGIN SELECT RAISE(ABORT, 'simulated failure'); END;",
            )
            .expect("install failure trigger");
        let store = SqliteHistoryDeletionStore::new(
            Database::open(test_database.path()).expect("reopen database"),
        );
        let preview = store.preview().expect("preview");
        assert_eq!(
            store.delete(&preview),
            Err(HistoryDeletionStoreError::Unavailable)
        );
        assert_eq!(store.preview().expect("preview after rollback"), preview);
    }

    fn seeded_database() -> TestDatabase {
        let mut database = TestDatabase::open();
        database
            .database_mut()
            .migrate_to_latest()
            .expect("migrate database");
        database.database().connection().execute_batch(
            "INSERT INTO sources (id, source_key, display_name, enabled, detection_state, created_at_ms, updated_at_ms)
             VALUES (1, 'claude-code', 'Claude Code', 1, 'available', 0, 0);
             INSERT INTO source_models (id, source_id, raw_model_id, display_name, first_seen_at_ms, last_seen_at_ms)
             VALUES (1, 1, 'model', 'Model', 0, 1);
             INSERT INTO projects (id, source_id, identity_key, identity_kind, display_name, first_seen_at_ms, last_seen_at_ms)
             VALUES (1, 1, 'project', 'label', 'Project', 0, 1);
             INSERT INTO refresh_runs (id, job_key, trigger, status, started_at_ms, finished_at_ms, requested_by_app_version, created_at_ms)
             VALUES (1, 'job-1', 'manual', 'succeeded', 0, 1, '0.1.0', 0);
             INSERT INTO import_runs (id, refresh_run_id, source_id, collector_key, collector_version, profile_version, projection, scope_kind, aggregation_timezone, status, records_seen, records_rejected, started_at_ms, finished_at_ms)
             VALUES (1, 1, 1, 'ccusage', '1.0.0', 1, 'daily', 'full', 'UTC', 'succeeded', 2, 0, 0, 1);
             INSERT INTO daily_usage (id, source_id, source_key, identity_version, usage_date, aggregation_timezone, project_id, total_tokens, cost_kind, cost_status, data_quality, record_state, absence_count, first_seen_at_ms, last_seen_at_ms, latest_import_id)
             VALUES (1, 1, 'day', 1, '2026-06-10', 'UTC', 1, 12, 'unknown', 'unavailable', 'complete', 'active', 0, 1, 1, 1);
             INSERT INTO daily_model_usage (daily_usage_id, source_id, model_id, total_tokens, cost_status, latest_import_id)
             VALUES (1, 1, 1, 12, 'unavailable', 1);
             INSERT INTO sessions (id, source_id, source_key, identity_version, source_session_id, project_id, first_activity_at_ms, last_activity_at_ms, total_tokens, cost_kind, cost_status, data_quality, record_state, absence_count, first_seen_at_ms, last_seen_at_ms, latest_import_id)
             VALUES (1, 1, 'session', 1, 'private-session', 1, 1781136000000, 1781136000000, 20, 'unknown', 'unavailable', 'complete', 'active', 0, 1, 1, 1);
             INSERT INTO session_model_usage (session_id, source_id, model_id, total_tokens, cost_status, latest_import_id)
             VALUES (1, 1, 1, 20, 'unavailable', 1);
             INSERT INTO app_settings (id, reporting_timezone, background_refresh_enabled, refresh_interval_minutes, launch_at_login, close_behavior, notifications_enabled, store_project_paths, created_at_ms, updated_at_ms)
             VALUES (1, 'UTC', 0, 15, 0, 'quit', 1, 0, 0, 0);
             INSERT INTO budgets (id, name, metric, period, limit_value, enabled, created_at_ms, updated_at_ms)
             VALUES (1, 'Monthly', 'tokens', 'monthly', 1000, 1, 0, 0);
             INSERT INTO budget_thresholds (budget_id, threshold_bps, enabled) VALUES (1, 8000, 1);
             INSERT INTO budget_notification_state (budget_id, period_start_date, aggregation_timezone, threshold_bps, observed_value, notified_at_ms, delivery_status)
             VALUES (1, '2026-06-01', 'UTC', 8000, 800, 1, 'delivered');",
        ).expect("seed history and configuration");
        database
    }
}
