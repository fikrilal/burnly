//! SQLite compact tray summary read adapter.

use std::sync::Mutex;

use rusqlite::{params, Connection, OptionalExtension};

use crate::application::ports::tray_summary_store::{TraySummaryStore, TraySummaryStoreError};
use crate::application::usage::{
    PersistedRefreshStatus, TraySummaryScope, TraySummaryStoreModelUsage, TraySummaryStoreResult,
};
use crate::domain::source::SourceKey;

use super::Database;

pub(crate) struct SqliteTraySummaryStore {
    database: Mutex<Database>,
}

impl SqliteTraySummaryStore {
    pub(crate) fn new(database: Database) -> Self {
        Self {
            database: Mutex::new(database),
        }
    }
}

impl TraySummaryStore for SqliteTraySummaryStore {
    fn read_tray_summary(
        &self,
        scope: &TraySummaryScope,
    ) -> Result<TraySummaryStoreResult, TraySummaryStoreError> {
        let database = self
            .database
            .lock()
            .map_err(|_| TraySummaryStoreError::Backend)?;
        read_tray_summary(database.connection(), scope)
    }
}

fn read_tray_summary(
    connection: &Connection,
    scope: &TraySummaryScope,
) -> Result<TraySummaryStoreResult, TraySummaryStoreError> {
    let today_total_tokens = read_period_total(
        connection,
        scope.today(),
        scope.today(),
        scope.aggregation_timezone(),
    )?;
    let week_total_tokens = read_period_total(
        connection,
        scope.week_start(),
        scope.week_end(),
        scope.aggregation_timezone(),
    )?;
    let month_total_tokens = read_period_total(
        connection,
        scope.month_start(),
        scope.month_end(),
        scope.aggregation_timezone(),
    )?;
    let today_models = read_model_usage(connection, scope.today(), scope.aggregation_timezone())?;
    let yesterday_models =
        read_model_usage(connection, scope.yesterday(), scope.aggregation_timezone())?;
    let has_partial_data =
        read_has_partial_today(connection, scope.today(), scope.aggregation_timezone())?;
    let (latest_refresh_status, last_successful_refresh_at_ms) = read_refresh_history(connection)?;

    Ok(TraySummaryStoreResult {
        today_total_tokens,
        week_total_tokens,
        month_total_tokens,
        today_models,
        yesterday_models,
        has_partial_data,
        latest_refresh_status,
        last_successful_refresh_at_ms,
    })
}

fn read_period_total(
    connection: &Connection,
    start_date: chrono::NaiveDate,
    end_date: chrono::NaiveDate,
    aggregation_timezone: &str,
) -> Result<u64, TraySummaryStoreError> {
    let value: i64 = connection
        .query_row(
            "SELECT COALESCE(SUM(total_tokens), 0)
            FROM daily_usage
            WHERE usage_date BETWEEN ?1 AND ?2
                AND aggregation_timezone = ?3
                AND record_state <> 'removed'",
            params![
                start_date.to_string(),
                end_date.to_string(),
                aggregation_timezone,
            ],
            |row| row.get(0),
        )
        .map_err(|_| TraySummaryStoreError::Backend)?;
    u64::try_from(value).map_err(|_| TraySummaryStoreError::ValueOutOfRange)
}

fn read_model_usage(
    connection: &Connection,
    usage_date: chrono::NaiveDate,
    aggregation_timezone: &str,
) -> Result<Vec<TraySummaryStoreModelUsage>, TraySummaryStoreError> {
    let mut statement = connection
        .prepare(
            "SELECT
                COALESCE(sm.display_name, sm.raw_model_id, 'Unknown') AS model_name,
                GROUP_CONCAT(DISTINCT sources.source_key),
                COALESCE(SUM(dmu.total_tokens), 0)
            FROM daily_model_usage dmu
            INNER JOIN daily_usage du ON du.id = dmu.daily_usage_id
            INNER JOIN sources ON sources.id = du.source_id
            LEFT JOIN source_models sm ON sm.id = dmu.model_id
            WHERE du.usage_date = ?1
                AND du.aggregation_timezone = ?2
                AND du.record_state <> 'removed'
            GROUP BY model_name
            ORDER BY SUM(dmu.total_tokens) DESC, model_name ASC",
        )
        .map_err(|_| TraySummaryStoreError::Backend)?;

    let rows = statement
        .query_map(
            params![usage_date.to_string(), aggregation_timezone],
            |row| {
                Ok(ModelUsageRow {
                    model_name: row.get(0)?,
                    source_keys: row.get(1)?,
                    total_tokens: row.get(2)?,
                })
            },
        )
        .map_err(|_| TraySummaryStoreError::Backend)?;

    rows.map(|row| {
        row.map_err(|_| TraySummaryStoreError::Backend)
            .and_then(model_usage_from_row)
    })
    .collect()
}

struct ModelUsageRow {
    model_name: String,
    source_keys: Option<String>,
    total_tokens: i64,
}

fn model_usage_from_row(
    row: ModelUsageRow,
) -> Result<TraySummaryStoreModelUsage, TraySummaryStoreError> {
    let total_tokens =
        u64::try_from(row.total_tokens).map_err(|_| TraySummaryStoreError::ValueOutOfRange)?;
    let source_keys = row
        .source_keys
        .unwrap_or_default()
        .split(',')
        .filter(|value| !value.trim().is_empty())
        .map(|value| SourceKey::from_storage(value).ok_or(TraySummaryStoreError::Backend))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(TraySummaryStoreModelUsage {
        model_name: row.model_name,
        source_keys,
        total_tokens,
    })
}

fn read_has_partial_today(
    connection: &Connection,
    usage_date: chrono::NaiveDate,
    aggregation_timezone: &str,
) -> Result<bool, TraySummaryStoreError> {
    let value: i64 = connection
        .query_row(
            "SELECT COALESCE(MAX(CASE
                WHEN data_quality <> 'complete' OR record_state = 'missing'
                THEN 1 ELSE 0
            END), 0)
            FROM daily_usage
            WHERE usage_date = ?1
                AND aggregation_timezone = ?2
                AND record_state <> 'removed'",
            params![usage_date.to_string(), aggregation_timezone],
            |row| row.get(0),
        )
        .map_err(|_| TraySummaryStoreError::Backend)?;
    Ok(value != 0)
}

fn read_refresh_history(
    connection: &Connection,
) -> Result<(Option<PersistedRefreshStatus>, Option<i64>), TraySummaryStoreError> {
    let latest_status = connection
        .query_row(
            "SELECT status
            FROM refresh_runs
            WHERE status IN ('succeeded', 'partial', 'failed', 'cancelled')
            ORDER BY id DESC
            LIMIT 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|_| TraySummaryStoreError::Backend)?
        .map(|status| refresh_status(&status))
        .transpose()?;
    let last_successful_refresh_at_ms = connection
        .query_row(
            "SELECT MAX(finished_at_ms)
            FROM refresh_runs
            WHERE status = 'succeeded'",
            [],
            |row| row.get::<_, Option<i64>>(0),
        )
        .map_err(|_| TraySummaryStoreError::Backend)?;

    Ok((latest_status, last_successful_refresh_at_ms))
}

fn refresh_status(value: &str) -> Result<PersistedRefreshStatus, TraySummaryStoreError> {
    match value {
        "succeeded" => Ok(PersistedRefreshStatus::Succeeded),
        "partial" => Ok(PersistedRefreshStatus::Partial),
        "failed" => Ok(PersistedRefreshStatus::Failed),
        "cancelled" => Ok(PersistedRefreshStatus::Cancelled),
        _ => Err(TraySummaryStoreError::Backend),
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use rusqlite::params;

    use super::*;

    fn migrated_store() -> (tempfile::TempDir, SqliteTraySummaryStore) {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("burnly.sqlite3");
        let mut database = Database::open(&path).expect("open database");
        database.migrate_to_latest().expect("migrate database");
        seed_settings(database.connection());
        (directory, SqliteTraySummaryStore::new(database))
    }

    #[test]
    fn reads_period_totals_models_refresh_and_partial_state() {
        let (_directory, store) = migrated_store();
        {
            let guard = store.connection();
            let conn = guard.connection();
            seed_source(conn, 1, "codex");
            seed_source(conn, 2, "opencode");
            seed_model(conn, 10, 1, "gpt-5.1", Some("GPT-5.1"));
            seed_model(conn, 11, 1, "gpt-5", None);
            seed_model(conn, 20, 2, "gpt-5.1", Some("GPT-5.1"));
            seed_model(conn, 21, 2, "mimo", None);

            let refresh_id = seed_refresh(conn, "partial", 1_500);
            let codex_import = seed_import(conn, refresh_id, 1);
            let opencode_import = seed_import(conn, refresh_id, 2);

            let yesterday_codex = seed_daily(
                conn,
                DailySeed::new(1, codex_import, "codex-yesterday", "2026-06-24", 400),
            );
            seed_daily_model_usage(conn, yesterday_codex, 1, 10, 300, codex_import);
            seed_daily_model_usage(conn, yesterday_codex, 1, 11, 100, codex_import);

            let today_codex = seed_daily(
                conn,
                DailySeed::new(1, codex_import, "codex-today", "2026-06-25", 800)
                    .quality("partial"),
            );
            let today_opencode = seed_daily(
                conn,
                DailySeed::new(2, opencode_import, "opencode-today", "2026-06-25", 200),
            );
            seed_daily_model_usage(conn, today_codex, 1, 10, 500, codex_import);
            seed_daily_model_usage(conn, today_codex, 1, 11, 300, codex_import);
            seed_daily_model_usage(conn, today_opencode, 2, 20, 150, opencode_import);
            seed_daily_model_usage(conn, today_opencode, 2, 21, 50, opencode_import);

            seed_daily(
                conn,
                DailySeed::new(1, codex_import, "week", "2026-06-23", 50),
            );
            seed_daily(
                conn,
                DailySeed::new(1, codex_import, "month", "2026-06-01", 25),
            );
            seed_daily(
                conn,
                DailySeed::new(1, codex_import, "other-timezone", "2026-06-25", 9_999)
                    .timezone("UTC"),
            );
            seed_daily(
                conn,
                DailySeed::new(1, codex_import, "removed", "2026-06-25", 9_999).state("removed"),
            );
        }

        let summary = store.read_tray_summary(&scope()).expect("summary");

        assert_eq!(summary.today_total_tokens, 1_000);
        assert_eq!(summary.week_total_tokens, 1_450);
        assert_eq!(summary.month_total_tokens, 1_475);
        assert!(summary.has_partial_data);
        assert_eq!(
            summary.latest_refresh_status,
            Some(PersistedRefreshStatus::Partial)
        );
        assert_eq!(summary.last_successful_refresh_at_ms, None);
        assert_eq!(summary.today_models.len(), 3);
        assert_eq!(summary.today_models[0].model_name, "GPT-5.1");
        assert_eq!(summary.today_models[0].total_tokens, 650);
        assert_eq!(
            summary.today_models[0].source_keys,
            vec![SourceKey::Codex, SourceKey::OpenCode]
        );
        assert_eq!(summary.yesterday_models[0].model_name, "GPT-5.1");
        assert_eq!(summary.yesterday_models[0].total_tokens, 300);
    }

    #[test]
    fn empty_database_returns_zero_summary() {
        let (_directory, store) = migrated_store();

        let summary = store.read_tray_summary(&scope()).expect("summary");

        assert_eq!(summary.today_total_tokens, 0);
        assert_eq!(summary.week_total_tokens, 0);
        assert_eq!(summary.month_total_tokens, 0);
        assert!(summary.today_models.is_empty());
        assert!(summary.yesterday_models.is_empty());
        assert_eq!(summary.latest_refresh_status, None);
    }

    impl SqliteTraySummaryStore {
        fn connection(&self) -> std::sync::MutexGuard<'_, Database> {
            self.database.lock().expect("database lock")
        }
    }

    fn scope() -> TraySummaryScope {
        TraySummaryScope::new(date(2026, 6, 25), "Asia/Jakarta").expect("scope")
    }

    fn seed_settings(connection: &Connection) {
        connection
            .execute(
                "INSERT INTO app_settings (
                    id, reporting_timezone, background_refresh_enabled,
                    refresh_interval_minutes, launch_at_login, close_behavior,
                    notifications_enabled, store_project_paths,
                    created_at_ms, updated_at_ms
                ) VALUES (1, 'Asia/Jakarta', 0, 15, 0, 'quit', 0, 0, 0, 0)",
                [],
            )
            .expect("seed settings");
    }

    fn seed_source(connection: &Connection, id: i64, key: &str) {
        connection
            .execute(
                "INSERT INTO sources (
                    id, source_key, display_name, enabled, detection_state,
                    created_at_ms, updated_at_ms
                ) VALUES (?1, ?2, ?2, 1, 'available', 0, 0)",
                params![id, key],
            )
            .expect("seed source");
    }

    fn seed_model(
        connection: &Connection,
        id: i64,
        source_id: i64,
        raw_model_id: &str,
        display_name: Option<&str>,
    ) {
        connection
            .execute(
                "INSERT INTO source_models (
                    id, source_id, raw_model_id, display_name,
                    first_seen_at_ms, last_seen_at_ms
                ) VALUES (?1, ?2, ?3, ?4, 0, 0)",
                params![id, source_id, raw_model_id, display_name],
            )
            .expect("seed model");
    }

    fn seed_refresh(connection: &Connection, status: &str, finished_at_ms: i64) -> i64 {
        connection
            .execute(
                "INSERT INTO refresh_runs (
                    job_key, trigger, status, started_at_ms, finished_at_ms,
                    requested_by_app_version, created_at_ms
                ) VALUES (?1, 'manual', ?2, 100, ?3, '0.1.0', 100)",
                params![
                    format!("job-{status}-{finished_at_ms}"),
                    status,
                    finished_at_ms
                ],
            )
            .expect("seed refresh");
        connection.last_insert_rowid()
    }

    fn seed_import(connection: &Connection, refresh_id: i64, source_id: i64) -> i64 {
        connection
            .execute(
                "INSERT INTO import_runs (
                    refresh_run_id, source_id, collector_key, collector_version,
                    profile_version, projection, scope_kind, aggregation_timezone,
                    status, records_seen, records_rejected, started_at_ms, finished_at_ms
                ) VALUES (?1, ?2, 'ccusage', '20.0.19', 1, 'daily', 'full',
                    'Asia/Jakarta', 'succeeded', 1, 0, 100, 200)",
                params![refresh_id, source_id],
            )
            .expect("seed import");
        connection.last_insert_rowid()
    }

    struct DailySeed<'a> {
        source_id: i64,
        latest_import_id: i64,
        source_key: &'a str,
        usage_date: &'a str,
        total_tokens: u64,
        timezone: &'a str,
        quality: &'a str,
        state: &'a str,
    }

    impl<'a> DailySeed<'a> {
        fn new(
            source_id: i64,
            latest_import_id: i64,
            source_key: &'a str,
            usage_date: &'a str,
            total_tokens: u64,
        ) -> Self {
            Self {
                source_id,
                latest_import_id,
                source_key,
                usage_date,
                total_tokens,
                timezone: "Asia/Jakarta",
                quality: "complete",
                state: "active",
            }
        }

        fn timezone(mut self, timezone: &'a str) -> Self {
            self.timezone = timezone;
            self
        }

        fn quality(mut self, quality: &'a str) -> Self {
            self.quality = quality;
            self
        }

        fn state(mut self, state: &'a str) -> Self {
            self.state = state;
            self
        }
    }

    fn seed_daily(connection: &Connection, seed: DailySeed<'_>) -> i64 {
        connection
            .execute(
                "INSERT INTO daily_usage (
                    source_id, source_key, identity_version, usage_date,
                    aggregation_timezone, total_tokens, cost_kind, cost_status,
                    data_quality, record_state, absence_count, first_seen_at_ms,
                    last_seen_at_ms, removed_at_ms, latest_import_id
                ) VALUES (?1, ?2, 1, ?3, ?4, ?5, 'collector_calculated',
                    'unavailable', ?6, ?7, ?8, 100, 200, ?9, ?10)",
                params![
                    seed.source_id,
                    seed.source_key,
                    seed.usage_date,
                    seed.timezone,
                    i64::try_from(seed.total_tokens).expect("tokens fit"),
                    seed.quality,
                    seed.state,
                    if seed.state == "active" { 0 } else { 2 },
                    if seed.state == "removed" {
                        Some(300_i64)
                    } else {
                        None
                    },
                    seed.latest_import_id,
                ],
            )
            .expect("seed daily");
        connection.last_insert_rowid()
    }

    fn seed_daily_model_usage(
        connection: &Connection,
        daily_usage_id: i64,
        source_id: i64,
        model_id: i64,
        total_tokens: u64,
        latest_import_id: i64,
    ) {
        connection
            .execute(
                "INSERT INTO daily_model_usage (
                    daily_usage_id, source_id, model_id, total_tokens,
                    cost_status, latest_import_id
                ) VALUES (?1, ?2, ?3, ?4, 'unavailable', ?5)",
                params![
                    daily_usage_id,
                    source_id,
                    model_id,
                    i64::try_from(total_tokens).expect("tokens fit"),
                    latest_import_id,
                ],
            )
            .expect("seed daily model usage");
    }

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
    }
}
