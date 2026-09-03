use std::collections::HashMap;
use std::sync::Mutex;

use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use chrono_tz::Tz;
use rusqlite::{params, params_from_iter, ToSql};

use crate::application::collection::CollectionScope;
use crate::application::ports::antigravity_baseline_store::AntigravityBaselineStatus;
use crate::application::ports::antigravity_usage_cache::{
    AntigravityCalendarAttribution, AntigravityTimestampOrigin, AntigravityUsageCache,
    AntigravityUsageCacheError, AntigravityUsageCacheReconcileResult, AntigravityUsageCacheUpsert,
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
    fn reconcile(
        &self,
        records: &[AntigravityUsageCacheUpsert],
        collected_at: DateTime<Utc>,
    ) -> Result<AntigravityUsageCacheReconcileResult, AntigravityUsageCacheError> {
        if records.is_empty() {
            return Ok(AntigravityUsageCacheReconcileResult {
                records: Vec::new(),
                legacy_records_repaired: 0,
            });
        }

        let mut database = self
            .database
            .lock()
            .map_err(|_| AntigravityUsageCacheError::Storage)?;
        let now_ms = collected_at.timestamp_millis();
        let transaction = database
            .connection_mut()
            .transaction()
            .map_err(|_| AntigravityUsageCacheError::Storage)?;

        let mut resolved = Vec::with_capacity(records.len());
        let mut legacy_records_repaired = 0_u32;
        let mut baseline_statuses = HashMap::new();

        for entry in records {
            let record = &entry.record;
            let proposed_dedupe_key = dedupe_key(record);
            let legacy_key = legacy_dedupe_key(record);
            let existing = existing_timestamp(
                &transaction,
                &proposed_dedupe_key,
                legacy_key.as_deref(),
                record,
            )?;
            let dedupe_key = existing
                .as_ref()
                .map_or(proposed_dedupe_key, |value| value.dedupe_key.clone());

            let baseline_status = match baseline_statuses.get(record.variant.as_str()) {
                Some(&status) => status,
                None => {
                    let status = variant_baseline_status(&transaction, &record.variant)?;
                    baseline_statuses.insert(record.variant.clone(), status);
                    status
                }
            };

            let (observed_at, timestamp_origin, calendar_attribution, first_seen_at_ms) =
                resolve_timestamp_and_attribution(
                    record,
                    entry.legacy_fallback_at,
                    existing.as_ref(),
                    baseline_status,
                    collected_at,
                )?;

            if existing.as_ref().is_some_and(|existing| {
                existing.timestamp_origin == AntigravityTimestampOrigin::LegacyUnknown
                    && timestamp_origin != AntigravityTimestampOrigin::LegacyUnknown
            }) {
                legacy_records_repaired = legacy_records_repaired.saturating_add(1);
            }
            let observed_at_ms = observed_at.timestamp_millis();
            transaction
                .execute(
                    "INSERT INTO antigravity_usage_cache (
                        dedupe_key, variant, conversation_id, response_id, raw_model_id,
                        model_label, api_provider, input_tokens, output_tokens,
                        thinking_output_tokens, response_output_tokens, cache_read_tokens,
                        cache_write_tokens, observed_at_ms, collector_version,
                        first_seen_at_ms, last_seen_at_ms, source_record_index,
                        timestamp_origin, calendar_attribution
                    ) VALUES (
                        ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20
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
                        last_seen_at_ms = excluded.last_seen_at_ms,
                        source_record_index = COALESCE(
                            excluded.source_record_index,
                            antigravity_usage_cache.source_record_index
                        ),
                        timestamp_origin = excluded.timestamp_origin,
                        calendar_attribution = excluded.calendar_attribution",
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
                        first_seen_at_ms,
                        now_ms,
                        record.source_record_index,
                        timestamp_origin_value(timestamp_origin),
                        calendar_attribution.as_str(),
                    ],
                )
                .map_err(|_| AntigravityUsageCacheError::Storage)?;

            let mut canonical = record.clone();
            canonical.observed_at = Some(observed_at);
            canonical.timestamp_origin = timestamp_origin;
            canonical.calendar_attribution = calendar_attribution;
            resolved.push(canonical);
        }

        transaction
            .commit()
            .map_err(|_| AntigravityUsageCacheError::Storage)?;
        Ok(AntigravityUsageCacheReconcileResult {
            records: resolved,
            legacy_records_repaired,
        })
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
                    source_record_index, observed_at_ms, timestamp_origin, calendar_attribution
             FROM antigravity_usage_cache
             WHERE calendar_attribution = 'dated' AND observed_at_ms >= ?1 AND observed_at_ms < ?2 AND (",
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

    if let Some(index) = record.source_record_index {
        return format!(
            "{}:{}:idx:{}",
            record.variant, record.conversation_id, index
        );
    }

    legacy_dedupe_key(record).expect("token fallback always exists")
}

fn legacy_dedupe_key(record: &CachedAntigravityUsageRecord) -> Option<String> {
    record.response_id.is_none().then(|| {
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
    })
}

#[derive(Debug)]
struct ExistingTimestamp {
    dedupe_key: String,
    observed_at: DateTime<Utc>,
    timestamp_origin: AntigravityTimestampOrigin,
    calendar_attribution: AntigravityCalendarAttribution,
    first_seen_at_ms: i64,
}

fn existing_timestamp(
    transaction: &rusqlite::Transaction<'_>,
    dedupe_key: &str,
    legacy_key: Option<&str>,
    record: &CachedAntigravityUsageRecord,
) -> Result<Option<ExistingTimestamp>, AntigravityUsageCacheError> {
    use rusqlite::OptionalExtension;

    let row = transaction
        .query_row(
            "SELECT dedupe_key, observed_at_ms, timestamp_origin, calendar_attribution, first_seen_at_ms
             FROM antigravity_usage_cache
             WHERE dedupe_key = ?1
                OR (?2 IS NOT NULL AND dedupe_key = ?2)
                OR (
                    ?3 IS NOT NULL
                    AND variant = ?4
                    AND conversation_id = ?5
                    AND source_record_index = ?3
                )
             ORDER BY CASE WHEN dedupe_key = ?1 THEN 0 ELSE 1 END
             LIMIT 1",
            params![
                dedupe_key,
                legacy_key,
                record.source_record_index,
                record.variant,
                record.conversation_id,
            ],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            },
        )
        .optional()
        .map_err(|_| AntigravityUsageCacheError::Storage)?;

    row.map(
        |(dedupe_key, observed_at_ms, origin, attribution_str, first_seen_at_ms)| {
            Ok(ExistingTimestamp {
                dedupe_key,
                observed_at: DateTime::<Utc>::from_timestamp_millis(observed_at_ms)
                    .ok_or(AntigravityUsageCacheError::Storage)?,
                timestamp_origin: parse_timestamp_origin(&origin)?,
                calendar_attribution: AntigravityCalendarAttribution::from_str(&attribution_str)
                    .ok_or(AntigravityUsageCacheError::Storage)?,
                first_seen_at_ms,
            })
        },
    )
    .transpose()
}

fn variant_baseline_status(
    transaction: &rusqlite::Transaction<'_>,
    variant: &str,
) -> Result<AntigravityBaselineStatus, AntigravityUsageCacheError> {
    use rusqlite::OptionalExtension;
    let status_str: Option<String> = transaction
        .query_row(
            "SELECT status FROM antigravity_baseline_state WHERE variant = ?1",
            [variant],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| AntigravityUsageCacheError::Storage)?;

    match status_str.as_deref() {
        Some("complete") => Ok(AntigravityBaselineStatus::Complete),
        Some("pending") | None => Ok(AntigravityBaselineStatus::Pending),
        _ => Err(AntigravityUsageCacheError::Storage),
    }
}

fn resolve_timestamp_and_attribution(
    record: &CachedAntigravityUsageRecord,
    legacy_fallback_at: Option<DateTime<Utc>>,
    existing: Option<&ExistingTimestamp>,
    baseline_status: AntigravityBaselineStatus,
    collected_at: DateTime<Utc>,
) -> Result<
    (
        DateTime<Utc>,
        AntigravityTimestampOrigin,
        AntigravityCalendarAttribution,
        i64,
    ),
    AntigravityUsageCacheError,
> {
    let Some(existing) = existing else {
        let (observed_at, origin, attribution) = match record.timestamp_origin {
            AntigravityTimestampOrigin::SourceReported => (
                record
                    .observed_at
                    .ok_or(AntigravityUsageCacheError::Storage)?,
                AntigravityTimestampOrigin::SourceReported,
                AntigravityCalendarAttribution::Dated,
            ),
            AntigravityTimestampOrigin::FirstSeen => {
                let observed_at = record.observed_at.unwrap_or(collected_at);
                let attribution = match baseline_status {
                    AntigravityBaselineStatus::Pending => {
                        AntigravityCalendarAttribution::UndatedBaseline
                    }
                    AntigravityBaselineStatus::Complete => AntigravityCalendarAttribution::Dated,
                };
                (
                    observed_at,
                    AntigravityTimestampOrigin::FirstSeen,
                    attribution,
                )
            }
            AntigravityTimestampOrigin::LegacyUnknown => {
                let observed_at = record.observed_at.unwrap_or(collected_at);
                let attribution = match baseline_status {
                    AntigravityBaselineStatus::Pending => {
                        AntigravityCalendarAttribution::UndatedBaseline
                    }
                    AntigravityBaselineStatus::Complete => AntigravityCalendarAttribution::Dated,
                };
                (
                    observed_at,
                    AntigravityTimestampOrigin::LegacyUnknown,
                    attribution,
                )
            }
            AntigravityTimestampOrigin::Unresolved => {
                let observed_at = record.observed_at.unwrap_or(collected_at);
                let attribution = match baseline_status {
                    AntigravityBaselineStatus::Pending => {
                        AntigravityCalendarAttribution::UndatedBaseline
                    }
                    AntigravityBaselineStatus::Complete => AntigravityCalendarAttribution::Dated,
                };
                (
                    observed_at,
                    AntigravityTimestampOrigin::FirstSeen,
                    attribution,
                )
            }
        };
        return Ok((
            observed_at,
            origin,
            attribution,
            collected_at.timestamp_millis(),
        ));
    };

    if record.timestamp_origin == AntigravityTimestampOrigin::SourceReported {
        let observed_at = record
            .observed_at
            .ok_or(AntigravityUsageCacheError::Storage)?;
        return Ok((
            observed_at,
            AntigravityTimestampOrigin::SourceReported,
            AntigravityCalendarAttribution::Dated,
            existing.first_seen_at_ms,
        ));
    }

    let (observed_at, origin) = match existing.timestamp_origin {
        AntigravityTimestampOrigin::SourceReported | AntigravityTimestampOrigin::FirstSeen => {
            (existing.observed_at, existing.timestamp_origin)
        }
        AntigravityTimestampOrigin::LegacyUnknown => {
            if legacy_fallback_at == Some(existing.observed_at) {
                (
                    DateTime::<Utc>::from_timestamp_millis(existing.first_seen_at_ms)
                        .ok_or(AntigravityUsageCacheError::Storage)?,
                    AntigravityTimestampOrigin::FirstSeen,
                )
            } else {
                (
                    existing.observed_at,
                    AntigravityTimestampOrigin::LegacyUnknown,
                )
            }
        }
        AntigravityTimestampOrigin::Unresolved => return Err(AntigravityUsageCacheError::Storage),
    };

    Ok((
        observed_at,
        origin,
        existing.calendar_attribution,
        existing.first_seen_at_ms,
    ))
}

fn timestamp_origin_value(origin: AntigravityTimestampOrigin) -> &'static str {
    match origin {
        AntigravityTimestampOrigin::SourceReported => "source_reported",
        AntigravityTimestampOrigin::FirstSeen => "first_seen",
        AntigravityTimestampOrigin::LegacyUnknown => "legacy_unknown",
        AntigravityTimestampOrigin::Unresolved => "unresolved",
    }
}

fn parse_timestamp_origin(
    value: &str,
) -> Result<AntigravityTimestampOrigin, AntigravityUsageCacheError> {
    match value {
        "source_reported" => Ok(AntigravityTimestampOrigin::SourceReported),
        "first_seen" => Ok(AntigravityTimestampOrigin::FirstSeen),
        "legacy_unknown" => Ok(AntigravityTimestampOrigin::LegacyUnknown),
        _ => Err(AntigravityUsageCacheError::Storage),
    }
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
    let observed_at_ms: i64 = row.get(13)?;
    let timestamp_origin: String = row.get(14)?;
    let calendar_attribution: String = row.get(15)?;
    Ok(CachedAntigravityUsageRecord {
        variant: row.get(0)?,
        conversation_id: row.get(1)?,
        response_id: row.get(2)?,
        raw_model_id: row.get(3)?,
        model_label: row.get(4)?,
        api_provider: row.get(5)?,
        source_record_index: row.get(12)?,
        input_tokens: u64::try_from(row.get::<_, i64>(6)?).unwrap_or(0),
        output_tokens: u64::try_from(row.get::<_, i64>(7)?).unwrap_or(0),
        thinking_output_tokens: u64::try_from(row.get::<_, i64>(8)?).unwrap_or(0),
        response_output_tokens: u64::try_from(row.get::<_, i64>(9)?).unwrap_or(0),
        cache_read_tokens: u64::try_from(row.get::<_, i64>(10)?).unwrap_or(0),
        cache_write_tokens: u64::try_from(row.get::<_, i64>(11)?).unwrap_or(0),
        observed_at: DateTime::<Utc>::from_timestamp_millis(observed_at_ms),
        timestamp_origin: parse_timestamp_origin(&timestamp_origin)
            .map_err(|_| rusqlite::Error::InvalidQuery)?,
        calendar_attribution: AntigravityCalendarAttribution::from_str(&calendar_attribution)
            .ok_or(rusqlite::Error::InvalidQuery)?,
    })
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;
    use crate::application::collection::CollectionScope;
    use crate::infrastructure::database::test_database::TestDatabase;

    fn cache_record(
        response_id: &str,
        observed_at: Option<DateTime<Utc>>,
        timestamp_origin: AntigravityTimestampOrigin,
    ) -> CachedAntigravityUsageRecord {
        CachedAntigravityUsageRecord {
            variant: "antigravity-cli".to_owned(),
            conversation_id: "conversation-a".to_owned(),
            response_id: Some(response_id.to_owned()),
            raw_model_id: "gemini".to_owned(),
            model_label: "Gemini".to_owned(),
            api_provider: None,
            source_record_index: Some(0),
            input_tokens: 10,
            output_tokens: 2,
            thinking_output_tokens: 0,
            response_output_tokens: 2,
            cache_read_tokens: 3,
            cache_write_tokens: 0,
            observed_at,
            timestamp_origin,
            calendar_attribution: AntigravityCalendarAttribution::Dated,
        }
    }

    fn migrated_cache_store() -> (TestDatabase, SqliteAntigravityUsageCacheStore) {
        let mut test_database = TestDatabase::open();
        test_database
            .database_mut()
            .migrate_to_latest()
            .expect("migrate");
        let store = SqliteAntigravityUsageCacheStore::new(
            Database::open(test_database.path()).expect("open cache db"),
        );
        (test_database, store)
    }

    #[test]
    fn timestampless_record_uses_immutable_first_seen_time() {
        let (_database, store) = migrated_cache_store();
        let first_seen = Utc.with_ymd_and_hms(2026, 8, 22, 10, 0, 0).unwrap();
        let later_refresh = Utc.with_ymd_and_hms(2026, 8, 23, 10, 0, 0).unwrap();
        let upsert = AntigravityUsageCacheUpsert {
            record: cache_record(
                "response-first-seen",
                None,
                AntigravityTimestampOrigin::Unresolved,
            ),
            legacy_fallback_at: Some(Utc.with_ymd_and_hms(2026, 8, 17, 10, 0, 0).unwrap()),
            collector_version: "local-rpc".to_owned(),
        };

        let first = store
            .reconcile(std::slice::from_ref(&upsert), first_seen)
            .expect("first reconcile");
        let repeated = store
            .reconcile(std::slice::from_ref(&upsert), later_refresh)
            .expect("repeated reconcile");

        assert_eq!(first.records[0].observed_at, Some(first_seen));
        assert_eq!(repeated.records[0].observed_at, Some(first_seen));
        assert_eq!(
            repeated.records[0].timestamp_origin,
            AntigravityTimestampOrigin::FirstSeen
        );
    }

    #[test]
    fn repairs_provable_legacy_conversation_timestamp_to_original_first_seen() {
        let (_database, store) = migrated_cache_store();
        let conversation_created = Utc.with_ymd_and_hms(2026, 8, 17, 10, 0, 0).unwrap();
        let first_seen = Utc.with_ymd_and_hms(2026, 8, 22, 10, 0, 0).unwrap();
        let repair_time = Utc.with_ymd_and_hms(2026, 8, 22, 11, 0, 0).unwrap();

        store
            .reconcile(
                &[AntigravityUsageCacheUpsert {
                    record: cache_record(
                        "response-legacy",
                        Some(conversation_created),
                        AntigravityTimestampOrigin::LegacyUnknown,
                    ),
                    legacy_fallback_at: None,
                    collector_version: "local-rpc".to_owned(),
                }],
                first_seen,
            )
            .expect("seed legacy row");

        let repaired = store
            .reconcile(
                &[AntigravityUsageCacheUpsert {
                    record: cache_record(
                        "response-legacy",
                        None,
                        AntigravityTimestampOrigin::Unresolved,
                    ),
                    legacy_fallback_at: Some(conversation_created),
                    collector_version: "local-rpc".to_owned(),
                }],
                repair_time,
            )
            .expect("repair legacy row");

        assert_eq!(repaired.records[0].observed_at, Some(first_seen));
        assert_eq!(
            repaired.records[0].timestamp_origin,
            AntigravityTimestampOrigin::FirstSeen
        );
        assert_eq!(repaired.legacy_records_repaired, 1);
    }

    #[test]
    fn preserves_ambiguous_legacy_timestamp() {
        let (_database, store) = migrated_cache_store();
        let stored = Utc.with_ymd_and_hms(2026, 8, 16, 9, 0, 0).unwrap();
        let different_fallback = Utc.with_ymd_and_hms(2026, 8, 17, 10, 0, 0).unwrap();
        let first_seen = Utc.with_ymd_and_hms(2026, 8, 22, 10, 0, 0).unwrap();
        store
            .reconcile(
                &[AntigravityUsageCacheUpsert {
                    record: cache_record(
                        "response-ambiguous",
                        Some(stored),
                        AntigravityTimestampOrigin::LegacyUnknown,
                    ),
                    legacy_fallback_at: None,
                    collector_version: "local-rpc".to_owned(),
                }],
                first_seen,
            )
            .expect("seed legacy row");

        let result = store
            .reconcile(
                &[AntigravityUsageCacheUpsert {
                    record: cache_record(
                        "response-ambiguous",
                        None,
                        AntigravityTimestampOrigin::Unresolved,
                    ),
                    legacy_fallback_at: Some(different_fallback),
                    collector_version: "local-rpc".to_owned(),
                }],
                first_seen,
            )
            .expect("preserve ambiguous row");

        assert_eq!(result.records[0].observed_at, Some(stored));
        assert_eq!(
            result.records[0].timestamp_origin,
            AntigravityTimestampOrigin::LegacyUnknown
        );
        assert_eq!(result.legacy_records_repaired, 0);
    }

    #[test]
    fn source_record_index_keeps_legacy_response_less_rows_stable_when_tokens_change() {
        let (database, store) = migrated_cache_store();
        let legacy_fallback = Utc.with_ymd_and_hms(2026, 8, 17, 10, 0, 0).unwrap();
        let first_seen = Utc.with_ymd_and_hms(2026, 8, 22, 10, 0, 0).unwrap();
        let mut legacy_record = cache_record(
            "unused-response",
            Some(legacy_fallback),
            AntigravityTimestampOrigin::LegacyUnknown,
        );
        legacy_record.response_id = None;
        legacy_record.source_record_index = None;
        store
            .reconcile(
                &[AntigravityUsageCacheUpsert {
                    record: legacy_record,
                    legacy_fallback_at: None,
                    collector_version: "local-rpc".to_owned(),
                }],
                first_seen,
            )
            .expect("seed response-less legacy row");

        let mut indexed_record = cache_record(
            "unused-response",
            None,
            AntigravityTimestampOrigin::Unresolved,
        );
        indexed_record.response_id = None;
        let indexed_upsert = AntigravityUsageCacheUpsert {
            record: indexed_record.clone(),
            legacy_fallback_at: Some(legacy_fallback),
            collector_version: "local-rpc".to_owned(),
        };
        store
            .reconcile(std::slice::from_ref(&indexed_upsert), first_seen)
            .expect("attach source row index");

        indexed_record.output_tokens = 9;
        store
            .reconcile(
                &[AntigravityUsageCacheUpsert {
                    record: indexed_record,
                    ..indexed_upsert
                }],
                first_seen,
            )
            .expect("update by source row index");

        let stored: (i64, i64, Option<i64>) = database
            .database()
            .connection
            .query_row(
                "SELECT COUNT(*), output_tokens, source_record_index
                 FROM antigravity_usage_cache",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("read cache identity");
        assert_eq!(stored, (1, 9, Some(0)));
    }

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
            .reconcile(
                &[AntigravityUsageCacheUpsert {
                    record: CachedAntigravityUsageRecord {
                        variant: "antigravity".to_owned(),
                        conversation_id: "conversation-a".to_owned(),
                        response_id: Some("response-1".to_owned()),
                        raw_model_id: "MODEL_PLACEHOLDER_M16".to_owned(),
                        model_label: "Gemini Pro".to_owned(),
                        api_provider: Some("API_PROVIDER_GOOGLE_GEMINI".to_owned()),
                        source_record_index: Some(0),
                        input_tokens: 100,
                        output_tokens: 20,
                        thinking_output_tokens: 5,
                        response_output_tokens: 15,
                        cache_read_tokens: 3,
                        cache_write_tokens: 1,
                        observed_at: Some(observed_at),
                        timestamp_origin: AntigravityTimestampOrigin::SourceReported,
                        calendar_attribution: AntigravityCalendarAttribution::Dated,
                    },
                    legacy_fallback_at: None,
                    collector_version: "local-rpc".to_owned(),
                }],
                observed_at,
            )
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
                source_record_index: Some(0),
                input_tokens: 10,
                output_tokens: 2,
                thinking_output_tokens: 0,
                response_output_tokens: 2,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
                observed_at: Some(observed_at),
                timestamp_origin: AntigravityTimestampOrigin::SourceReported,
                calendar_attribution: AntigravityCalendarAttribution::Dated,
            },
            legacy_fallback_at: None,
            collector_version: "local-rpc".to_owned(),
        };

        store
            .reconcile(std::slice::from_ref(&upsert), observed_at)
            .expect("first upsert");
        store
            .reconcile(std::slice::from_ref(&upsert), observed_at)
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
            .reconcile(
                &[AntigravityUsageCacheUpsert {
                    record: CachedAntigravityUsageRecord {
                        variant: "antigravity".to_owned(),
                        conversation_id: "conversation-a".to_owned(),
                        response_id: Some("response-old".to_owned()),
                        raw_model_id: "MODEL_PLACEHOLDER_M16".to_owned(),
                        model_label: "Gemini Pro".to_owned(),
                        api_provider: None,
                        source_record_index: Some(0),
                        input_tokens: 10,
                        output_tokens: 2,
                        thinking_output_tokens: 0,
                        response_output_tokens: 2,
                        cache_read_tokens: 0,
                        cache_write_tokens: 0,
                        observed_at: Some(observed_at),
                        timestamp_origin: AntigravityTimestampOrigin::SourceReported,
                        calendar_attribution: AntigravityCalendarAttribution::Dated,
                    },
                    legacy_fallback_at: None,
                    collector_version: "local-rpc".to_owned(),
                }],
                observed_at,
            )
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

    #[test]
    fn baseline_attribution_logic_governs_dated_vs_undated_baseline() {
        let (database, store) = migrated_cache_store();
        let collected_at = Utc.with_ymd_and_hms(2026, 9, 3, 12, 0, 0).unwrap();
        let source_time = Utc.with_ymd_and_hms(2026, 8, 1, 10, 0, 0).unwrap();

        // 1. By default (no entry in antigravity_baseline_state), variant baseline status is Pending.
        // A new Unresolved record resolves to UndatedBaseline and FirstSeen.
        let mut unresolved_rec = cache_record(
            "resp-unresolved",
            None,
            AntigravityTimestampOrigin::Unresolved,
        );
        unresolved_rec.source_record_index = Some(1);
        let result = store
            .reconcile(
                &[AntigravityUsageCacheUpsert {
                    record: unresolved_rec,
                    legacy_fallback_at: None,
                    collector_version: "local-rpc".to_owned(),
                }],
                collected_at,
            )
            .expect("reconcile unresolved");
        assert_eq!(
            result.records[0].calendar_attribution,
            AntigravityCalendarAttribution::UndatedBaseline
        );
        assert_eq!(
            result.records[0].timestamp_origin,
            AntigravityTimestampOrigin::FirstSeen
        );

        // 2. A new LegacyUnknown record during Pending baseline resolves to UndatedBaseline.
        let mut legacy_rec = cache_record(
            "resp-legacy",
            Some(collected_at),
            AntigravityTimestampOrigin::LegacyUnknown,
        );
        legacy_rec.source_record_index = Some(2);
        let result = store
            .reconcile(
                &[AntigravityUsageCacheUpsert {
                    record: legacy_rec,
                    legacy_fallback_at: None,
                    collector_version: "local-rpc".to_owned(),
                }],
                collected_at,
            )
            .expect("reconcile legacy");
        assert_eq!(
            result.records[0].calendar_attribution,
            AntigravityCalendarAttribution::UndatedBaseline
        );
        assert_eq!(
            result.records[0].timestamp_origin,
            AntigravityTimestampOrigin::LegacyUnknown
        );

        // 3. A new SourceReported record during Pending baseline resolves to Dated.
        let mut source_rec = cache_record(
            "resp-source",
            Some(source_time),
            AntigravityTimestampOrigin::SourceReported,
        );
        source_rec.source_record_index = Some(3);
        let result = store
            .reconcile(
                &[AntigravityUsageCacheUpsert {
                    record: source_rec,
                    legacy_fallback_at: None,
                    collector_version: "local-rpc".to_owned(),
                }],
                collected_at,
            )
            .expect("reconcile source");
        assert_eq!(
            result.records[0].calendar_attribution,
            AntigravityCalendarAttribution::Dated
        );
        assert_eq!(
            result.records[0].timestamp_origin,
            AntigravityTimestampOrigin::SourceReported
        );

        // 4. Existing UndatedBaseline record re-scanned without source timestamp preserves UndatedBaseline.
        let mut unresolved_rescan = cache_record(
            "resp-unresolved",
            None,
            AntigravityTimestampOrigin::Unresolved,
        );
        unresolved_rescan.source_record_index = Some(1);
        let result = store
            .reconcile(
                &[AntigravityUsageCacheUpsert {
                    record: unresolved_rescan,
                    legacy_fallback_at: None,
                    collector_version: "local-rpc".to_owned(),
                }],
                collected_at,
            )
            .expect("rescan unresolved");
        assert_eq!(
            result.records[0].calendar_attribution,
            AntigravityCalendarAttribution::UndatedBaseline
        );

        // 5. Existing UndatedBaseline record re-scanned WITH genuine SourceReported timestamp upgrades to Dated.
        let mut upgraded_rec = cache_record(
            "resp-unresolved",
            Some(source_time),
            AntigravityTimestampOrigin::SourceReported,
        );
        upgraded_rec.source_record_index = Some(1);
        let result = store
            .reconcile(
                &[AntigravityUsageCacheUpsert {
                    record: upgraded_rec,
                    legacy_fallback_at: None,
                    collector_version: "local-rpc".to_owned(),
                }],
                collected_at,
            )
            .expect("upgrade unresolved to source reported");
        assert_eq!(
            result.records[0].calendar_attribution,
            AntigravityCalendarAttribution::Dated
        );
        assert_eq!(
            result.records[0].timestamp_origin,
            AntigravityTimestampOrigin::SourceReported
        );
        assert_eq!(result.records[0].observed_at, Some(source_time));

        // 6. Mark variant baseline as complete.
        database
            .database()
            .connection
            .execute(
                "INSERT INTO antigravity_baseline_state (
                    variant, status, started_at_ms, completed_at_ms, updated_at_ms
                ) VALUES ('antigravity-cli', 'complete', 1000, 2000, 2000)",
                [],
            )
            .expect("insert complete baseline");

        // Now, a new Unresolved record after Complete baseline resolves to Dated and FirstSeen.
        let mut post_baseline_unresolved = cache_record(
            "resp-post-baseline",
            None,
            AntigravityTimestampOrigin::Unresolved,
        );
        post_baseline_unresolved.source_record_index = Some(4);
        let result = store
            .reconcile(
                &[AntigravityUsageCacheUpsert {
                    record: post_baseline_unresolved,
                    legacy_fallback_at: None,
                    collector_version: "local-rpc".to_owned(),
                }],
                collected_at,
            )
            .expect("reconcile post-baseline unresolved");
        assert_eq!(
            result.records[0].calendar_attribution,
            AntigravityCalendarAttribution::Dated
        );
        assert_eq!(
            result.records[0].timestamp_origin,
            AntigravityTimestampOrigin::FirstSeen
        );

        // A new LegacyUnknown record after Complete baseline resolves to Dated.
        let mut post_baseline_legacy = cache_record(
            "resp-post-legacy",
            Some(collected_at),
            AntigravityTimestampOrigin::LegacyUnknown,
        );
        post_baseline_legacy.source_record_index = Some(5);
        let result = store
            .reconcile(
                &[AntigravityUsageCacheUpsert {
                    record: post_baseline_legacy,
                    legacy_fallback_at: None,
                    collector_version: "local-rpc".to_owned(),
                }],
                collected_at,
            )
            .expect("reconcile post-baseline legacy");
        assert_eq!(
            result.records[0].calendar_attribution,
            AntigravityCalendarAttribution::Dated
        );

        // 7. read_for_scope returns only Dated records, excluding UndatedBaseline.
        // We still have resp-legacy as UndatedBaseline!
        let scope_records = store
            .read_for_scope(
                &CollectionScope::Full,
                "UTC",
                &[("antigravity-cli", "conversation-a")],
            )
            .expect("read for scope");

        let response_ids: Vec<_> = scope_records
            .iter()
            .filter_map(|r| r.response_id.as_deref())
            .collect();
        assert!(response_ids.contains(&"resp-source"));
        assert!(response_ids.contains(&"resp-unresolved")); // upgraded to dated
        assert!(response_ids.contains(&"resp-post-baseline"));
        assert!(response_ids.contains(&"resp-post-legacy"));
        assert!(!response_ids.contains(&"resp-legacy")); // undated baseline excluded!

        for rec in &scope_records {
            assert_eq!(
                rec.calendar_attribution,
                AntigravityCalendarAttribution::Dated
            );
        }
    }
}
