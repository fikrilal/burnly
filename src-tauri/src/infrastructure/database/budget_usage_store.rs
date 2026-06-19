use std::sync::Mutex;

use rusqlite::{params, Connection};

use crate::application::budget_evaluation::{
    BudgetCostCompleteness, BudgetUsageAggregate, BudgetUsageCost, BudgetUsageRequest,
};
use crate::application::ports::budget_usage_store::{BudgetUsageStore, BudgetUsageStoreError};
use crate::domain::budget::BudgetScope;
use crate::domain::usage::CurrencyCode;

use super::Database;

pub(crate) struct SqliteBudgetUsageStore {
    database: Mutex<Database>,
}

impl SqliteBudgetUsageStore {
    pub(crate) fn new(database: Database) -> Self {
        Self {
            database: Mutex::new(database),
        }
    }
}

impl BudgetUsageStore for SqliteBudgetUsageStore {
    fn aggregate_budget_usage(
        &self,
        request: &BudgetUsageRequest,
    ) -> Result<BudgetUsageAggregate, BudgetUsageStoreError> {
        let database = self
            .database
            .lock()
            .map_err(|_| BudgetUsageStoreError::Backend)?;
        aggregate(database.connection(), request)
    }
}

fn aggregate(
    connection: &Connection,
    request: &BudgetUsageRequest,
) -> Result<BudgetUsageAggregate, BudgetUsageStoreError> {
    let row = match request.scope {
        BudgetScope::Global => {
            let sql = aggregate_sql("");
            connection.query_row(
                &sql,
                params![
                    request.period.start_date().to_string(),
                    request.period.end_date().to_string(),
                    request.period.aggregation_timezone(),
                ],
                BudgetUsageRow::from_row,
            )
        }
        BudgetScope::Source(source_id) => {
            let sql = aggregate_sql("AND source_id = ?4");
            connection.query_row(
                &sql,
                params![
                    request.period.start_date().to_string(),
                    request.period.end_date().to_string(),
                    request.period.aggregation_timezone(),
                    source_id,
                ],
                BudgetUsageRow::from_row,
            )
        }
    }
    .map_err(|_| BudgetUsageStoreError::Backend)?;

    row.into_domain()
}

fn aggregate_sql(scope_clause: &'static str) -> String {
    format!(
        "SELECT
            COALESCE(SUM(total_tokens), 0),
            COUNT(DISTINCT usage_date),
            COALESCE(SUM(CASE
                WHEN cost_status IN ('available', 'estimated')
                THEN cost_amount_micros
                ELSE 0
            END), 0),
            COALESCE(SUM(CASE
                WHEN cost_status IN ('available', 'estimated')
                THEN 1 ELSE 0
            END), 0),
            COALESCE(SUM(CASE
                WHEN cost_status = 'unavailable'
                THEN 1 ELSE 0
            END), 0),
            MIN(CASE
                WHEN cost_status IN ('available', 'estimated')
                THEN cost_currency
            END),
            MAX(CASE
                WHEN cost_status IN ('available', 'estimated')
                THEN cost_currency
            END)
         FROM daily_usage
         WHERE usage_date BETWEEN ?1 AND ?2
            AND aggregation_timezone = ?3
            AND record_state = 'active'
            {scope_clause}"
    )
}

struct BudgetUsageRow {
    total_tokens: i64,
    active_days: i64,
    cost_amount_micros: i64,
    valued_days: i64,
    unavailable_days: i64,
    minimum_currency: Option<String>,
    maximum_currency: Option<String>,
}

impl BudgetUsageRow {
    fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            total_tokens: row.get(0)?,
            active_days: row.get(1)?,
            cost_amount_micros: row.get(2)?,
            valued_days: row.get(3)?,
            unavailable_days: row.get(4)?,
            minimum_currency: row.get(5)?,
            maximum_currency: row.get(6)?,
        })
    }

    fn into_domain(self) -> Result<BudgetUsageAggregate, BudgetUsageStoreError> {
        let total_tokens =
            u64::try_from(self.total_tokens).map_err(|_| BudgetUsageStoreError::ValueOutOfRange)?;
        let active_days =
            u32::try_from(self.active_days).map_err(|_| BudgetUsageStoreError::ValueOutOfRange)?;
        let unavailable_days = u32::try_from(self.unavailable_days)
            .map_err(|_| BudgetUsageStoreError::ValueOutOfRange)?;
        let cost = cost_from_values(
            self.cost_amount_micros,
            self.valued_days,
            unavailable_days,
            self.minimum_currency,
            self.maximum_currency,
        )?;

        Ok(BudgetUsageAggregate {
            total_tokens,
            active_days,
            cost,
        })
    }
}

fn cost_from_values(
    amount_micros: i64,
    valued_days: i64,
    unavailable_days: u32,
    minimum_currency: Option<String>,
    maximum_currency: Option<String>,
) -> Result<BudgetUsageCost, BudgetUsageStoreError> {
    if valued_days == 0 {
        return Ok(BudgetUsageCost {
            amount_micros: None,
            currency: None,
            completeness: BudgetCostCompleteness::Unavailable,
            unavailable_days,
        });
    }
    if minimum_currency != maximum_currency {
        return Err(BudgetUsageStoreError::MixedCurrencies);
    }

    let amount_micros =
        u64::try_from(amount_micros).map_err(|_| BudgetUsageStoreError::ValueOutOfRange)?;
    let currency = minimum_currency
        .map(CurrencyCode::new)
        .transpose()
        .map_err(|_| BudgetUsageStoreError::ValueOutOfRange)?
        .ok_or(BudgetUsageStoreError::ValueOutOfRange)?;
    let completeness = if unavailable_days == 0 {
        BudgetCostCompleteness::Complete
    } else {
        BudgetCostCompleteness::Partial
    };

    Ok(BudgetUsageCost {
        amount_micros: Some(amount_micros),
        currency: Some(currency),
        completeness,
        unavailable_days,
    })
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::*;
    use crate::application::budget_evaluation::BudgetPeriodWindow;
    use crate::domain::budget::BudgetPeriod;

    #[test]
    fn aggregates_global_and_source_active_daily_facts() {
        let store = store();
        seed_daily_usage(
            &store,
            DailySeed::new(1, "2026-06-01", 1_000, Some(100_000)),
        );
        seed_daily_usage(
            &store,
            DailySeed::new(2, "2026-06-02", 2_000, Some(200_000)),
        );
        seed_daily_usage(
            &store,
            DailySeed::new(2, "2026-06-03", 9_000, Some(900_000)).removed(),
        );
        seed_daily_usage(
            &store,
            DailySeed::new(1, "2026-05-31", 4_000, Some(400_000)),
        );

        let global = store
            .aggregate_budget_usage(&request(BudgetScope::Global))
            .expect("aggregate global");
        assert_eq!(global.total_tokens, 3_000);
        assert_eq!(global.active_days, 2);
        assert_eq!(global.cost.amount_micros, Some(300_000));

        let source = store
            .aggregate_budget_usage(&request(BudgetScope::source(2).expect("scope")))
            .expect("aggregate source");
        assert_eq!(source.total_tokens, 2_000);
        assert_eq!(source.active_days, 1);
        assert_eq!(source.cost.amount_micros, Some(200_000));
    }

    #[test]
    fn preserves_unavailable_and_partial_cost_semantics() {
        let store = store();
        seed_daily_usage(
            &store,
            DailySeed::new(1, "2026-06-01", 1_000, Some(100_000)),
        );
        seed_daily_usage(&store, DailySeed::new(1, "2026-06-02", 1_000, None));

        let aggregate = store
            .aggregate_budget_usage(&request(BudgetScope::Global))
            .expect("aggregate");

        assert_eq!(aggregate.cost.amount_micros, Some(100_000));
        assert_eq!(aggregate.cost.completeness, BudgetCostCompleteness::Partial);
        assert_eq!(aggregate.cost.unavailable_days, 1);

        let empty = store
            .aggregate_budget_usage(&request(BudgetScope::source(99).expect("scope")))
            .expect("aggregate empty");
        assert_eq!(empty.total_tokens, 0);
        assert_eq!(empty.cost.completeness, BudgetCostCompleteness::Unavailable);
    }

    #[test]
    fn rejects_mixed_cost_currencies() {
        let store = store();
        seed_daily_usage(
            &store,
            DailySeed::new(1, "2026-06-01", 1_000, Some(100_000)),
        );
        seed_daily_usage(
            &store,
            DailySeed::new(1, "2026-06-02", 1_000, Some(200_000)).currency("EUR"),
        );

        assert_eq!(
            store.aggregate_budget_usage(&request(BudgetScope::Global)),
            Err(BudgetUsageStoreError::MixedCurrencies)
        );
    }

    fn store() -> SqliteBudgetUsageStore {
        let directory = tempfile::TempDir::new().expect("temporary directory");
        let path = directory.keep().join("burnly.sqlite3");
        let mut database = Database::open(path).expect("open database");
        database.migrate_to_latest().expect("migrate database");
        seed_source(database.connection(), 1, "claude-code");
        seed_source(database.connection(), 2, "codex");
        seed_import(database.connection(), 1);
        seed_import(database.connection(), 2);
        SqliteBudgetUsageStore::new(database)
    }

    fn request(scope: BudgetScope) -> BudgetUsageRequest {
        BudgetUsageRequest {
            period: BudgetPeriodWindow::new(
                BudgetPeriod::Monthly,
                NaiveDate::from_ymd_opt(2026, 6, 1).expect("date"),
                NaiveDate::from_ymd_opt(2026, 6, 30).expect("date"),
                "UTC",
            )
            .expect("period"),
            scope,
        }
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
            .expect("insert source");
    }

    fn seed_import(connection: &Connection, source_id: i64) {
        connection
            .execute(
                "INSERT INTO refresh_runs (
                    id, job_key, trigger, status, started_at_ms, finished_at_ms,
                    requested_by_app_version, created_at_ms
                 ) VALUES (?1, ?2, 'manual', 'succeeded', 0, 1, '0.1.0', 0)",
                params![source_id, format!("job-{source_id}")],
            )
            .expect("insert refresh run");
        connection
            .execute(
                "INSERT INTO import_runs (
                    id, refresh_run_id, source_id, collector_key, collector_version,
                    profile_version, projection, scope_kind, aggregation_timezone,
                    status, records_seen, records_rejected, started_at_ms, finished_at_ms
                 ) VALUES (
                    ?1, ?1, ?1, 'ccusage', '20.0.14', 1, 'daily', 'full', 'UTC',
                    'succeeded', 1, 0, 0, 1
                 )",
                [source_id],
            )
            .expect("insert import run");
    }

    #[derive(Clone)]
    struct DailySeed<'a> {
        source_id: i64,
        usage_date: &'a str,
        total_tokens: u64,
        cost_amount_micros: Option<u64>,
        currency: &'a str,
        record_state: &'a str,
    }

    impl<'a> DailySeed<'a> {
        fn new(
            source_id: i64,
            usage_date: &'a str,
            total_tokens: u64,
            cost_amount_micros: Option<u64>,
        ) -> Self {
            Self {
                source_id,
                usage_date,
                total_tokens,
                cost_amount_micros,
                currency: "USD",
                record_state: "active",
            }
        }

        fn currency(mut self, currency: &'a str) -> Self {
            self.currency = currency;
            self
        }

        fn removed(mut self) -> Self {
            self.record_state = "removed";
            self
        }
    }

    fn seed_daily_usage(store: &SqliteBudgetUsageStore, seed: DailySeed<'_>) {
        let database = store.database.lock().expect("database lock");
        let source_key = if seed.source_id == 1 {
            "claude-code"
        } else {
            "codex"
        };
        let cost_status = if seed.cost_amount_micros.is_some() {
            "estimated"
        } else {
            "unavailable"
        };
        database
            .connection()
            .execute(
                "INSERT INTO daily_usage (
                    source_id, source_key, identity_version, usage_date,
                    aggregation_timezone, total_tokens, cost_amount_micros,
                    cost_currency, cost_kind, cost_status, data_quality,
                    record_state, absence_count, first_seen_at_ms, last_seen_at_ms,
                    removed_at_ms, latest_import_id
                 ) VALUES (
                    ?1, ?2, 1, ?3, 'UTC', ?4, ?5, ?6, 'collector_calculated',
                    ?7, 'complete', ?8, ?9, 0, 0, ?10, ?1
                 )",
                params![
                    seed.source_id,
                    format!("{source_key}:daily:v1:UTC:{}", seed.usage_date),
                    seed.usage_date,
                    i64::try_from(seed.total_tokens).expect("tokens"),
                    seed.cost_amount_micros
                        .map(|value| i64::try_from(value).expect("cost")),
                    seed.cost_amount_micros.map(|_| seed.currency),
                    cost_status,
                    seed.record_state,
                    if seed.record_state == "active" { 0 } else { 2 },
                    if seed.record_state == "active" {
                        None
                    } else {
                        Some(1)
                    },
                ],
            )
            .expect("insert daily usage");
    }
}
