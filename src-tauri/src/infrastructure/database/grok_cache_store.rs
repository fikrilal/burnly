use std::sync::Mutex;

use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use chrono_tz::Tz;
use rusqlite::{params, params_from_iter, ToSql};

use crate::application::collection::CollectionScope;
use crate::application::ports::grok_usage_cache::{
    CachedGrokUsageRecord, GrokUnifiedLogCheckpoint, GrokUsageCache, GrokUsageCacheError,
    GrokUsageCacheUpsert,
};

use super::Database;

pub(crate) struct SqliteGrokUsageCacheStore {
    database: Mutex<Database>,
}

impl SqliteGrokUsageCacheStore {
    pub(crate) fn new(database: Database) -> Self {
        Self {
            database: Mutex::new(database),
        }
    }
}

impl GrokUsageCache for SqliteGrokUsageCacheStore {
    fn upsert(&self, records: &[GrokUsageCacheUpsert]) -> Result<(), GrokUsageCacheError> {
        if records.is_empty() {
            return Ok(());
        }

        let mut database = self
            .database
            .lock()
            .map_err(|_| GrokUsageCacheError::Storage)?;
        let now_ms = Utc::now().timestamp_millis();
        let transaction = database
            .connection_mut()
            .transaction()
            .map_err(|_| GrokUsageCacheError::Storage)?;

        for entry in records {
            let record = &entry.record;
            let dedupe_key = dedupe_key(record);
            let observed_at_ms = record.observed_at.timestamp_millis();
            transaction
                .execute(
                    "INSERT INTO grok_usage_cache (
                        dedupe_key, session_id, observed_at_ms, loop_index, pid,
                        raw_model_id, model_display_name, project_path, prompt_tokens,
                        cached_prompt_tokens, completion_tokens, reasoning_tokens,
                        log_offset, collector_version, first_seen_at_ms, last_seen_at_ms
                    ) VALUES (
                        ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16
                    )
                    ON CONFLICT(dedupe_key) DO UPDATE SET
                        raw_model_id = excluded.raw_model_id,
                        model_display_name = excluded.model_display_name,
                        project_path = excluded.project_path,
                        prompt_tokens = excluded.prompt_tokens,
                        cached_prompt_tokens = excluded.cached_prompt_tokens,
                        completion_tokens = excluded.completion_tokens,
                        reasoning_tokens = excluded.reasoning_tokens,
                        log_offset = excluded.log_offset,
                        collector_version = excluded.collector_version,
                        last_seen_at_ms = excluded.last_seen_at_ms",
                    params![
                        dedupe_key,
                        record.session_id,
                        observed_at_ms,
                        i64::from(record.loop_index),
                        i64::try_from(record.pid).unwrap_or(i64::MAX),
                        record.raw_model_id,
                        record.model_display_name,
                        record.project_path,
                        i64::try_from(record.prompt_tokens).unwrap_or(i64::MAX),
                        i64::try_from(record.cached_prompt_tokens).unwrap_or(i64::MAX),
                        i64::try_from(record.completion_tokens).unwrap_or(i64::MAX),
                        i64::try_from(record.reasoning_tokens).unwrap_or(i64::MAX),
                        i64::try_from(entry.log_offset).unwrap_or(i64::MAX),
                        entry.collector_version,
                        now_ms,
                        now_ms,
                    ],
                )
                .map_err(|_| GrokUsageCacheError::Storage)?;
        }

        transaction
            .commit()
            .map_err(|_| GrokUsageCacheError::Storage)?;
        Ok(())
    }

    fn read_for_scope(
        &self,
        scope: &CollectionScope,
        aggregation_timezone: &str,
        session_ids: &[&str],
    ) -> Result<Vec<CachedGrokUsageRecord>, GrokUsageCacheError> {
        let timezone = aggregation_timezone
            .parse::<Tz>()
            .map_err(|_| GrokUsageCacheError::InvalidScope)?;
        let (start_ms, end_ms) = scope_bounds(scope, timezone)?;

        let mut database = self
            .database
            .lock()
            .map_err(|_| GrokUsageCacheError::Storage)?;

        let mut query = String::from(
            "SELECT session_id, observed_at_ms, loop_index, pid, raw_model_id,
                    model_display_name, project_path, prompt_tokens,
                    cached_prompt_tokens, completion_tokens, reasoning_tokens
             FROM grok_usage_cache
             WHERE observed_at_ms >= ?1 AND observed_at_ms < ?2",
        );
        let mut sql_params: Vec<Box<dyn ToSql>> = vec![Box::new(start_ms), Box::new(end_ms)];

        if !session_ids.is_empty() {
            query.push_str(" AND (");
            for (index, session_id) in session_ids.iter().enumerate() {
                if index > 0 {
                    query.push_str(" OR ");
                }
                query.push_str("session_id = ?");
                sql_params.push(Box::new(*session_id));
            }
            query.push(')');
        }

        query.push_str(" ORDER BY observed_at_ms ASC, id ASC");

        let mut statement = database
            .connection_mut()
            .prepare(&query)
            .map_err(|_| GrokUsageCacheError::Storage)?;
        let rows = statement
            .query_map(params_from_iter(sql_params.iter()), map_cached_row)
            .map_err(|_| GrokUsageCacheError::Storage)?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|_| GrokUsageCacheError::Storage)
    }

    fn read_checkpoint(&self) -> Result<Option<GrokUnifiedLogCheckpoint>, GrokUsageCacheError> {
        let mut database = self
            .database
            .lock()
            .map_err(|_| GrokUsageCacheError::Storage)?;
        let mut statement = database
            .connection_mut()
            .prepare(
                "SELECT file_inode, file_size, byte_offset
                 FROM grok_unified_log_checkpoint
                 WHERE id = 1",
            )
            .map_err(|_| GrokUsageCacheError::Storage)?;

        let mut rows = statement
            .query([])
            .map_err(|_| GrokUsageCacheError::Storage)?;
        let Some(row) = rows.next().map_err(|_| GrokUsageCacheError::Storage)? else {
            return Ok(None);
        };

        let inode: Option<i64> = row.get(0).map_err(|_| GrokUsageCacheError::Storage)?;
        let file_size: i64 = row.get(1).map_err(|_| GrokUsageCacheError::Storage)?;
        let byte_offset: i64 = row.get(2).map_err(|_| GrokUsageCacheError::Storage)?;
        Ok(Some(GrokUnifiedLogCheckpoint {
            file_inode: inode.and_then(|value| u64::try_from(value).ok()),
            file_size: u64::try_from(file_size).unwrap_or(0),
            byte_offset: u64::try_from(byte_offset).unwrap_or(0),
        }))
    }

    fn write_checkpoint(
        &self,
        checkpoint: GrokUnifiedLogCheckpoint,
    ) -> Result<(), GrokUsageCacheError> {
        let mut database = self
            .database
            .lock()
            .map_err(|_| GrokUsageCacheError::Storage)?;
        let now_ms = Utc::now().timestamp_millis();
        let inode = checkpoint
            .file_inode
            .and_then(|value| i64::try_from(value).ok());
        database
            .connection_mut()
            .execute(
                "INSERT INTO grok_unified_log_checkpoint (
                    id, file_inode, file_size, byte_offset, updated_at_ms
                ) VALUES (1, ?1, ?2, ?3, ?4)
                ON CONFLICT(id) DO UPDATE SET
                    file_inode = excluded.file_inode,
                    file_size = excluded.file_size,
                    byte_offset = excluded.byte_offset,
                    updated_at_ms = excluded.updated_at_ms",
                params![
                    inode,
                    i64::try_from(checkpoint.file_size).unwrap_or(i64::MAX),
                    i64::try_from(checkpoint.byte_offset).unwrap_or(i64::MAX),
                    now_ms,
                ],
            )
            .map_err(|_| GrokUsageCacheError::Storage)?;
        Ok(())
    }
}

fn dedupe_key(record: &CachedGrokUsageRecord) -> String {
    format!(
        "{}:{}:{}:{}:{}:{}",
        record.session_id,
        record.observed_at.timestamp_millis(),
        record.loop_index,
        record.prompt_tokens,
        record.completion_tokens,
        record.pid
    )
}

fn scope_bounds(scope: &CollectionScope, timezone: Tz) -> Result<(i64, i64), GrokUsageCacheError> {
    match scope {
        CollectionScope::Full => Ok((0, i64::MAX)),
        CollectionScope::Incremental(scope) => {
            let start = local_midnight(timezone, scope.start_date())?;
            let end_date = scope
                .end_date()
                .succ_opt()
                .ok_or(GrokUsageCacheError::InvalidScope)?;
            let end = local_midnight(timezone, end_date)?;
            Ok((start.timestamp_millis(), end.timestamp_millis()))
        }
    }
}

fn local_midnight(timezone: Tz, date: NaiveDate) -> Result<DateTime<Utc>, GrokUsageCacheError> {
    timezone
        .from_local_datetime(&date.and_hms_opt(0, 0, 0).expect("midnight"))
        .single()
        .map(|value| value.with_timezone(&Utc))
        .ok_or(GrokUsageCacheError::InvalidScope)
}

fn map_cached_row(row: &rusqlite::Row<'_>) -> Result<CachedGrokUsageRecord, rusqlite::Error> {
    let observed_at_ms: i64 = row.get(1)?;
    Ok(CachedGrokUsageRecord {
        session_id: row.get(0)?,
        observed_at: DateTime::<Utc>::from_timestamp_millis(observed_at_ms)
            .unwrap_or_else(Utc::now),
        loop_index: u32::try_from(row.get::<_, i64>(2)?).unwrap_or(u32::MAX),
        pid: u64::try_from(row.get::<_, i64>(3)?).unwrap_or(0),
        raw_model_id: row.get(4)?,
        model_display_name: row.get(5)?,
        project_path: row.get(6)?,
        prompt_tokens: u64::try_from(row.get::<_, i64>(7)?).unwrap_or(0),
        cached_prompt_tokens: u64::try_from(row.get::<_, i64>(8)?).unwrap_or(0),
        completion_tokens: u64::try_from(row.get::<_, i64>(9)?).unwrap_or(0),
        reasoning_tokens: u64::try_from(row.get::<_, i64>(10)?).unwrap_or(0),
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
        let store = SqliteGrokUsageCacheStore::new(Database::open(&path).expect("open cache db"));
        let observed_at = Utc.with_ymd_and_hms(2026, 7, 6, 10, 0, 0).unwrap();

        store
            .upsert(&[sample_upsert(observed_at, "session-a", 1)])
            .expect("upsert");

        let records = store
            .read_for_scope(
                &CollectionScope::incremental(observed_at.date_naive(), observed_at.date_naive())
                    .expect("scope"),
                "UTC",
                &["session-a"],
            )
            .expect("read");

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].session_id, "session-a");
        assert_eq!(records[0].prompt_tokens, 12000);
    }

    #[test]
    fn repeated_upsert_with_same_inference_key_is_idempotent() {
        let mut test_database = TestDatabase::open();
        test_database
            .database_mut()
            .migrate_to_latest()
            .expect("migrate");
        let path = test_database.path().to_path_buf();
        let store = SqliteGrokUsageCacheStore::new(Database::open(&path).expect("open cache db"));
        let observed_at = Utc.with_ymd_and_hms(2026, 7, 6, 10, 0, 0).unwrap();
        let upsert = sample_upsert(observed_at, "session-a", 1);

        store
            .upsert(std::slice::from_ref(&upsert))
            .expect("first upsert");
        store
            .upsert(std::slice::from_ref(&upsert))
            .expect("second upsert");

        let records = store
            .read_for_scope(&CollectionScope::Full, "UTC", &["session-a"])
            .expect("read");
        assert_eq!(records.len(), 1);
    }

    #[test]
    fn persists_and_reads_unified_log_checkpoint() {
        let mut test_database = TestDatabase::open();
        test_database
            .database_mut()
            .migrate_to_latest()
            .expect("migrate");
        let path = test_database.path().to_path_buf();
        let store = SqliteGrokUsageCacheStore::new(Database::open(&path).expect("open cache db"));

        store
            .write_checkpoint(GrokUnifiedLogCheckpoint {
                file_inode: Some(42),
                file_size: 10_000,
                byte_offset: 10_000,
            })
            .expect("write checkpoint");

        let checkpoint = store
            .read_checkpoint()
            .expect("read checkpoint")
            .expect("checkpoint");
        assert_eq!(checkpoint.file_inode, Some(42));
        assert_eq!(checkpoint.file_size, 10_000);
        assert_eq!(checkpoint.byte_offset, 10_000);
    }

    fn sample_upsert(
        observed_at: DateTime<Utc>,
        session_id: &str,
        loop_index: u32,
    ) -> GrokUsageCacheUpsert {
        GrokUsageCacheUpsert {
            record: CachedGrokUsageRecord {
                session_id: session_id.to_owned(),
                observed_at,
                loop_index,
                pid: 1001,
                raw_model_id: "grok-composer-2.5-fast".to_owned(),
                model_display_name: Some("Composer 2.5".to_owned()),
                project_path: Some("/tmp/project".to_owned()),
                prompt_tokens: 12000,
                cached_prompt_tokens: 8000,
                completion_tokens: 240,
                reasoning_tokens: 0,
            },
            collector_version: "local".to_owned(),
            log_offset: 512,
        }
    }
}
