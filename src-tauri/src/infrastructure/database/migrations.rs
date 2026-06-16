use rusqlite::Connection;
use rusqlite_migration::{Migrations, M};

use super::PersistenceError;

const MIGRATION_LIST: &[M<'static>] =
    &[M::up(include_str!("../../../migrations/0001_initial.sql")).foreign_key_check()];
const MIGRATIONS: Migrations<'static> = Migrations::from_slice(MIGRATION_LIST);

pub(super) fn to_latest(connection: &mut Connection) -> Result<(), PersistenceError> {
    MIGRATIONS
        .to_latest(connection)
        .map_err(PersistenceError::migration)
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;

    use rusqlite::{params, Connection};
    use rusqlite_migration::{Error as MigrationError, MigrationDefinitionError, SchemaVersion};

    use super::*;
    use crate::infrastructure::database::error::PersistenceErrorKind;
    use crate::infrastructure::database::test_database::TestDatabase;

    #[test]
    fn migration_definition_is_valid() {
        MIGRATIONS.validate().expect("validate migrations");
    }

    #[test]
    fn fresh_database_migrates_to_latest() {
        let mut test_database = TestDatabase::open();

        test_database
            .database_mut()
            .migrate_to_latest()
            .expect("migrate database");

        assert_eq!(schema_version(test_database.database()), 1);
        assert_eq!(table_count(test_database.database()), 13);
        assert!(all_product_tables_are_strict(test_database.database()));
        assert_foreign_keys_clean(test_database.database());
        assert_integrity_ok(test_database.database());
    }

    #[test]
    fn repeated_migration_is_a_no_op() {
        let mut test_database = TestDatabase::open();

        test_database
            .database_mut()
            .migrate_to_latest()
            .expect("first migration");
        test_database
            .database_mut()
            .migrate_to_latest()
            .expect("second migration");

        assert_eq!(schema_version(test_database.database()), 1);
        assert_eq!(table_count(test_database.database()), 13);
    }

    #[test]
    fn newer_schema_is_rejected_without_changing_version() {
        let mut test_database = TestDatabase::open();
        test_database
            .database()
            .connection
            .pragma_update(None, "user_version", 2)
            .expect("set newer schema version");

        let error = test_database
            .database_mut()
            .migrate_to_latest()
            .expect_err("newer schema must fail");

        assert_eq!(error.kind(), PersistenceErrorKind::Migration);
        assert_eq!(schema_version(test_database.database()), 2);
    }

    #[test]
    fn failed_migration_preserves_previous_committed_version() {
        let migrations = Migrations::new(vec![
            M::up("CREATE TABLE stable (id INTEGER PRIMARY KEY) STRICT;"),
            M::up("CREATE TABLE broken (").foreign_key_check(),
        ]);
        let mut connection = Connection::open_in_memory().expect("open database");

        migrations.to_version(&mut connection, 1).expect("apply v1");
        let error = migrations
            .to_latest(&mut connection)
            .expect_err("invalid v2 must fail");

        assert!(matches!(error, MigrationError::RusqliteError { .. }));
        assert_eq!(
            migrations
                .current_version(&connection)
                .expect("read version"),
            SchemaVersion::Inside(NonZeroUsize::new(1).expect("nonzero"))
        );
        assert!(table_exists(&connection, "stable"));
        assert!(!table_exists(&connection, "broken"));
    }

    #[test]
    fn rejects_invalid_source_and_settings_values() {
        let mut test_database = migrated_database();
        let connection = &mut test_database.database_mut().connection;

        assert!(connection
            .execute(
                "INSERT INTO sources (
                    source_key, display_name, enabled, detection_state,
                    created_at_ms, updated_at_ms
                ) VALUES ('', 'Claude Code', 1, 'unknown', 0, 0)",
                [],
            )
            .is_err());
        assert!(connection
            .execute(
                "INSERT INTO app_settings (
                    id, reporting_timezone, background_refresh_enabled,
                    refresh_interval_minutes, launch_at_login, close_behavior,
                    notifications_enabled, store_project_paths,
                    created_at_ms, updated_at_ms
                ) VALUES (2, 'UTC', 1, 15, 0, 'hide', 1, 0, 0, 0)",
                [],
            )
            .is_err());
    }

    #[test]
    fn rejects_cross_source_project_reference() {
        let mut test_database = migrated_database();
        let connection = &mut test_database.database_mut().connection;
        insert_source(connection, 1, "claude-code");
        insert_source(connection, 2, "codex");
        connection
            .execute(
                "INSERT INTO projects (
                    id, source_id, identity_key, identity_kind,
                    first_seen_at_ms, last_seen_at_ms
                ) VALUES (1, 1, 'project-1', 'label', 0, 0)",
                [],
            )
            .expect("insert project");
        let import_id = insert_import_run(connection, 2);

        let result = connection.execute(
            "INSERT INTO daily_usage (
                source_id, source_key, identity_version, usage_date,
                aggregation_timezone, project_id, total_tokens,
                cost_kind, cost_status, data_quality, record_state,
                absence_count, first_seen_at_ms, last_seen_at_ms,
                latest_import_id
            ) VALUES (2, 'daily-1', 1, '2026-06-14', 'UTC', 1, 1,
                'unknown', 'unavailable', 'complete', 'active', 0, 0, 0, ?1)",
            [import_id],
        );

        assert!(result.is_err());
    }

    #[test]
    fn enforces_lifecycle_and_cost_consistency() {
        let mut test_database = migrated_database();
        let connection = &mut test_database.database_mut().connection;
        insert_source(connection, 1, "claude-code");
        let import_id = insert_import_run(connection, 1);

        let invalid_lifecycle = connection.execute(
            "INSERT INTO daily_usage (
                source_id, source_key, identity_version, usage_date,
                aggregation_timezone, total_tokens, cost_kind, cost_status,
                data_quality, record_state, absence_count, first_seen_at_ms,
                last_seen_at_ms, latest_import_id
            ) VALUES (1, 'daily-lifecycle', 1, '2026-06-14', 'UTC', 1,
                'unknown', 'unavailable', 'complete', 'removed', 0, 0, 0, ?1)",
            [import_id],
        );
        let invalid_cost = connection.execute(
            "INSERT INTO daily_usage (
                source_id, source_key, identity_version, usage_date,
                aggregation_timezone, total_tokens, cost_amount_micros,
                cost_kind, cost_status, data_quality, record_state,
                absence_count, first_seen_at_ms, last_seen_at_ms,
                latest_import_id
            ) VALUES (1, 'daily-cost', 1, '2026-06-14', 'UTC', 1, 100,
                'collector_calculated', 'estimated', 'complete', 'active', 0,
                0, 0, ?1)",
            [import_id],
        );

        assert!(invalid_lifecycle.is_err());
        assert!(invalid_cost.is_err());
    }

    #[test]
    fn rejects_negative_tokens_and_duplicate_session_identity() {
        let mut test_database = migrated_database();
        let connection = &mut test_database.database_mut().connection;
        insert_source(connection, 1, "claude-code");
        let import_id = insert_import_run(connection, 1);

        let negative_tokens = insert_session(
            connection,
            import_id,
            "session-negative",
            "source-negative",
            -1,
        );
        insert_session(connection, import_id, "session-one", "source-one", 1)
            .expect("insert session");
        let duplicate_identity =
            insert_session(connection, import_id, "session-two", "source-one", 1);

        assert!(negative_tokens.is_err());
        assert!(duplicate_identity.is_err());
    }

    #[test]
    fn enforces_budget_currency_and_notification_identity() {
        let mut test_database = migrated_database();
        let connection = &mut test_database.database_mut().connection;

        let invalid_token_currency = connection.execute(
            "INSERT INTO budgets (
                name, metric, period, limit_value, currency, enabled,
                created_at_ms, updated_at_ms
            ) VALUES ('Tokens', 'tokens', 'monthly', 100, 'USD', 1, 0, 0)",
            [],
        );
        let invalid_cost_currency = connection.execute(
            "INSERT INTO budgets (
                name, metric, period, limit_value, currency, enabled,
                created_at_ms, updated_at_ms
            ) VALUES ('Cost', 'cost', 'monthly', 100, 'usd', 1, 0, 0)",
            [],
        );

        connection
            .execute(
                "INSERT INTO budgets (
                    id, name, metric, period, limit_value, currency, enabled,
                    created_at_ms, updated_at_ms
                ) VALUES (1, 'Cost', 'cost', 'monthly', 100, 'USD', 1, 0, 0)",
                [],
            )
            .expect("insert budget");
        connection
            .execute(
                "INSERT INTO budget_thresholds (budget_id, threshold_bps, enabled)
                 VALUES (1, 8000, 1)",
                [],
            )
            .expect("insert threshold");
        let missing_threshold = connection.execute(
            "INSERT INTO budget_notification_state (
                budget_id, period_start_date, aggregation_timezone,
                threshold_bps, observed_value, notified_at_ms, delivery_status
            ) VALUES (1, '2026-06-01', 'UTC', 9000, 90, 0, 'delivered')",
            [],
        );

        assert!(invalid_token_currency.is_err());
        assert!(invalid_cost_currency.is_err());
        assert!(missing_threshold.is_err());
    }

    #[test]
    fn enforces_unknown_model_uniqueness_and_parent_cascade() {
        let mut test_database = migrated_database();
        let connection = &mut test_database.database_mut().connection;
        insert_source(connection, 1, "claude-code");
        let import_id = insert_import_run(connection, 1);
        let daily_id = insert_daily_usage(connection, import_id);

        connection
            .execute(
                "INSERT INTO daily_model_usage (
                    daily_usage_id, source_id, model_id, cost_status,
                    latest_import_id
                ) VALUES (?1, 1, NULL, 'unavailable', ?2)",
                params![daily_id, import_id],
            )
            .expect("insert unknown model row");
        let duplicate_unknown = connection.execute(
            "INSERT INTO daily_model_usage (
                daily_usage_id, source_id, model_id, cost_status,
                latest_import_id
            ) VALUES (?1, 1, NULL, 'unavailable', ?2)",
            params![daily_id, import_id],
        );

        assert!(duplicate_unknown.is_err());

        connection
            .execute("DELETE FROM daily_usage WHERE id = ?1", [daily_id])
            .expect("delete parent daily usage");
        let child_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM daily_model_usage", [], |row| {
                row.get(0)
            })
            .expect("count model rows");
        assert_eq!(child_count, 0);
    }

    fn migrated_database() -> TestDatabase {
        let mut database = TestDatabase::open();
        database
            .database_mut()
            .migrate_to_latest()
            .expect("migrate database");
        database
    }

    fn insert_source(connection: &Connection, id: i64, key: &str) {
        connection
            .execute(
                "INSERT INTO sources (
                    id, source_key, display_name, enabled, detection_state,
                    created_at_ms, updated_at_ms
                ) VALUES (?1, ?2, ?2, 1, 'available', 0, 0)",
                params![id, key],
            )
            .expect("insert source");
    }

    fn insert_import_run(connection: &Connection, source_id: i64) -> i64 {
        connection
            .execute(
                "INSERT INTO refresh_runs (
                    id, job_key, trigger, status, requested_by_app_version,
                    created_at_ms
                ) VALUES (1, 'job-1', 'manual', 'running', '0.1.0', 0)",
                [],
            )
            .ok();
        connection
            .execute(
                "INSERT INTO import_runs (
                    refresh_run_id, source_id, collector_key,
                    collector_version, profile_version, projection, scope_kind,
                    aggregation_timezone, status, records_seen,
                    records_rejected, started_at_ms
                ) VALUES (1, ?1, 'ccusage', '1.0.0', 1, 'daily', 'full',
                    'UTC', 'running', 0, 0, 0)",
                [source_id],
            )
            .expect("insert import run");
        connection.last_insert_rowid()
    }

    fn insert_session(
        connection: &Connection,
        import_id: i64,
        source_key: &str,
        source_session_id: &str,
        total_tokens: i64,
    ) -> rusqlite::Result<usize> {
        connection.execute(
            "INSERT INTO sessions (
                source_id, source_key, identity_version, source_session_id,
                total_tokens, cost_kind, cost_status, data_quality,
                record_state, absence_count, first_seen_at_ms,
                last_seen_at_ms, latest_import_id
            ) VALUES (1, ?1, 1, ?2, ?3, 'unknown', 'unavailable',
                'complete', 'active', 0, 0, 0, ?4)",
            params![source_key, source_session_id, total_tokens, import_id],
        )
    }

    fn insert_daily_usage(connection: &Connection, import_id: i64) -> i64 {
        connection
            .execute(
                "INSERT INTO daily_usage (
                    source_id, source_key, identity_version, usage_date,
                    aggregation_timezone, total_tokens, cost_kind, cost_status,
                    data_quality, record_state, absence_count, first_seen_at_ms,
                    last_seen_at_ms, latest_import_id
                ) VALUES (1, 'daily-parent', 1, '2026-06-14', 'UTC', 1,
                    'unknown', 'unavailable', 'complete', 'active', 0, 0, 0, ?1)",
                [import_id],
            )
            .expect("insert daily usage");
        connection.last_insert_rowid()
    }

    fn schema_version(database: &super::super::Database) -> i64 {
        database
            .connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .expect("query schema version")
    }

    fn table_count(database: &super::super::Database) -> i64 {
        database
            .connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_schema
                 WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
                [],
                |row| row.get(0),
            )
            .expect("count tables")
    }

    fn all_product_tables_are_strict(database: &super::super::Database) -> bool {
        let mut statement = database
            .connection
            .prepare("PRAGMA table_list")
            .expect("prepare table list");
        let strict_values = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(1)?, row.get::<_, i64>(5)?))
            })
            .expect("query table list")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect table list");

        strict_values
            .into_iter()
            .filter(|(name, _)| !name.starts_with("sqlite_"))
            .all(|(_, strict)| strict == 1)
    }

    fn assert_foreign_keys_clean(database: &super::super::Database) {
        let violations: i64 = database
            .connection
            .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
                row.get(0)
            })
            .expect("check foreign keys");
        assert_eq!(violations, 0);
    }

    fn assert_integrity_ok(database: &super::super::Database) {
        let result: String = database
            .connection
            .pragma_query_value(None, "integrity_check", |row| row.get(0))
            .expect("check integrity");
        assert_eq!(result, "ok");
    }

    fn table_exists(connection: &Connection, table: &str) -> bool {
        connection
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = ?1
                )",
                [table],
                |row| row.get(0),
            )
            .expect("query table existence")
    }

    #[test]
    fn migration_library_classifies_database_too_far_ahead() {
        let migrations = Migrations::new(vec![M::up("CREATE TABLE one (id INTEGER) STRICT;")]);
        let mut connection = Connection::open_in_memory().expect("open database");
        connection
            .pragma_update(None, "user_version", 2)
            .expect("set version");

        let error = migrations
            .to_latest(&mut connection)
            .expect_err("newer database must fail");

        assert!(matches!(
            error,
            MigrationError::MigrationDefinition(MigrationDefinitionError::DatabaseTooFarAhead)
        ));
    }
}
