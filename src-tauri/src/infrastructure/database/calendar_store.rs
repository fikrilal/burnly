use crate::infrastructure::database::Database;
use chrono::NaiveDate;
use std::sync::Mutex;

use crate::application::ports::calendar_store::{CalendarStore, CalendarStoreError};
use crate::application::ports::day_detail_store::{DayDetailStore, DayDetailStoreError};
use crate::application::usage::{
    CalendarDayInfo, CalendarPeriod, CalendarReadModel, DayDetailModel, DayDetailReadModel,
    OverviewCost, OverviewDataStatus,
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
                    SUM(total_tokens) as total_tokens,
                    COUNT(DISTINCT source_id) as active_sources,
                    SUM(cost_amount_micros) as cost_micros,
                    MAX(cost_currency) as currency
                FROM daily_usage
                WHERE record_state = 'active'
                  AND usage_date >= ?
                  AND usage_date <= ?
                GROUP BY usage_date
                ORDER BY usage_date ASC
                "#,
            )
            .map_err(|_| CalendarStoreError::Backend)?;

        let start_date_str = period.start_date().to_string();
        let end_date_str = period.end_date().to_string();

        let rows = stmt
            .query_map([&start_date_str, &end_date_str], |row| {
                let usage_date: String = row.get(0)?;
                let date = NaiveDate::parse_from_str(&usage_date, "%Y-%m-%d")
                    .unwrap_or(period.start_date());

                let total_tokens: i64 = row.get(1).unwrap_or(0);
                let active_sources: i64 = row.get(2).unwrap_or(0);
                let cost_micros: Option<i64> = row.get(3)?;
                let currency_str: Option<String> = row.get(4)?;

                let currency = currency_str.and_then(|c| CurrencyCode::new(c.as_str()).ok());

                Ok(CalendarDayInfo {
                    date,
                    total_tokens: total_tokens as u64,
                    active_sources: active_sources as u32,
                    cost: OverviewCost {
                        amount_micros: cost_micros.map(|v| v as u64),
                        currency,
                        valuation: crate::application::usage::CostValuation::Unavailable, // Simplify for now
                        completeness: crate::application::usage::CostCompleteness::Unavailable,
                        unavailable_days: 0,
                    },
                    has_partial_data: false,
                })
            })
            .map_err(|_| CalendarStoreError::Backend)?;

        let mut days = Vec::new();
        for row in rows {
            days.push(row.map_err(|_| CalendarStoreError::Backend)?);
        }

        Ok(CalendarReadModel {
            period: period.clone(),
            days,
            data_status: OverviewDataStatus::Current,
        })
    }
}

impl DayDetailStore for SqliteCalendarStore {
    fn read_day_detail(
        &self,
        date: NaiveDate,
    ) -> Result<Option<DayDetailReadModel>, DayDetailStoreError> {
        let db = self
            .database
            .lock()
            .map_err(|_| DayDetailStoreError::Backend)?;
        let conn = db.connection();

        let date_str = date.to_string();

        let mut stmt = conn
            .prepare(
                r#"
                SELECT 
                    SUM(total_tokens) as total_tokens,
                    SUM(cost_amount_micros) as cost_micros,
                    MAX(cost_currency) as currency
                FROM daily_usage
                WHERE record_state = 'active'
                  AND usage_date = ?
                "#,
            )
            .map_err(|_| DayDetailStoreError::Backend)?;

        let mut rows = stmt
            .query([&date_str])
            .map_err(|_| DayDetailStoreError::Backend)?;

        let row = match rows.next().map_err(|_| DayDetailStoreError::Backend)? {
            Some(row) => row,
            None => {
                return Ok(Some(DayDetailReadModel {
                    date,
                    total_tokens: 0,
                    cost: OverviewCost {
                        amount_micros: None,
                        currency: None,
                        valuation: crate::application::usage::CostValuation::Unavailable,
                        completeness: crate::application::usage::CostCompleteness::Unavailable,
                        unavailable_days: 0,
                    },
                    models: Vec::new(),
                    as_of_ms: 0,
                }));
            }
        };

        let total_tokens: Option<i64> = row.get(0).map_err(|_| DayDetailStoreError::Backend)?;
        if total_tokens.is_none() {
            return Ok(Some(DayDetailReadModel {
                date,
                total_tokens: 0,
                cost: OverviewCost {
                    amount_micros: None,
                    currency: None,
                    valuation: crate::application::usage::CostValuation::Unavailable,
                    completeness: crate::application::usage::CostCompleteness::Unavailable,
                    unavailable_days: 0,
                },
                models: Vec::new(),
                as_of_ms: 0,
            }));
        }
        let total_tokens = total_tokens.unwrap_or(0);
        let cost_micros: Option<i64> = row.get(1).map_err(|_| DayDetailStoreError::Backend)?;
        let currency_str: Option<String> = row.get(2).map_err(|_| DayDetailStoreError::Backend)?;

        let currency = currency_str.and_then(|c| CurrencyCode::new(c.as_str()).ok());

        // Now get models
        let mut model_stmt = conn
            .prepare(
                r#"
                SELECT 
                    du.source_key,
                    COALESCE(sm.display_name, sm.raw_model_id) AS model_name,
                    dmu.total_tokens,
                    dmu.cost_amount_micros,
                    dmu.cost_currency,
                    dmu.cost_status
                FROM daily_model_usage dmu
                INNER JOIN daily_usage du ON dmu.daily_usage_id = du.id
                LEFT JOIN source_models sm ON dmu.model_id = sm.id
                WHERE du.record_state = 'active'
                  AND du.usage_date = ?
                ORDER BY model_name ASC
                "#,
            )
            .map_err(|_| DayDetailStoreError::Backend)?;

        let model_rows = model_stmt
            .query_map([&date_str], |row| {
                let source_key_str: String = row.get(0)?;
                let source =
                    SourceKey::from_storage(&source_key_str).unwrap_or(SourceKey::ClaudeCode);
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
                    tokens: m_tokens.unwrap_or(0) as u64,
                    cost: OverviewCost {
                        amount_micros: m_cost.map(|v| v as u64),
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

        Ok(Some(DayDetailReadModel {
            date,
            total_tokens: total_tokens as u64,
            cost: OverviewCost {
                amount_micros: cost_micros.map(|v| v as u64),
                currency,
                valuation: crate::application::usage::CostValuation::Unavailable,
                completeness: crate::application::usage::CostCompleteness::Unavailable,
                unavailable_days: 0,
            },
            models,
            as_of_ms: 0,
        }))
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
        connection
            .execute(
                "INSERT INTO daily_usage (
                    source_id, source_key, identity_version, usage_date,
                    aggregation_timezone, total_tokens, cost_amount_micros,
                    cost_currency, cost_kind, cost_status, data_quality,
                    record_state, absence_count, first_seen_at_ms, last_seen_at_ms,
                    latest_import_id
                ) VALUES (?1, ?2, 1, ?3, 'UTC', ?4, NULL, NULL,
                    'collector_calculated', 'unavailable', 'complete',
                    'active', 0, 100, 200, ?5)",
                params![source_id, source_key, date_str, total_tokens, import_id],
            )
            .expect("seed daily usage");
        connection.last_insert_rowid()
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
            .read_day_detail(NaiveDate::from_ymd_opt(2026, 6, 13).unwrap())
            .expect("read day detail")
            .expect("should return some detail");

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
            .read_day_detail(NaiveDate::from_ymd_opt(2026, 6, 13).unwrap())
            .expect("read day detail")
            .expect("should return some empty detail model");

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
