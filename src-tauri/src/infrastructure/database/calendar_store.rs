use crate::infrastructure::database::Database;
use chrono::NaiveDate;
use rusqlite::params;
use std::sync::Mutex;

use crate::application::ports::calendar_store::{CalendarStore, CalendarStoreError};
use crate::application::ports::day_detail_store::{DayDetailStore, DayDetailStoreError};
use crate::application::usage::{
    CalendarDayInfo, CalendarPeriod, CalendarReadModel, CostCompleteness, CostValuation,
    DayDetailModel, DayDetailPeriod, DayDetailReadModel, OverviewCost, OverviewDataStatus,
};
use crate::domain::source::SourceKey;
use crate::domain::usage::CurrencyCode;

pub(crate) struct SqliteCalendarStore {
    database: Mutex<Database>,
}

impl SqliteCalendarStore {
    pub(crate) fn new(database: Database) -> Self {
        Self {
            database: Mutex::new(database),
        }
    }
}

impl CalendarStore for SqliteCalendarStore {
    fn read_calendar(
        &self,
        period: &CalendarPeriod,
    ) -> Result<CalendarReadModel, CalendarStoreError> {
        let db = self
            .database
            .lock()
            .map_err(|_| CalendarStoreError::Backend)?;
        let conn = db.connection();

        let mut stmt = conn
            .prepare(
                r#"
                SELECT 
                    usage_date,
                    COALESCE(SUM(total_tokens), 0) AS total_tokens,
                    COUNT(DISTINCT source_id) AS active_sources,
                    COALESCE(SUM(CASE
                        WHEN cost_status IN ('available', 'estimated')
                        THEN cost_amount_micros
                        ELSE 0
                    END), 0) AS cost_micros,
                    COALESCE(SUM(CASE
                        WHEN cost_status IN ('available', 'estimated')
                        THEN 1 ELSE 0
                    END), 0) AS valued_days,
                    COALESCE(SUM(CASE
                        WHEN cost_status = 'estimated'
                        THEN 1 ELSE 0
                    END), 0) AS estimated_days,
                    COALESCE(SUM(CASE
                        WHEN cost_status = 'unavailable'
                        THEN 1 ELSE 0
                    END), 0) AS unavailable_days,
                    MIN(CASE
                        WHEN cost_status IN ('available', 'estimated')
                        THEN cost_currency
                    END) AS minimum_currency,
                    MAX(CASE
                        WHEN cost_status IN ('available', 'estimated')
                        THEN cost_currency
                    END) AS maximum_currency,
                    MAX(CASE
                        WHEN data_quality <> 'complete' OR record_state = 'missing'
                        THEN 1 ELSE 0
                    END) AS has_partial_data
                FROM daily_usage
                WHERE record_state <> 'removed'
                  AND usage_date >= ?1
                  AND usage_date <= ?2
                  AND aggregation_timezone = ?3
                GROUP BY usage_date
                ORDER BY usage_date ASC
                "#,
            )
            .map_err(|_| CalendarStoreError::Backend)?;

        let start_date_str = period.start_date().to_string();
        let end_date_str = period.end_date().to_string();

        let rows = stmt
            .query_map(
                params![
                    &start_date_str,
                    &end_date_str,
                    period.aggregation_timezone()
                ],
                |row| {
                    let usage_date: String = row.get(0)?;
                    let date = NaiveDate::parse_from_str(&usage_date, "%Y-%m-%d")
                        .unwrap_or(period.start_date());

                    Ok(CalendarDayRow {
                        date,
                        total_tokens: row.get(1)?,
                        active_sources: row.get(2)?,
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
            .map_err(|_| CalendarStoreError::Backend)?;

        let mut days = Vec::new();
        for row in rows {
            days.push(calendar_day_from_row(
                row.map_err(|_| CalendarStoreError::Backend)?,
            )?);
        }

        let data_status = if days.is_empty() {
            OverviewDataStatus::Empty
        } else if days.iter().any(|day| day.has_partial_data) {
            OverviewDataStatus::Partial
        } else {
            OverviewDataStatus::Current
        };

        Ok(CalendarReadModel {
            period: period.clone(),
            days,
            data_status,
        })
    }
}

impl DayDetailStore for SqliteCalendarStore {
    fn read_day_detail(
        &self,
        period: &DayDetailPeriod,
    ) -> Result<DayDetailReadModel, DayDetailStoreError> {
        let db = self
            .database
            .lock()
            .map_err(|_| DayDetailStoreError::Backend)?;
        let conn = db.connection();

        let date = period.date();
        let date_str = date.to_string();

        let mut stmt = conn
            .prepare(
                r#"
                SELECT 
                    COALESCE(SUM(total_tokens), 0) AS total_tokens,
                    COALESCE(SUM(CASE
                        WHEN cost_status IN ('available', 'estimated')
                        THEN cost_amount_micros
                        ELSE 0
                    END), 0) AS cost_micros,
                    COALESCE(SUM(CASE
                        WHEN cost_status IN ('available', 'estimated')
                        THEN 1 ELSE 0
                    END), 0) AS valued_days,
                    COALESCE(SUM(CASE
                        WHEN cost_status = 'estimated'
                        THEN 1 ELSE 0
                    END), 0) AS estimated_days,
                    COALESCE(SUM(CASE
                        WHEN cost_status = 'unavailable'
                        THEN 1 ELSE 0
                    END), 0) AS unavailable_days,
                    MIN(CASE
                        WHEN cost_status IN ('available', 'estimated')
                        THEN cost_currency
                    END) AS minimum_currency,
                    MAX(CASE
                        WHEN cost_status IN ('available', 'estimated')
                        THEN cost_currency
                    END) AS maximum_currency
                FROM daily_usage
                WHERE record_state <> 'removed'
                  AND usage_date = ?1
                  AND aggregation_timezone = ?2
                "#,
            )
            .map_err(|_| DayDetailStoreError::Backend)?;

        let mut rows = stmt
            .query(params![&date_str, period.aggregation_timezone()])
            .map_err(|_| DayDetailStoreError::Backend)?;

        let row = match rows.next().map_err(|_| DayDetailStoreError::Backend)? {
            Some(row) => row,
            None => {
                return Ok(empty_day_detail(date));
            }
        };

        let total_tokens: Option<i64> = row.get(0).map_err(|_| DayDetailStoreError::Backend)?;
        if total_tokens.is_none() {
            return Ok(empty_day_detail(date));
        }
        let total_tokens = u64::try_from(total_tokens.unwrap_or(0))
            .map_err(|_| DayDetailStoreError::ValueOutOfRange)?;
        let cost = day_detail_cost_from_values(
            row.get(1).map_err(|_| DayDetailStoreError::Backend)?,
            row.get(2).map_err(|_| DayDetailStoreError::Backend)?,
            row.get(3).map_err(|_| DayDetailStoreError::Backend)?,
            row.get(4).map_err(|_| DayDetailStoreError::Backend)?,
            row.get(5).map_err(|_| DayDetailStoreError::Backend)?,
            row.get(6).map_err(|_| DayDetailStoreError::Backend)?,
        )?;

        // Now get models
        let mut model_stmt = conn
            .prepare(
                r#"
                SELECT 
                    sources.source_key,
                    COALESCE(sm.display_name, sm.raw_model_id) AS model_name,
                    dmu.total_tokens,
                    dmu.cost_amount_micros,
                    dmu.cost_currency,
                    dmu.cost_status
                FROM daily_model_usage dmu
                INNER JOIN daily_usage du ON dmu.daily_usage_id = du.id
                INNER JOIN sources ON sources.id = du.source_id
                LEFT JOIN source_models sm ON dmu.model_id = sm.id
                WHERE du.record_state <> 'removed'
                  AND du.usage_date = ?1
                  AND du.aggregation_timezone = ?2
                ORDER BY model_name ASC
                "#,
            )
            .map_err(|_| DayDetailStoreError::Backend)?;

        let model_rows = model_stmt
            .query_map(params![&date_str, period.aggregation_timezone()], |row| {
                let source_key_str: String = row.get(0)?;
                let source = SourceKey::from_storage(&source_key_str)
                    .ok_or(rusqlite::Error::InvalidQuery)?;
                let model_name: String = row.get(1)?;
                let m_tokens: Option<i64> = row.get(2)?;
                let m_cost: Option<i64> = row.get(3)?;
                let m_curr: Option<String> = row.get(4)?;
                let cost_status: String = row.get(5)?;

                let m_currency = m_curr.and_then(|c| CurrencyCode::new(c.as_str()).ok());
                let valuation = match cost_status.as_str() {
                    "estimated" => crate::application::usage::CostValuation::Estimated,
                    _ => crate::application::usage::CostValuation::Unavailable,
                };
                let completeness = match cost_status.as_str() {
                    "estimated" => crate::application::usage::CostCompleteness::Complete,
                    _ => crate::application::usage::CostCompleteness::Unavailable,
                };

                Ok(DayDetailModel {
                    source,
                    model: model_name,
                    tokens: u64::try_from(m_tokens.unwrap_or(0))
                        .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(2, 0))?,
                    cost: OverviewCost {
                        amount_micros: m_cost
                            .map(u64::try_from)
                            .transpose()
                            .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(3, 0))?,
                        currency: m_currency,
                        valuation,
                        completeness,
                        unavailable_days: 0,
                    },
                })
            })
            .map_err(|_| DayDetailStoreError::Backend)?;

        let mut models = Vec::new();
        for m_row in model_rows {
            models.push(m_row.map_err(|_| DayDetailStoreError::Backend)?);
        }

        Ok(DayDetailReadModel {
            date,
            total_tokens,
            cost,
            models,
            as_of_ms: 0,
        })
    }
}

struct CalendarDayRow {
    date: NaiveDate,
    total_tokens: i64,
    active_sources: i64,
    cost_amount_micros: i64,
    valued_days: i64,
    estimated_days: i64,
    unavailable_days: i64,
    minimum_currency: Option<String>,
    maximum_currency: Option<String>,
    has_partial_data: bool,
}

fn calendar_day_from_row(row: CalendarDayRow) -> Result<CalendarDayInfo, CalendarStoreError> {
    let total_tokens =
        u64::try_from(row.total_tokens).map_err(|_| CalendarStoreError::ValueOutOfRange)?;
    let active_sources =
        u32::try_from(row.active_sources).map_err(|_| CalendarStoreError::ValueOutOfRange)?;
    let cost = calendar_cost_from_values(
        row.cost_amount_micros,
        row.valued_days,
        row.estimated_days,
        row.unavailable_days,
        row.minimum_currency,
        row.maximum_currency,
    )?;

    Ok(CalendarDayInfo {
        date: row.date,
        total_tokens,
        active_sources,
        cost,
        has_partial_data: row.has_partial_data,
    })
}

fn calendar_cost_from_values(
    amount_micros: i64,
    valued_days: i64,
    estimated_days: i64,
    unavailable_days: i64,
    minimum_currency: Option<String>,
    maximum_currency: Option<String>,
) -> Result<OverviewCost, CalendarStoreError> {
    cost_from_values(
        amount_micros,
        valued_days,
        estimated_days,
        unavailable_days,
        minimum_currency,
        maximum_currency,
    )
    .map_err(|error| match error {
        CostReadError::ValueOutOfRange => CalendarStoreError::ValueOutOfRange,
        CostReadError::MixedCurrencies => CalendarStoreError::MixedCurrencies,
        CostReadError::Backend => CalendarStoreError::Backend,
    })
}

fn day_detail_cost_from_values(
    amount_micros: i64,
    valued_days: i64,
    estimated_days: i64,
    unavailable_days: i64,
    minimum_currency: Option<String>,
    maximum_currency: Option<String>,
) -> Result<OverviewCost, DayDetailStoreError> {
    cost_from_values(
        amount_micros,
        valued_days,
        estimated_days,
        unavailable_days,
        minimum_currency,
        maximum_currency,
    )
    .map_err(|error| match error {
        CostReadError::ValueOutOfRange => DayDetailStoreError::ValueOutOfRange,
        CostReadError::MixedCurrencies => DayDetailStoreError::MixedCurrencies,
        CostReadError::Backend => DayDetailStoreError::Backend,
    })
}

enum CostReadError {
    Backend,
    ValueOutOfRange,
    MixedCurrencies,
}

fn cost_from_values(
    amount_micros: i64,
    valued_days: i64,
    estimated_days: i64,
    unavailable_days: i64,
    minimum_currency: Option<String>,
    maximum_currency: Option<String>,
) -> Result<OverviewCost, CostReadError> {
    let unavailable_days =
        u32::try_from(unavailable_days).map_err(|_| CostReadError::ValueOutOfRange)?;
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
        return Err(CostReadError::MixedCurrencies);
    }

    let currency = minimum_currency
        .and_then(|value| CurrencyCode::new(value).ok())
        .ok_or(CostReadError::Backend)?;
    let amount_micros = u64::try_from(amount_micros).map_err(|_| CostReadError::ValueOutOfRange)?;

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

fn empty_day_detail(date: NaiveDate) -> DayDetailReadModel {
    DayDetailReadModel {
        date,
        total_tokens: 0,
        cost: OverviewCost {
            amount_micros: None,
            currency: None,
            valuation: CostValuation::Unavailable,
            completeness: CostCompleteness::Unavailable,
            unavailable_days: 0,
        },
        models: Vec::new(),
        as_of_ms: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::{params, Connection};

    fn migrated_store() -> (tempfile::TempDir, SqliteCalendarStore) {
        let directory = tempfile::tempdir().expect("create temporary directory");
        let database_path = directory.path().join("burnly.sqlite3");
        let mut database = Database::open(&database_path).expect("open database");
        database.migrate_to_latest().expect("migrate database");
        seed_settings(database.connection());
        (directory, SqliteCalendarStore::new(database))
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

    fn seed_daily_usage(
        connection: &Connection,
        source_id: i64,
        source_key: &str,
        date_str: &str,
        total_tokens: i64,
        import_id: i64,
    ) -> i64 {
        seed_daily_usage_with(
            connection,
            DailyUsageSeed::new(source_id, source_key, date_str, total_tokens, import_id),
        )
    }

    struct DailyUsageSeed<'a> {
        source_id: i64,
        source_key: &'a str,
        date_str: &'a str,
        total_tokens: i64,
        import_id: i64,
        aggregation_timezone: &'a str,
        cost_amount_micros: Option<i64>,
        cost_currency: Option<&'a str>,
        cost_status: &'a str,
        data_quality: &'a str,
        record_state: &'a str,
        absence_count: i64,
        removed_at_ms: Option<i64>,
    }

    impl<'a> DailyUsageSeed<'a> {
        fn new(
            source_id: i64,
            source_key: &'a str,
            date_str: &'a str,
            total_tokens: i64,
            import_id: i64,
        ) -> Self {
            Self {
                source_id,
                source_key,
                date_str,
                total_tokens,
                import_id,
                aggregation_timezone: "UTC",
                cost_amount_micros: None,
                cost_currency: None,
                cost_status: "unavailable",
                data_quality: "complete",
                record_state: "active",
                absence_count: 0,
                removed_at_ms: None,
            }
        }

        fn timezone(mut self, aggregation_timezone: &'a str) -> Self {
            self.aggregation_timezone = aggregation_timezone;
            self
        }

        fn cost(mut self, amount_micros: i64, currency: &'a str, status: &'a str) -> Self {
            self.cost_amount_micros = Some(amount_micros);
            self.cost_currency = Some(currency);
            self.cost_status = status;
            self
        }

        fn state(mut self, record_state: &'a str, absence_count: i64) -> Self {
            self.record_state = record_state;
            self.absence_count = absence_count;
            self.removed_at_ms = (record_state == "removed").then_some(300);
            self
        }
    }

    fn seed_daily_usage_with(connection: &Connection, seed: DailyUsageSeed<'_>) -> i64 {
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
                    seed.date_str,
                    seed.aggregation_timezone,
                    seed.total_tokens,
                    seed.cost_amount_micros,
                    seed.cost_currency,
                    seed.cost_status,
                    seed.data_quality,
                    seed.record_state,
                    seed.absence_count,
                    seed.removed_at_ms,
                    seed.import_id
                ],
            )
            .expect("seed daily usage");
        connection.last_insert_rowid()
    }

    fn calendar_period(start: &str, end: &str, timezone: &str) -> CalendarPeriod {
        CalendarPeriod::new(
            NaiveDate::parse_from_str(start, "%Y-%m-%d").expect("start date"),
            NaiveDate::parse_from_str(end, "%Y-%m-%d").expect("end date"),
            timezone,
        )
        .expect("calendar period")
    }

    fn day_detail_period(date: &str, timezone: &str) -> DayDetailPeriod {
        DayDetailPeriod::new(
            NaiveDate::parse_from_str(date, "%Y-%m-%d").expect("detail date"),
            timezone,
        )
        .expect("day detail period")
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

    #[test]
    fn calendar_respects_timezone_and_includes_missing_but_not_removed_rows() {
        let (_directory, store) = migrated_store();
        {
            let db = store.database.lock().expect("lock");
            let conn = db.connection();
            seed_source(conn, 1, "claude-code");
            let refresh_id = seed_refresh(conn, "succeeded", 200);
            let import_id = seed_import(conn, refresh_id, 1);

            seed_daily_usage_with(
                conn,
                DailyUsageSeed::new(
                    1,
                    "claude-code:daily:v1:UTC:2026-06-13",
                    "2026-06-13",
                    100,
                    import_id,
                )
                .cost(20, "USD", "estimated"),
            );
            seed_daily_usage_with(
                conn,
                DailyUsageSeed::new(
                    1,
                    "claude-code:daily:v1:UTC:2026-06-14",
                    "2026-06-14",
                    0,
                    import_id,
                )
                .state("missing", 1),
            );
            seed_daily_usage_with(
                conn,
                DailyUsageSeed::new(
                    1,
                    "claude-code:daily:v1:UTC:2026-06-15",
                    "2026-06-15",
                    300,
                    import_id,
                )
                .state("removed", 2),
            );
            seed_daily_usage_with(
                conn,
                DailyUsageSeed::new(
                    1,
                    "claude-code:daily:v1:Asia/Jakarta:2026-06-13",
                    "2026-06-13",
                    900,
                    import_id,
                )
                .timezone("Asia/Jakarta")
                .cost(180, "USD", "available"),
            );
        }

        let calendar = store
            .read_calendar(&calendar_period("2026-06-13", "2026-06-15", "UTC"))
            .expect("read calendar");

        assert_eq!(calendar.days.len(), 2);
        assert_eq!(calendar.days[0].date.to_string(), "2026-06-13");
        assert_eq!(calendar.days[0].total_tokens, 100);
        assert_eq!(calendar.days[0].cost.amount_micros, Some(20));
        assert_eq!(calendar.days[0].cost.valuation, CostValuation::Estimated);
        assert_eq!(calendar.days[1].date.to_string(), "2026-06-14");
        assert_eq!(calendar.days[1].total_tokens, 0);
        assert!(calendar.days[1].has_partial_data);
        assert_eq!(
            calendar.days[1].cost.completeness,
            CostCompleteness::Unavailable
        );
        assert_eq!(calendar.data_status, OverviewDataStatus::Partial);
    }

    #[test]
    fn calendar_reports_partial_cost_when_some_rows_are_unavailable() {
        let (_directory, store) = migrated_store();
        {
            let db = store.database.lock().expect("lock");
            let conn = db.connection();
            seed_source(conn, 1, "claude-code");
            seed_source(conn, 2, "codex");
            let refresh_id = seed_refresh(conn, "succeeded", 200);
            let claude_import = seed_import(conn, refresh_id, 1);
            let codex_import = seed_import(conn, refresh_id, 2);

            seed_daily_usage_with(
                conn,
                DailyUsageSeed::new(
                    1,
                    "claude-code:daily:v1:UTC:2026-06-13",
                    "2026-06-13",
                    100,
                    claude_import,
                )
                .cost(20, "USD", "available"),
            );
            seed_daily_usage(
                conn,
                2,
                "codex:daily:v1:UTC:2026-06-13",
                "2026-06-13",
                50,
                codex_import,
            );
        }

        let calendar = store
            .read_calendar(&calendar_period("2026-06-13", "2026-06-13", "UTC"))
            .expect("read calendar");

        assert_eq!(calendar.days.len(), 1);
        let day = &calendar.days[0];
        assert_eq!(day.total_tokens, 150);
        assert_eq!(day.active_sources, 2);
        assert_eq!(day.cost.amount_micros, Some(20));
        assert_eq!(day.cost.completeness, CostCompleteness::Partial);
        assert_eq!(day.cost.unavailable_days, 1);
    }

    #[test]
    fn reads_day_detail_with_models() {
        let (_directory, store) = migrated_store();
        {
            let db = store.database.lock().expect("lock");
            let conn = db.connection();
            seed_source(conn, 1, "claude-code");
            seed_model(conn, 10, 1, "claude-3-5-sonnet", Some("Claude 3.5 Sonnet"));
            seed_model(conn, 11, 1, "claude-3-haiku", None);

            let refresh_id = seed_refresh(conn, "succeeded", 200);
            let import_id = seed_import(conn, refresh_id, 1);
            let daily_id = seed_daily_usage(
                conn,
                1,
                "claude-code:daily:v1:UTC:2026-06-13",
                "2026-06-13",
                1000,
                import_id,
            );

            seed_daily_model_usage(
                conn,
                daily_id,
                1,
                10,
                800,
                Some(2400),
                Some("USD"),
                "estimated",
                import_id,
            );
            seed_daily_model_usage(
                conn,
                daily_id,
                1,
                11,
                200,
                None,
                None,
                "unavailable",
                import_id,
            );
        }

        let detail = store
            .read_day_detail(&day_detail_period("2026-06-13", "UTC"))
            .expect("read day detail");

        assert_eq!(detail.date, NaiveDate::from_ymd_opt(2026, 6, 13).unwrap());
        assert_eq!(detail.total_tokens, 1000);
        assert_eq!(detail.models.len(), 2);

        let m1 = &detail.models[0];
        assert_eq!(m1.source, SourceKey::ClaudeCode);
        assert_eq!(m1.model, "Claude 3.5 Sonnet");
        assert_eq!(m1.tokens, 800);
        assert_eq!(m1.cost.amount_micros, Some(2400));
        assert_eq!(m1.cost.currency, Some(CurrencyCode::new("USD").unwrap()));
        assert_eq!(
            m1.cost.valuation,
            crate::application::usage::CostValuation::Estimated
        );

        let m2 = &detail.models[1];
        assert_eq!(m2.source, SourceKey::ClaudeCode);
        assert_eq!(m2.model, "claude-3-haiku");
        assert_eq!(m2.tokens, 200);
        assert_eq!(m2.cost.amount_micros, None);
        assert_eq!(
            m2.cost.valuation,
            crate::application::usage::CostValuation::Unavailable
        );
    }

    #[test]
    fn reads_empty_day_detail() {
        let (_directory, store) = migrated_store();

        let detail = store
            .read_day_detail(&day_detail_period("2026-06-13", "UTC"))
            .expect("read day detail");

        assert_eq!(detail.date, NaiveDate::from_ymd_opt(2026, 6, 13).unwrap());
        assert_eq!(detail.total_tokens, 0);
        assert_eq!(detail.models.len(), 0);
        assert_eq!(
            detail.cost.valuation,
            crate::application::usage::CostValuation::Unavailable
        );
        assert_eq!(
            detail.cost.completeness,
            crate::application::usage::CostCompleteness::Unavailable
        );
    }
}
