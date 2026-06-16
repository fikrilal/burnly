//! SQLite overview read adapter.

use std::sync::Mutex;

use rusqlite::{params, Connection, OptionalExtension};

use crate::application::ports::overview_store::{OverviewStore, OverviewStoreError};
use crate::application::usage::{
    CostCompleteness, CostValuation, OverviewCost, OverviewModel, OverviewPeriod, OverviewSource,
    OverviewStoreResult, PersistedRefreshStatus,
};
use crate::domain::source::SourceKey;
use crate::domain::usage::CurrencyCode;

use super::Database;

pub(crate) struct SqliteOverviewStore {
    database: Mutex<Database>,
}

impl SqliteOverviewStore {
    pub(crate) fn new(database: Database) -> Self {
        Self {
            database: Mutex::new(database),
        }
    }
}

impl OverviewStore for SqliteOverviewStore {
    fn read_overview(
        &self,
        period: &OverviewPeriod,
    ) -> Result<OverviewStoreResult, OverviewStoreError> {
        let database = self
            .database
            .lock()
            .map_err(|_| OverviewStoreError::Backend)?;
        read_overview(database.connection(), period)
    }
}

fn read_overview(
    connection: &Connection,
    period: &OverviewPeriod,
) -> Result<OverviewStoreResult, OverviewStoreError> {
    let sources = read_sources(connection, period)?;
    let models = read_models(connection, period)?;
    let total_tokens = sources.iter().try_fold(0_u64, |total, source| {
        total
            .checked_add(source.total_tokens)
            .ok_or(OverviewStoreError::ValueOutOfRange)
    })?;
    let active_days = read_active_days(connection, period)?;
    let cost = combine_costs(&sources)?;
    let has_partial_data = sources.iter().any(|source| source.has_partial_data);
    let (latest_refresh_status, last_successful_refresh_at_ms) = read_refresh_history(connection)?;

    Ok(OverviewStoreResult {
        total_tokens,
        active_days,
        cost,
        sources,
        models,
        has_partial_data,
        latest_refresh_status,
        last_successful_refresh_at_ms,
    })
}

fn read_sources(
    connection: &Connection,
    period: &OverviewPeriod,
) -> Result<Vec<OverviewSource>, OverviewStoreError> {
    let mut statement = connection
        .prepare(
            "SELECT
                sources.source_key,
                COALESCE(SUM(daily_usage.total_tokens), 0),
                COUNT(DISTINCT daily_usage.usage_date),
                SUM(CASE
                    WHEN daily_usage.cost_status IN ('available', 'estimated')
                    THEN daily_usage.cost_amount_micros
                    ELSE 0
                END),
                SUM(CASE
                    WHEN daily_usage.cost_status IN ('available', 'estimated')
                    THEN 1 ELSE 0
                END),
                SUM(CASE
                    WHEN daily_usage.cost_status = 'estimated'
                    THEN 1 ELSE 0
                END),
                SUM(CASE
                    WHEN daily_usage.cost_status = 'unavailable'
                    THEN 1 ELSE 0
                END),
                MIN(CASE
                    WHEN daily_usage.cost_status IN ('available', 'estimated')
                    THEN daily_usage.cost_currency
                END),
                MAX(CASE
                    WHEN daily_usage.cost_status IN ('available', 'estimated')
                    THEN daily_usage.cost_currency
                END),
                MAX(CASE WHEN daily_usage.data_quality <> 'complete' THEN 1 ELSE 0 END)
            FROM daily_usage
            INNER JOIN sources ON sources.id = daily_usage.source_id
            WHERE daily_usage.usage_date BETWEEN ?1 AND ?2
                AND daily_usage.aggregation_timezone = ?3
                AND daily_usage.record_state <> 'removed'
            GROUP BY sources.id, sources.source_key
            ORDER BY sources.source_key",
        )
        .map_err(|_| OverviewStoreError::Backend)?;

    let rows = statement
        .query_map(
            params![
                period.start_date().to_string(),
                period.end_date().to_string(),
                period.aggregation_timezone(),
            ],
            |row| {
                Ok(SourceRow {
                    source_key: row.get(0)?,
                    total_tokens: row.get(1)?,
                    active_days: row.get(2)?,
                    cost_amount_micros: row.get(3)?,
                    valued_days: row.get(4)?,
                    estimated_days: row.get(5)?,
                    unavailable_days: row.get(6)?,
                    minimum_currency: row.get(7)?,
                    maximum_currency: row.get(8)?,
                    has_partial_data: row.get::<_, i64>(9)? != 0,
                })
            },
        )
        .map_err(|_| OverviewStoreError::Backend)?;

    rows.map(|row| {
        row.map_err(|_| OverviewStoreError::Backend)
            .and_then(source_from_row)
    })
    .collect()
}

struct SourceRow {
    source_key: String,
    total_tokens: i64,
    active_days: i64,
    cost_amount_micros: i64,
    valued_days: i64,
    estimated_days: i64,
    unavailable_days: i64,
    minimum_currency: Option<String>,
    maximum_currency: Option<String>,
    has_partial_data: bool,
}

fn source_from_row(row: SourceRow) -> Result<OverviewSource, OverviewStoreError> {
    let source = SourceKey::from_storage(&row.source_key).ok_or(OverviewStoreError::Backend)?;
    let total_tokens =
        u64::try_from(row.total_tokens).map_err(|_| OverviewStoreError::ValueOutOfRange)?;
    let active_days =
        u32::try_from(row.active_days).map_err(|_| OverviewStoreError::ValueOutOfRange)?;
    let cost = cost_from_values(
        row.cost_amount_micros,
        row.valued_days,
        row.estimated_days,
        row.unavailable_days,
        row.minimum_currency,
        row.maximum_currency,
    )?;

    Ok(OverviewSource {
        source,
        total_tokens,
        active_days,
        cost,
        has_partial_data: row.has_partial_data,
    })
}

fn read_models(
    connection: &Connection,
    period: &OverviewPeriod,
) -> Result<Vec<OverviewModel>, OverviewStoreError> {
    let mut statement = connection
        .prepare(
            "SELECT
                COALESCE(sm.display_name, sm.raw_model_id, 'Unknown') AS model_name,
                COALESCE(SUM(dmu.total_tokens), 0),
                SUM(CASE
                    WHEN dmu.cost_status = 'estimated'
                    THEN dmu.cost_amount_micros
                    ELSE 0
                END),
                SUM(CASE
                    WHEN dmu.cost_status = 'estimated'
                    THEN 1 ELSE 0
                END),
                SUM(CASE
                    WHEN dmu.cost_status = 'estimated'
                    THEN 1 ELSE 0
                END),
                SUM(CASE
                    WHEN dmu.cost_status = 'unavailable'
                    THEN 1 ELSE 0
                END),
                MIN(CASE
                    WHEN dmu.cost_status = 'estimated'
                    THEN dmu.cost_currency
                END),
                MAX(CASE
                    WHEN dmu.cost_status = 'estimated'
                    THEN dmu.cost_currency
                END)
            FROM daily_model_usage dmu
            INNER JOIN daily_usage du ON du.id = dmu.daily_usage_id
            LEFT JOIN source_models sm ON sm.id = dmu.model_id
            WHERE du.usage_date BETWEEN ?1 AND ?2
                AND du.aggregation_timezone = ?3
                AND du.record_state <> 'removed'
            GROUP BY model_name
            ORDER BY SUM(dmu.total_tokens) DESC, model_name ASC",
        )
        .map_err(|_| OverviewStoreError::Backend)?;

    let rows = statement
        .query_map(
            params![
                period.start_date().to_string(),
                period.end_date().to_string(),
                period.aggregation_timezone(),
            ],
            |row| {
                Ok(ModelRow {
                    name: row.get(0)?,
                    total_tokens: row.get(1)?,
                    cost_amount_micros: row.get(2)?,
                    valued_days: row.get(3)?,
                    estimated_days: row.get(4)?,
                    unavailable_days: row.get(5)?,
                    minimum_currency: row.get(6)?,
                    maximum_currency: row.get(7)?,
                })
            },
        )
        .map_err(|_| OverviewStoreError::Backend)?;

    rows.map(|row| {
        row.map_err(|_| OverviewStoreError::Backend)
            .and_then(model_from_row)
    })
    .collect()
}

struct ModelRow {
    name: String,
    total_tokens: i64,
    cost_amount_micros: i64,
    valued_days: i64,
    estimated_days: i64,
    unavailable_days: i64,
    minimum_currency: Option<String>,
    maximum_currency: Option<String>,
}

fn model_from_row(row: ModelRow) -> Result<OverviewModel, OverviewStoreError> {
    let total_tokens =
        u64::try_from(row.total_tokens).map_err(|_| OverviewStoreError::ValueOutOfRange)?;
    let cost = cost_from_values(
        row.cost_amount_micros,
        row.valued_days,
        row.estimated_days,
        row.unavailable_days,
        row.minimum_currency,
        row.maximum_currency,
    )?;

    Ok(OverviewModel {
        name: row.name,
        total_tokens,
        cost,
    })
}

fn cost_from_values(
    amount_micros: i64,
    valued_days: i64,
    estimated_days: i64,
    unavailable_days: i64,
    minimum_currency: Option<String>,
    maximum_currency: Option<String>,
) -> Result<OverviewCost, OverviewStoreError> {
    let unavailable_days =
        u32::try_from(unavailable_days).map_err(|_| OverviewStoreError::ValueOutOfRange)?;
    if valued_days == 0 {
        return Ok(OverviewCost {
            amount_micros: None,
            currency: None,
            valuation: CostValuation::Unavailable,
            completeness: CostCompleteness::Unavailable,
            unavailable_days,
        });
    }
    if minimum_currency != maximum_currency {
        return Err(OverviewStoreError::MixedCurrencies);
    }
    let currency = minimum_currency
        .and_then(|value| CurrencyCode::new(value).ok())
        .ok_or(OverviewStoreError::Backend)?;
    let amount_micros =
        u64::try_from(amount_micros).map_err(|_| OverviewStoreError::ValueOutOfRange)?;

    Ok(OverviewCost {
        amount_micros: Some(amount_micros),
        currency: Some(currency),
        valuation: if estimated_days > 0 {
            CostValuation::Estimated
        } else {
            CostValuation::Available
        },
        completeness: if unavailable_days == 0 {
            CostCompleteness::Complete
        } else {
            CostCompleteness::Partial
        },
        unavailable_days,
    })
}

fn combine_costs(sources: &[OverviewSource]) -> Result<OverviewCost, OverviewStoreError> {
    let mut total = 0_u64;
    let mut currency: Option<CurrencyCode> = None;
    let mut valued = false;
    let mut estimated = false;
    let mut unavailable_days = 0_u32;

    for source in sources {
        unavailable_days = unavailable_days
            .checked_add(source.cost.unavailable_days)
            .ok_or(OverviewStoreError::ValueOutOfRange)?;
        if let Some(amount) = source.cost.amount_micros {
            estimated |= source.cost.valuation == CostValuation::Estimated;
            let source_currency = source
                .cost
                .currency
                .as_ref()
                .ok_or(OverviewStoreError::Backend)?;
            if currency
                .as_ref()
                .is_some_and(|current| current != source_currency)
            {
                return Err(OverviewStoreError::MixedCurrencies);
            }
            currency = Some(source_currency.clone());
            total = total
                .checked_add(amount)
                .ok_or(OverviewStoreError::ValueOutOfRange)?;
            valued = true;
        }
    }

    Ok(OverviewCost {
        amount_micros: valued.then_some(total),
        currency,
        valuation: if !valued {
            CostValuation::Unavailable
        } else if estimated {
            CostValuation::Estimated
        } else {
            CostValuation::Available
        },
        completeness: if !valued {
            CostCompleteness::Unavailable
        } else if unavailable_days > 0 {
            CostCompleteness::Partial
        } else {
            CostCompleteness::Complete
        },
        unavailable_days,
    })
}

fn read_active_days(
    connection: &Connection,
    period: &OverviewPeriod,
) -> Result<u32, OverviewStoreError> {
    let value: i64 = connection
        .query_row(
            "SELECT COUNT(DISTINCT usage_date)
            FROM daily_usage
            WHERE usage_date BETWEEN ?1 AND ?2
                AND aggregation_timezone = ?3
                AND record_state <> 'removed'",
            params![
                period.start_date().to_string(),
                period.end_date().to_string(),
                period.aggregation_timezone(),
            ],
            |row| row.get(0),
        )
        .map_err(|_| OverviewStoreError::Backend)?;
    u32::try_from(value).map_err(|_| OverviewStoreError::ValueOutOfRange)
}

fn read_refresh_history(
    connection: &Connection,
) -> Result<(Option<PersistedRefreshStatus>, Option<i64>), OverviewStoreError> {
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
        .map_err(|_| OverviewStoreError::Backend)?
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
        .map_err(|_| OverviewStoreError::Backend)?;

    Ok((latest_status, last_successful_refresh_at_ms))
}

fn refresh_status(value: &str) -> Result<PersistedRefreshStatus, OverviewStoreError> {
    match value {
        "succeeded" => Ok(PersistedRefreshStatus::Succeeded),
        "partial" => Ok(PersistedRefreshStatus::Partial),
        "failed" => Ok(PersistedRefreshStatus::Failed),
        "cancelled" => Ok(PersistedRefreshStatus::Cancelled),
        _ => Err(OverviewStoreError::Backend),
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::*;

    fn migrated_store() -> (tempfile::TempDir, SqliteOverviewStore) {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("burnly.sqlite3");
        let mut database = Database::open(&path).expect("open database");
        database.migrate_to_latest().expect("migrate database");
        seed_settings(database.connection());
        (directory, SqliteOverviewStore::new(database))
    }

    #[test]
    fn aggregates_model_usage_correctly() {
        let (_directory, store) = migrated_store();
        {
            let guard = store.connection();
            let conn = guard.connection();
            seed_source(conn, 1, "claude-code");
            seed_model(conn, 10, 1, "claude-3-5-sonnet", Some("Claude 3.5 Sonnet"));
            seed_model(conn, 11, 1, "claude-3-haiku", None);

            let refresh_id = seed_refresh(conn, "succeeded", 200);
            let import_id = seed_import(conn, refresh_id, 1);

            let daily_1 = seed_daily(
                conn,
                DailySeed::new(1, import_id, "claude-13", "2026-06-13", 100),
            );
            let daily_2 = seed_daily(
                conn,
                DailySeed::new(1, import_id, "claude-14", "2026-06-14", 200),
            );

            seed_daily_model_usage(
                conn,
                daily_1,
                1,
                10,
                80,
                Some(20),
                Some("USD"),
                "estimated",
                import_id,
            );
            seed_daily_model_usage(
                conn,
                daily_2,
                1,
                10,
                120,
                Some(30),
                Some("USD"),
                "estimated",
                import_id,
            );
            seed_daily_model_usage(
                conn,
                daily_2,
                1,
                11,
                50,
                None,
                None,
                "unavailable",
                import_id,
            );
        }

        let overview = store.read_overview(&period()).expect("overview");

        assert_eq!(overview.models.len(), 2);

        // Ordered by SUM(total_tokens) DESC
        assert_eq!(overview.models[0].name, "Claude 3.5 Sonnet");
        assert_eq!(overview.models[0].total_tokens, 200);
        assert_eq!(overview.models[0].cost.amount_micros, Some(50));
        assert_eq!(
            overview.models[0]
                .cost
                .currency
                .as_ref()
                .map(CurrencyCode::as_str),
            Some("USD")
        );
        assert_eq!(overview.models[0].cost.valuation, CostValuation::Estimated);

        assert_eq!(overview.models[1].name, "claude-3-haiku"); // uses raw_model_id when display_name is None
        assert_eq!(overview.models[1].total_tokens, 50);
        assert_eq!(overview.models[1].cost.amount_micros, None);
        assert_eq!(
            overview.models[1].cost.valuation,
            CostValuation::Unavailable
        );
    }

    #[test]
    fn aggregates_authoritative_daily_facts_and_cost_completeness() {
        let (_directory, store) = migrated_store();
        seed_source(store.connection().connection(), 1, "claude-code");
        seed_source(store.connection().connection(), 2, "codex");
        let refresh_id = seed_refresh(store.connection().connection(), "succeeded", 200);
        let claude_import = seed_import(store.connection().connection(), refresh_id, 1);
        let codex_import = seed_import(store.connection().connection(), refresh_id, 2);
        seed_daily(
            store.connection().connection(),
            DailySeed::new(1, claude_import, "claude-13", "2026-06-13", 100).cost(
                40,
                "USD",
                "estimated",
            ),
        );
        seed_daily(
            store.connection().connection(),
            DailySeed::new(1, claude_import, "claude-14", "2026-06-14", 200).quality("partial"),
        );
        seed_daily(
            store.connection().connection(),
            DailySeed::new(2, codex_import, "codex-14", "2026-06-14", 50).cost(
                10,
                "USD",
                "available",
            ),
        );

        let overview = store.read_overview(&period()).expect("overview");

        assert_eq!(overview.total_tokens, 350);
        assert_eq!(overview.active_days, 2);
        assert_eq!(overview.cost.amount_micros, Some(50));
        assert_eq!(
            overview.cost.currency.as_ref().map(CurrencyCode::as_str),
            Some("USD")
        );
        assert_eq!(overview.cost.completeness, CostCompleteness::Partial);
        assert_eq!(overview.cost.valuation, CostValuation::Estimated);
        assert_eq!(overview.cost.unavailable_days, 1);
        assert!(overview.has_partial_data);
        assert_eq!(
            overview.latest_refresh_status,
            Some(PersistedRefreshStatus::Succeeded)
        );
        assert_eq!(overview.last_successful_refresh_at_ms, Some(200));
        assert_eq!(overview.sources.len(), 2);
        assert_eq!(overview.sources[0].source, SourceKey::ClaudeCode);
        assert_eq!(overview.sources[0].total_tokens, 300);
        assert_eq!(overview.sources[0].cost.valuation, CostValuation::Estimated);
        assert_eq!(overview.sources[1].source, SourceKey::Codex);
        assert_eq!(overview.sources[1].total_tokens, 50);
        assert_eq!(overview.sources[1].cost.valuation, CostValuation::Available);
    }

    #[test]
    fn includes_missing_rows_excludes_removed_rows_and_respects_period_timezone() {
        let (_directory, store) = migrated_store();
        seed_source(store.connection().connection(), 1, "claude-code");
        let refresh_id = seed_refresh(store.connection().connection(), "succeeded", 200);
        let import_id = seed_import(store.connection().connection(), refresh_id, 1);
        seed_daily(
            store.connection().connection(),
            DailySeed::new(1, import_id, "active", "2026-06-13", 100),
        );
        seed_daily(
            store.connection().connection(),
            DailySeed::new(1, import_id, "missing", "2026-06-14", 200).state("missing", 1, None),
        );
        seed_daily(
            store.connection().connection(),
            DailySeed::new(1, import_id, "removed", "2026-06-15", 400).state(
                "removed",
                2,
                Some(300),
            ),
        );
        seed_daily(
            store.connection().connection(),
            DailySeed::new(1, import_id, "other-timezone", "2026-06-14", 800)
                .timezone("Asia/Jakarta"),
        );
        seed_daily(
            store.connection().connection(),
            DailySeed::new(1, import_id, "outside", "2026-05-31", 1_600),
        );

        let overview = store.read_overview(&period()).expect("overview");

        assert_eq!(overview.total_tokens, 300);
        assert_eq!(overview.active_days, 2);
    }

    #[test]
    fn empty_database_returns_a_valid_empty_result() {
        let (_directory, store) = migrated_store();

        let overview = store.read_overview(&period()).expect("empty overview");

        assert_eq!(overview.total_tokens, 0);
        assert_eq!(overview.active_days, 0);
        assert!(overview.sources.is_empty());
        assert_eq!(overview.cost.completeness, CostCompleteness::Unavailable);
        assert_eq!(overview.cost.amount_micros, None);
        assert_eq!(overview.latest_refresh_status, None);
    }

    #[test]
    fn rejects_mixed_cost_currencies() {
        let (_directory, store) = migrated_store();
        seed_source(store.connection().connection(), 1, "claude-code");
        let refresh_id = seed_refresh(store.connection().connection(), "succeeded", 200);
        let import_id = seed_import(store.connection().connection(), refresh_id, 1);
        seed_daily(
            store.connection().connection(),
            DailySeed::new(1, import_id, "usd", "2026-06-13", 100).cost(40, "USD", "estimated"),
        );
        seed_daily(
            store.connection().connection(),
            DailySeed::new(1, import_id, "eur", "2026-06-14", 100).cost(30, "EUR", "estimated"),
        );

        assert_eq!(
            store.read_overview(&period()),
            Err(OverviewStoreError::MixedCurrencies)
        );
    }

    #[test]
    fn overview_remains_queryable_after_database_reopen() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("burnly.sqlite3");
        {
            let mut database = Database::open(&path).expect("open database");
            database.migrate_to_latest().expect("migrate database");
            seed_settings(database.connection());
            seed_source(database.connection(), 1, "claude-code");
            let refresh_id = seed_refresh(database.connection(), "partial", 200);
            let import_id = seed_import(database.connection(), refresh_id, 1);
            seed_daily(
                database.connection(),
                DailySeed::new(1, import_id, "daily", "2026-06-14", 100),
            );
        }
        let store = SqliteOverviewStore::new(Database::open(&path).expect("reopen database"));

        let overview = store.read_overview(&period()).expect("reopened overview");

        assert_eq!(overview.total_tokens, 100);
        assert_eq!(
            overview.latest_refresh_status,
            Some(PersistedRefreshStatus::Partial)
        );
    }

    #[test]
    fn invalid_numeric_values_are_rejected_by_conversion() {
        assert_eq!(
            cost_from_values(-1, 1, 0, 0, Some("USD".to_owned()), Some("USD".to_owned()),),
            Err(OverviewStoreError::ValueOutOfRange)
        );
    }

    impl SqliteOverviewStore {
        fn connection(&self) -> std::sync::MutexGuard<'_, Database> {
            self.database.lock().expect("database lock")
        }
    }

    fn period() -> OverviewPeriod {
        OverviewPeriod::new(
            NaiveDate::from_ymd_opt(2026, 6, 1).expect("start"),
            NaiveDate::from_ymd_opt(2026, 6, 15).expect("end"),
            "UTC",
        )
        .expect("period")
    }

    fn seed_settings(connection: &Connection) {
        connection
            .execute(
                "INSERT INTO app_settings (
                    id, reporting_timezone, background_refresh_enabled,
                    refresh_interval_minutes, launch_at_login, close_behavior,
                    notifications_enabled, store_project_paths,
                    created_at_ms, updated_at_ms
                ) VALUES (1, 'UTC', 0, 15, 0, 'quit', 0, 0, 0, 0)",
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
                    status, records_seen, records_rejected, started_at_ms,
                    finished_at_ms
                ) VALUES (?1, ?2, 'ccusage', '20.0.11', 1, 'daily', 'full',
                    'UTC', 'succeeded', 1, 0, 100, 200)",
                params![refresh_id, source_id],
            )
            .expect("seed import");
        connection.last_insert_rowid()
    }

    struct DailySeed<'a> {
        source_id: i64,
        import_id: i64,
        source_key: &'a str,
        usage_date: &'a str,
        timezone: &'a str,
        total_tokens: i64,
        amount_micros: Option<i64>,
        currency: Option<&'a str>,
        cost_status: &'a str,
        quality: &'a str,
        state: &'a str,
        absence_count: i64,
        removed_at_ms: Option<i64>,
    }

    impl<'a> DailySeed<'a> {
        fn new(
            source_id: i64,
            import_id: i64,
            source_key: &'a str,
            usage_date: &'a str,
            total_tokens: i64,
        ) -> Self {
            Self {
                source_id,
                import_id,
                source_key,
                usage_date,
                timezone: "UTC",
                total_tokens,
                amount_micros: None,
                currency: None,
                cost_status: "unavailable",
                quality: "complete",
                state: "active",
                absence_count: 0,
                removed_at_ms: None,
            }
        }

        fn cost(mut self, amount_micros: i64, currency: &'a str, status: &'a str) -> Self {
            self.amount_micros = Some(amount_micros);
            self.currency = Some(currency);
            self.cost_status = status;
            self
        }

        fn quality(mut self, quality: &'a str) -> Self {
            self.quality = quality;
            self
        }

        fn timezone(mut self, timezone: &'a str) -> Self {
            self.timezone = timezone;
            self
        }

        fn state(mut self, state: &'a str, absence_count: i64, removed_at_ms: Option<i64>) -> Self {
            self.state = state;
            self.absence_count = absence_count;
            self.removed_at_ms = removed_at_ms;
            self
        }
    }

    fn seed_daily(connection: &Connection, seed: DailySeed<'_>) -> i64 {
        connection
            .execute(
                "INSERT INTO daily_usage (
                    source_id, source_key, identity_version, usage_date,
                    aggregation_timezone, total_tokens, cost_amount_micros,
                    cost_currency, cost_kind, cost_status, data_quality,
                    record_state, absence_count, first_seen_at_ms, last_seen_at_ms,
                    removed_at_ms, latest_import_id
                ) VALUES (?1, ?2, 1, ?3, ?4, ?5, ?6, ?7,
                    'collector_calculated', ?8, ?9, ?10, ?11, 100, 200, ?12, ?13)",
                params![
                    seed.source_id,
                    seed.source_key,
                    seed.usage_date,
                    seed.timezone,
                    seed.total_tokens,
                    seed.amount_micros,
                    seed.currency,
                    seed.cost_status,
                    seed.quality,
                    seed.state,
                    seed.absence_count,
                    seed.removed_at_ms,
                    seed.import_id,
                ],
            )
            .expect("seed daily usage");
        connection.last_insert_rowid()
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
                    id, source_id, raw_model_id, display_name, provider_key,
                    first_seen_at_ms, last_seen_at_ms
                ) VALUES (?1, ?2, ?3, ?4, 'openai', 0, 0)",
                params![id, source_id, raw_model_id, display_name],
            )
            .expect("seed model");
    }

    #[allow(clippy::too_many_arguments)]
    fn seed_daily_model_usage(
        connection: &Connection,
        daily_usage_id: i64,
        source_id: i64,
        model_id: i64,
        tokens: i64,
        cost_amount: Option<i64>,
        cost_currency: Option<&str>,
        cost_status: &str,
        import_id: i64,
    ) {
        connection
            .execute(
                "INSERT INTO daily_model_usage (
                    daily_usage_id, source_id, model_id, total_tokens, cost_amount_micros,
                    cost_currency, cost_status, latest_import_id
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    daily_usage_id,
                    source_id,
                    model_id,
                    tokens,
                    cost_amount,
                    cost_currency,
                    cost_status,
                    import_id
                ],
            )
            .expect("seed daily model usage");
    }
}
