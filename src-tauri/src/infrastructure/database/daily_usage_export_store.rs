//! SQLite adapter for scoped daily usage export (collect sync).

#![allow(
    dead_code,
    reason = "Constructed by collect-sync composition in later chunks"
)]

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

use chrono::NaiveDate;
use rusqlite::{params, params_from_iter, Connection};

use crate::application::collect_sync::{ExportedDailyFact, ExportedDailyModel, UploadScope};
use crate::application::ports::daily_usage_export_store::{
    DailyUsageExportQuery, DailyUsageExportStore, DailyUsageExportStoreError,
};

use super::Database;

pub(crate) struct SqliteDailyUsageExportStore {
    database: Mutex<Database>,
}

impl SqliteDailyUsageExportStore {
    pub(crate) fn new(database: Database) -> Self {
        Self {
            database: Mutex::new(database),
        }
    }
}

impl DailyUsageExportStore for SqliteDailyUsageExportStore {
    fn export_daily_facts(
        &self,
        query: &DailyUsageExportQuery,
    ) -> Result<Vec<ExportedDailyFact>, DailyUsageExportStoreError> {
        let database = self
            .database
            .lock()
            .map_err(|_| DailyUsageExportStoreError::Backend)?;
        export_daily_facts(database.connection(), query)
    }
}

fn export_daily_facts(
    connection: &Connection,
    query: &DailyUsageExportQuery,
) -> Result<Vec<ExportedDailyFact>, DailyUsageExportStoreError> {
    let reporting_timezone = query.reporting_timezone.trim();
    if reporting_timezone.is_empty() {
        return Err(DailyUsageExportStoreError::InvalidScope);
    }

    let parents = match &query.scope {
        UploadScope::Full => load_parents_full(connection, reporting_timezone)?,
        UploadScope::Incremental {
            source_keys,
            start_date,
            end_date,
        } => load_parents_incremental(
            connection,
            reporting_timezone,
            source_keys,
            *start_date,
            *end_date,
        )?,
    };

    if parents.is_empty() {
        return Ok(Vec::new());
    }

    let parent_ids: Vec<i64> = parents.keys().copied().collect();
    let mut models_by_parent = load_models(connection, &parent_ids)?;

    let mut facts = Vec::with_capacity(parents.len());
    for (id, mut fact) in parents.into_iter() {
        fact.models = models_by_parent.remove(&id).unwrap_or_default();
        facts.push(fact);
    }

    facts.sort_by(|left, right| {
        left.usage_date
            .cmp(&right.usage_date)
            .then_with(|| left.identity_key.cmp(&right.identity_key))
    });
    Ok(facts)
}

fn load_parents_full(
    connection: &Connection,
    reporting_timezone: &str,
) -> Result<BTreeMap<i64, ExportedDailyFact>, DailyUsageExportStoreError> {
    let mut statement = connection
        .prepare(
            "SELECT
                du.id,
                du.source_key,
                du.identity_version,
                sources.source_key,
                du.usage_date,
                du.aggregation_timezone,
                du.input_tokens,
                du.output_tokens,
                du.cache_creation_tokens,
                du.cache_read_tokens,
                du.total_tokens,
                du.unclassified_tokens,
                du.cost_status,
                du.cost_kind,
                du.cost_amount_micros,
                du.cost_currency,
                du.data_quality,
                du.record_state,
                du.first_seen_at_ms,
                du.last_seen_at_ms,
                du.removed_at_ms
             FROM daily_usage du
             INNER JOIN sources ON sources.id = du.source_id
             WHERE du.aggregation_timezone = ?1
               AND du.record_state IN ('active', 'missing', 'removed')
             ORDER BY du.usage_date ASC, du.source_key ASC",
        )
        .map_err(|_| DailyUsageExportStoreError::Backend)?;

    let rows = statement
        .query_map(params![reporting_timezone], map_parent_row)
        .map_err(|_| DailyUsageExportStoreError::Backend)?;

    collect_parents(rows)
}

fn load_parents_incremental(
    connection: &Connection,
    reporting_timezone: &str,
    source_keys: &BTreeSet<String>,
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> Result<BTreeMap<i64, ExportedDailyFact>, DailyUsageExportStoreError> {
    if source_keys.is_empty() {
        return Err(DailyUsageExportStoreError::InvalidScope);
    }

    let placeholders = source_keys
        .iter()
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT
            du.id,
            du.source_key,
            du.identity_version,
            sources.source_key,
            du.usage_date,
            du.aggregation_timezone,
            du.input_tokens,
            du.output_tokens,
            du.cache_creation_tokens,
            du.cache_read_tokens,
            du.total_tokens,
            du.unclassified_tokens,
            du.cost_status,
            du.cost_kind,
            du.cost_amount_micros,
            du.cost_currency,
            du.data_quality,
            du.record_state,
            du.first_seen_at_ms,
            du.last_seen_at_ms,
            du.removed_at_ms
         FROM daily_usage du
         INNER JOIN sources ON sources.id = du.source_id
         WHERE du.aggregation_timezone = ?1
           AND du.usage_date BETWEEN ?2 AND ?3
           AND sources.source_key IN ({placeholders})
           AND du.record_state IN ('active', 'missing', 'removed')
         ORDER BY du.usage_date ASC, du.source_key ASC"
    );

    let mut statement = connection
        .prepare(&sql)
        .map_err(|_| DailyUsageExportStoreError::Backend)?;

    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    params.push(Box::new(reporting_timezone.to_owned()));
    params.push(Box::new(start_date.to_string()));
    params.push(Box::new(end_date.to_string()));
    for key in source_keys {
        params.push(Box::new(key.clone()));
    }

    let rows = statement
        .query_map(
            params_from_iter(params.iter().map(|value| value.as_ref())),
            map_parent_row,
        )
        .map_err(|_| DailyUsageExportStoreError::Backend)?;

    collect_parents(rows)
}

fn collect_parents(
    rows: impl Iterator<Item = Result<(i64, ExportedDailyFact), rusqlite::Error>>,
) -> Result<BTreeMap<i64, ExportedDailyFact>, DailyUsageExportStoreError> {
    let mut parents = BTreeMap::new();
    for row in rows {
        let (id, fact) = row.map_err(|_| DailyUsageExportStoreError::Backend)?;
        parents.insert(id, fact);
    }
    Ok(parents)
}

fn map_parent_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<(i64, ExportedDailyFact)> {
    let id: i64 = row.get(0)?;
    let identity_version: i64 = row.get(2)?;
    let identity_version = u16::try_from(identity_version).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            2,
            rusqlite::types::Type::Integer,
            Box::new(error),
        )
    })?;
    let total_tokens: i64 = row.get(10)?;
    let total_tokens = u64::try_from(total_tokens).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            10,
            rusqlite::types::Type::Integer,
            Box::new(error),
        )
    })?;

    Ok((
        id,
        ExportedDailyFact {
            identity_key: row.get(1)?,
            identity_version,
            source_key: row.get(3)?,
            usage_date: row.get(4)?,
            aggregation_timezone: row.get(5)?,
            input_tokens: optional_u64(row, 6)?,
            output_tokens: optional_u64(row, 7)?,
            cache_creation_tokens: optional_u64(row, 8)?,
            cache_read_tokens: optional_u64(row, 9)?,
            total_tokens,
            unclassified_tokens: optional_u64(row, 11)?,
            cost_status: row.get(12)?,
            cost_kind: row.get(13)?,
            cost_amount_micros: row.get(14)?,
            cost_currency: row.get(15)?,
            data_quality: row.get(16)?,
            record_state: row.get(17)?,
            first_seen_at_ms: row.get(18)?,
            last_seen_at_ms: row.get(19)?,
            removed_at_ms: row.get(20)?,
            models: Vec::new(),
        },
    ))
}

fn load_models(
    connection: &Connection,
    parent_ids: &[i64],
) -> Result<BTreeMap<i64, Vec<ExportedDailyModel>>, DailyUsageExportStoreError> {
    if parent_ids.is_empty() {
        return Ok(BTreeMap::new());
    }

    let placeholders = parent_ids
        .iter()
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT
            dmu.daily_usage_id,
            sm.raw_model_id,
            sm.display_name,
            sm.provider_key,
            dmu.input_tokens,
            dmu.output_tokens,
            dmu.cache_creation_tokens,
            dmu.cache_read_tokens,
            dmu.total_tokens,
            dmu.cost_status,
            dmu.cost_amount_micros,
            dmu.cost_currency
         FROM daily_model_usage dmu
         LEFT JOIN source_models sm
           ON sm.id = dmu.model_id AND sm.source_id = dmu.source_id
         WHERE dmu.daily_usage_id IN ({placeholders})
         ORDER BY dmu.daily_usage_id ASC, sm.raw_model_id ASC, dmu.id ASC"
    );

    let mut statement = connection
        .prepare(&sql)
        .map_err(|_| DailyUsageExportStoreError::Backend)?;
    let rows = statement
        .query_map(params_from_iter(parent_ids.iter()), |row| {
            let parent_id: i64 = row.get(0)?;
            let model = ExportedDailyModel {
                raw_model_id: row.get(1)?,
                display_name: row.get(2)?,
                provider_key: row.get(3)?,
                input_tokens: optional_u64(row, 4)?,
                output_tokens: optional_u64(row, 5)?,
                cache_creation_tokens: optional_u64(row, 6)?,
                cache_read_tokens: optional_u64(row, 7)?,
                total_tokens: optional_u64(row, 8)?,
                cost_status: row.get(9)?,
                cost_amount_micros: row.get(10)?,
                cost_currency: row.get(11)?,
            };
            Ok((parent_id, model))
        })
        .map_err(|_| DailyUsageExportStoreError::Backend)?;

    let mut models_by_parent: BTreeMap<i64, Vec<ExportedDailyModel>> = BTreeMap::new();
    for row in rows {
        let (parent_id, model) = row.map_err(|_| DailyUsageExportStoreError::Backend)?;
        models_by_parent.entry(parent_id).or_default().push(model);
    }
    Ok(models_by_parent)
}

fn optional_u64(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<Option<u64>> {
    let value: Option<i64> = row.get(index)?;
    value
        .map(|number| {
            u64::try_from(number).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    index,
                    rusqlite::types::Type::Integer,
                    Box::new(error),
                )
            })
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::ports::daily_usage_export_store::DailyUsageExportStore;

    fn open_store() -> SqliteDailyUsageExportStore {
        let directory = tempfile::TempDir::new().expect("tempdir");
        let path = directory.keep().join("burnly.sqlite3");
        let mut database = Database::open(&path).expect("open");
        database.migrate_to_latest().expect("migrate");
        seed_usage(database.connection());
        SqliteDailyUsageExportStore::new(database)
    }

    #[test]
    fn full_export_reads_only_allowed_tables_and_timezone() {
        let store = open_store();
        let facts = store
            .export_daily_facts(&DailyUsageExportQuery::full("UTC"))
            .expect("export");
        assert_eq!(facts.len(), 2);
        assert_eq!(facts[0].usage_date, "2026-07-08");
        assert_eq!(facts[0].source_key, "claude-code");
        assert_eq!(facts[0].models.len(), 1);
        assert_eq!(
            facts[0].models[0].raw_model_id.as_deref(),
            Some("claude-sonnet-4")
        );
        assert_eq!(facts[1].usage_date, "2026-07-09");
    }

    #[test]
    fn incremental_export_filters_sources_and_dates() {
        let store = open_store();
        let query = DailyUsageExportQuery::incremental(
            "UTC",
            ["claude-code".to_owned()],
            NaiveDate::from_ymd_opt(2026, 7, 9).expect("date"),
            NaiveDate::from_ymd_opt(2026, 7, 9).expect("date"),
        )
        .expect("query");
        let facts = store.export_daily_facts(&query).expect("export");
        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].usage_date, "2026-07-09");
    }

    fn seed_usage(connection: &Connection) {
        connection
            .execute(
                "INSERT INTO sources (
                    id, source_key, display_name, enabled, detection_state,
                    created_at_ms, updated_at_ms
                 ) VALUES (1, 'claude-code', 'Claude Code', 1, 'available', 1, 1)",
                [],
            )
            .expect("source");
        connection
            .execute(
                "INSERT INTO source_models (
                    id, source_id, raw_model_id, display_name, provider_key,
                    first_seen_at_ms, last_seen_at_ms
                 ) VALUES (1, 1, 'claude-sonnet-4', NULL, 'anthropic', 1, 1)",
                [],
            )
            .expect("model");
        connection
            .execute(
                "INSERT INTO refresh_runs (
                    id, job_key, trigger, status, started_at_ms, finished_at_ms,
                    requested_by_app_version, created_at_ms
                 ) VALUES (1, 'job-1', 'manual', 'succeeded', 1, 2, '0.1.0', 1)",
                [],
            )
            .expect("refresh");
        connection
            .execute(
                "INSERT INTO import_runs (
                    id, refresh_run_id, source_id, collector_key, collector_version,
                    profile_version, projection, scope_kind, aggregation_timezone,
                    status, records_seen, records_rejected, started_at_ms, finished_at_ms
                 ) VALUES (
                    1, 1, 1, 'ccusage', '1.0.0',
                    1, 'daily', 'full', 'UTC',
                    'succeeded', 0, 0, 1, 2
                 )",
                [],
            )
            .expect("import");

        insert_daily(
            connection,
            1,
            "claude-code:daily:v1:UTC:2026-07-08",
            "2026-07-08",
            "active",
            None,
        );
        insert_daily(
            connection,
            2,
            "claude-code:daily:v1:UTC:2026-07-09",
            "2026-07-09",
            "active",
            None,
        );
        connection
            .execute(
                "INSERT INTO daily_model_usage (
                    id, daily_usage_id, source_id, model_id,
                    input_tokens, output_tokens, cache_creation_tokens, cache_read_tokens,
                    total_tokens, unclassified_tokens, cost_amount_micros, cost_currency,
                    cost_status, latest_import_id
                 ) VALUES (
                    1, 1, 1, 1,
                    10, 5, 0, 0,
                    15, 0, NULL, NULL,
                    'unavailable', 1
                 )",
                [],
            )
            .expect("daily model");
    }

    fn insert_daily(
        connection: &Connection,
        id: i64,
        identity_key: &str,
        usage_date: &str,
        record_state: &str,
        removed_at_ms: Option<i64>,
    ) {
        let absence = match record_state {
            "active" => 0,
            "missing" => 1,
            "removed" => 2,
            _ => 0,
        };
        connection
            .execute(
                "INSERT INTO daily_usage (
                    id, source_id, source_key, identity_version, usage_date,
                    aggregation_timezone, project_id,
                    input_tokens, output_tokens, cache_creation_tokens, cache_read_tokens,
                    total_tokens, unclassified_tokens,
                    cost_amount_micros, cost_currency, cost_kind, cost_status,
                    data_quality, record_state, absence_count,
                    first_seen_at_ms, last_seen_at_ms, removed_at_ms, latest_import_id
                 ) VALUES (
                    ?1, 1, ?2, 1, ?3,
                    'UTC', NULL,
                    10, 5, 0, 0,
                    15, 0,
                    NULL, NULL, 'unknown', 'unavailable',
                    'complete', ?4, ?5,
                    100, 200, ?6, 1
                 )",
                params![
                    id,
                    identity_key,
                    usage_date,
                    record_state,
                    absence,
                    removed_at_ms
                ],
            )
            .expect("daily");
    }
}
