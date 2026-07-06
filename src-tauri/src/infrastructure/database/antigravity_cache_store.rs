use std::sync::Mutex;

use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use chrono_tz::Tz;
use rusqlite::{params, params_from_iter, ToSql};

use crate::application::collection::CollectionScope;
use crate::application::ports::antigravity_usage_cache::{
    AntigravityUsageCache, AntigravityUsageCacheError, AntigravityUsageCacheUpsert,
    CachedAntigravityUsageRecord,
};

use super::Database;

pub(crate) struct SqliteAntigravityUsageCacheStore {
    database: Mutex<Database>,
}

impl SqliteAntigravityUsageCacheStore {
    pub(crate) fn new(database: Database) -> Self {
        Self {
            database: Mutex::new(database),
        }
    }
}

impl AntigravityUsageCache for SqliteAntigravityUsageCacheStore {
    fn upsert(
        &self,
        records: &[AntigravityUsageCacheUpsert],
    ) -> Result<(), AntigravityUsageCacheError> {
        if records.is_empty() {
            return Ok(());
        }

        let mut database = self
            .database
            .lock()
            .map_err(|_| AntigravityUsageCacheError::Storage)?;
        let now_ms = Utc::now().timestamp_millis();
        let transaction = database
            .connection_mut()
            .transaction()
            .map_err(|_| AntigravityUsageCacheError::Storage)?;

        for entry in records {
            let record = &entry.record;
            let dedupe_key = dedupe_key(record);
            let observed_at_ms = record.observed_at.timestamp_millis();
            transaction
                .execute(
                    "INSERT INTO antigravity_usage_cache (
                        dedupe_key, variant, conversation_id, response_id, raw_model_id,
                        model_label, api_provider, input_tokens, output_tokens,
                        thinking_output_tokens, response_output_tokens, cache_read_tokens,
                        cache_write_tokens, observed_at_ms, collector_version,
                        first_seen_at_ms, last_seen_at_ms
                    ) VALUES (
                        ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17
                    )
                    ON CONFLICT(dedupe_key) DO UPDATE SET
                        model_label = excluded.model_label,
                        api_provider = excluded.api_provider,
                        input_tokens = excluded.input_tokens,
                        output_tokens = excluded.output_tokens,
                        thinking_output_tokens = excluded.thinking_output_tokens,
                        response_output_tokens = excluded.response_output_tokens,
                        cache_read_tokens = excluded.cache_read_tokens,
                        cache_write_tokens = excluded.cache_write_tokens,
                        observed_at_ms = excluded.observed_at_ms,
                        collector_version = excluded.collector_version,
                        last_seen_at_ms = excluded.last_seen_at_ms",
                    params![
                        dedupe_key,
                        record.variant,
                        record.conversation_id,
                        record.response_id,
                        record.raw_model_id,
                        record.model_label,
                        record.api_provider,
                        i64::try_from(record.input_tokens).unwrap_or(i64::MAX),
                        i64::try_from(record.output_tokens).unwrap_or(i64::MAX),
                        i64::try_from(record.thinking_output_tokens).unwrap_or(i64::MAX),
                        i64::try_from(record.response_output_tokens).unwrap_or(i64::MAX),
                        i64::try_from(record.cache_read_tokens).unwrap_or(i64::MAX),
                        i64::try_from(record.cache_write_tokens).unwrap_or(i64::MAX),
                        observed_at_ms,
                        entry.collector_version,
                        now_ms,
                        now_ms,
                    ],
                )
                .map_err(|_| AntigravityUsageCacheError::Storage)?;
        }

        transaction
            .commit()
            .map_err(|_| AntigravityUsageCacheError::Storage)?;
        Ok(())
    }

    fn read_for_scope(
        &self,
        scope: &CollectionScope,
        aggregation_timezone: &str,
        conversations: &[(&str, &str)],
    ) -> Result<Vec<CachedAntigravityUsageRecord>, AntigravityUsageCacheError> {
        if conversations.is_empty() {
            return Ok(Vec::new());
        }

        let timezone = aggregation_timezone
            .parse::<Tz>()
            .map_err(|_| AntigravityUsageCacheError::InvalidScope)?;
        let (start_ms, end_ms) = scope_bounds(scope, timezone)?;

        let mut database = self
            .database
            .lock()
            .map_err(|_| AntigravityUsageCacheError::Storage)?;

        let mut query = String::from(
            "SELECT variant, conversation_id, response_id, raw_model_id, model_label,
                    api_provider, input_tokens, output_tokens, thinking_output_tokens,
                    response_output_tokens, cache_read_tokens, cache_write_tokens,
                    observed_at_ms
             FROM antigravity_usage_cache
             WHERE observed_at_ms >= ?1 AND observed_at_ms < ?2 AND (",
        );
        let mut params: Vec<Box<dyn ToSql>> = vec![Box::new(start_ms), Box::new(end_ms)];
        for (index, (variant, conversation_id)) in conversations.iter().enumerate() {
            if index > 0 {
                query.push_str(" OR ");
            }
            query.push_str("(variant = ? AND conversation_id = ?)");
            params.push(Box::new(*variant));
            params.push(Box::new(*conversation_id));
        }
        query.push_str(") ORDER BY observed_at_ms ASC, id ASC");

        let mut statement = database
            .connection_mut()
            .prepare(&query)
            .map_err(|_| AntigravityUsageCacheError::Storage)?;
        let rows = statement
            .query_map(params_from_iter(params.iter()), map_cached_row)
            .map_err(|_| AntigravityUsageCacheError::Storage)?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|_| AntigravityUsageCacheError::Storage)
    }
}

fn dedupe_key(record: &CachedAntigravityUsageRecord) -> String {
    if let Some(response_id) = record
        .response_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return format!(
            "{}:{}:{}",
            record.variant, record.conversation_id, response_id
        );
    }

    format!(
        "{}:{}:{}:{}:{}:{}:{}",
        record.variant,
        record.conversation_id,
        record.raw_model_id,
        record.input_tokens,
        record.output_tokens,
        record.thinking_output_tokens,
        record.cache_read_tokens
    )
}

fn scope_bounds(
    scope: &CollectionScope,
    timezone: Tz,
) -> Result<(i64, i64), AntigravityUsageCacheError> {
    match scope {
        CollectionScope::Full => Ok((0, i64::MAX)),
        CollectionScope::Incremental(scope) => {
            let start = local_midnight(timezone, scope.start_date())?;
            let end_date = scope
                .end_date()
                .succ_opt()
                .ok_or(AntigravityUsageCacheError::InvalidScope)?;
            let end = local_midnight(timezone, end_date)?;
            Ok((start.timestamp_millis(), end.timestamp_millis()))
        }
    }
}

fn local_midnight(
    timezone: Tz,
    date: NaiveDate,
) -> Result<DateTime<Utc>, AntigravityUsageCacheError> {
    timezone
        .from_local_datetime(&date.and_hms_opt(0, 0, 0).expect("midnight"))
        .single()
        .map(|value| value.with_timezone(&Utc))
        .ok_or(AntigravityUsageCacheError::InvalidScope)
}

fn map_cached_row(
    row: &rusqlite::Row<'_>,
) -> Result<CachedAntigravityUsageRecord, rusqlite::Error> {
    let observed_at_ms: i64 = row.get(12)?;
    Ok(CachedAntigravityUsageRecord {
        variant: row.get(0)?,
        conversation_id: row.get(1)?,
        response_id: row.get(2)?,
        raw_model_id: row.get(3)?,
        model_label: row.get(4)?,
        api_provider: row.get(5)?,
        input_tokens: u64::try_from(row.get::<_, i64>(6)?).unwrap_or(0),
        output_tokens: u64::try_from(row.get::<_, i64>(7)?).unwrap_or(0),
        thinking_output_tokens: u64::try_from(row.get::<_, i64>(8)?).unwrap_or(0),
        response_output_tokens: u64::try_from(row.get::<_, i64>(9)?).unwrap_or(0),
        cache_read_tokens: u64::try_from(row.get::<_, i64>(10)?).unwrap_or(0),
        cache_write_tokens: u64::try_from(row.get::<_, i64>(11)?).unwrap_or(0),
        observed_at: DateTime::<Utc>::from_timestamp_millis(observed_at_ms)
            .unwrap_or_else(Utc::now),
    })
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;
    use crate::application::collection::CollectionScope;
    use crate::infrastructure::database::test_database::TestDatabase;

    #[test]
    fn upsert_and_read_records_for_refresh_window() {
        let mut test_database = TestDatabase::open();
        test_database
            .database_mut()
            .migrate_to_latest()
            .expect("migrate");
        let path = test_database.path().to_path_buf();
        let store =
            SqliteAntigravityUsageCacheStore::new(Database::open(&path).expect("open cache db"));

        let observed_at = Utc.with_ymd_and_hms(2026, 7, 2, 10, 0, 0).unwrap();
        store
            .upsert(&[AntigravityUsageCacheUpsert {
                record: CachedAntigravityUsageRecord {
                    variant: "antigravity".to_owned(),
                    conversation_id: "conversation-a".to_owned(),
                    response_id: Some("response-1".to_owned()),
                    raw_model_id: "MODEL_PLACEHOLDER_M16".to_owned(),
                    model_label: "Gemini Pro".to_owned(),
                    api_provider: Some("API_PROVIDER_GOOGLE_GEMINI".to_owned()),
                    input_tokens: 100,
                    output_tokens: 20,
                    thinking_output_tokens: 5,
                    response_output_tokens: 15,
                    cache_read_tokens: 3,
                    cache_write_tokens: 1,
                    observed_at,
                },
                collector_version: "local-rpc".to_owned(),
            }])
            .expect("upsert");

        let records = store
            .read_for_scope(
                &CollectionScope::incremental(observed_at.date_naive(), observed_at.date_naive())
                    .expect("scope"),
                "UTC",
                &[("antigravity", "conversation-a")],
            )
            .expect("read");

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].response_id.as_deref(), Some("response-1"));
    }

    #[test]
    fn repeated_upsert_with_same_response_id_is_idempotent() {
        let mut test_database = TestDatabase::open();
        test_database
            .database_mut()
            .migrate_to_latest()
            .expect("migrate");
        let path = test_database.path().to_path_buf();
        let store =
            SqliteAntigravityUsageCacheStore::new(Database::open(&path).expect("open cache db"));
        let observed_at = Utc.with_ymd_and_hms(2026, 7, 2, 10, 0, 0).unwrap();
        let upsert = AntigravityUsageCacheUpsert {
            record: CachedAntigravityUsageRecord {
                variant: "antigravity".to_owned(),
                conversation_id: "conversation-a".to_owned(),
                response_id: Some("response-1".to_owned()),
                raw_model_id: "MODEL_PLACEHOLDER_M16".to_owned(),
                model_label: "Gemini Pro".to_owned(),
                api_provider: None,
                input_tokens: 10,
                output_tokens: 2,
                thinking_output_tokens: 0,
                response_output_tokens: 2,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
                observed_at,
            },
            collector_version: "local-rpc".to_owned(),
        };

        store
            .upsert(std::slice::from_ref(&upsert))
            .expect("first upsert");
        store
            .upsert(std::slice::from_ref(&upsert))
            .expect("second upsert");

        let records = store
            .read_for_scope(
                &CollectionScope::Full,
                "UTC",
                &[("antigravity", "conversation-a")],
            )
            .expect("read");
        assert_eq!(records.len(), 1);
    }

    #[test]
    fn stale_records_outside_incremental_window_are_not_returned() {
        let mut test_database = TestDatabase::open();
        test_database
            .database_mut()
            .migrate_to_latest()
            .expect("migrate");
        let path = test_database.path().to_path_buf();
        let store =
            SqliteAntigravityUsageCacheStore::new(Database::open(&path).expect("open cache db"));
        let observed_at = Utc.with_ymd_and_hms(2026, 7, 1, 10, 0, 0).unwrap();
        store
            .upsert(&[AntigravityUsageCacheUpsert {
                record: CachedAntigravityUsageRecord {
                    variant: "antigravity".to_owned(),
                    conversation_id: "conversation-a".to_owned(),
                    response_id: Some("response-old".to_owned()),
                    raw_model_id: "MODEL_PLACEHOLDER_M16".to_owned(),
                    model_label: "Gemini Pro".to_owned(),
                    api_provider: None,
                    input_tokens: 10,
                    output_tokens: 2,
                    thinking_output_tokens: 0,
                    response_output_tokens: 2,
                    cache_read_tokens: 0,
                    cache_write_tokens: 0,
                    observed_at,
                },
                collector_version: "local-rpc".to_owned(),
            }])
            .expect("upsert");

        let records = store
            .read_for_scope(
                &CollectionScope::incremental(
                    chrono::NaiveDate::from_ymd_opt(2026, 7, 2).expect("date"),
                    chrono::NaiveDate::from_ymd_opt(2026, 7, 2).expect("date"),
                )
                .expect("scope"),
                "UTC",
                &[("antigravity", "conversation-a")],
            )
            .expect("read");

        assert!(records.is_empty());
    }
}
