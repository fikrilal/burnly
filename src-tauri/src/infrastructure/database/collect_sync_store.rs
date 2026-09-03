//! SQLite durable collect-sync state and outbox adapter.

#![allow(
    dead_code,
    reason = "Constructed by collect-sync composition in later chunks"
)]

use std::sync::Mutex;

use rusqlite::{params, Connection, OptionalExtension};

use crate::application::collect_sync::{
    merge_upload_scopes, PreparedBatch, StoredUploadScope, UploadScope, WireUploadScope,
};
use crate::application::ports::collect_sync_store::{
    BaselineStatus, CollectSyncAccountKey, CollectSyncState, CollectSyncStore,
    CollectSyncStoreError, CreateGenerationInput, CreateGenerationResult, OutboxBatch,
    OutboxBatchStatus,
};

use super::Database;

pub(crate) struct SqliteCollectSyncStore {
    database: Mutex<Database>,
}

impl SqliteCollectSyncStore {
    pub(crate) fn new(database: Database) -> Self {
        Self {
            database: Mutex::new(database),
        }
    }
}

impl CollectSyncStore for SqliteCollectSyncStore {
    fn load_state(
        &self,
        account: &CollectSyncAccountKey,
    ) -> Result<Option<CollectSyncState>, CollectSyncStoreError> {
        let database = self
            .database
            .lock()
            .map_err(|_| CollectSyncStoreError::Backend)?;
        load_state(database.connection(), account)
    }

    fn ensure_state(
        &self,
        account: &CollectSyncAccountKey,
        now_ms: i64,
    ) -> Result<CollectSyncState, CollectSyncStoreError> {
        let mut database = self
            .database
            .lock()
            .map_err(|_| CollectSyncStoreError::Backend)?;
        let transaction = database
            .connection_mut()
            .transaction()
            .map_err(|_| CollectSyncStoreError::Backend)?;
        let state = ensure_state(&transaction, account, now_ms)?;
        transaction
            .commit()
            .map_err(|_| CollectSyncStoreError::Backend)?;
        Ok(state)
    }

    fn merge_pending_scope(
        &self,
        account: &CollectSyncAccountKey,
        scope: UploadScope,
        now_ms: i64,
    ) -> Result<UploadScope, CollectSyncStoreError> {
        let mut database = self
            .database
            .lock()
            .map_err(|_| CollectSyncStoreError::Backend)?;
        let transaction = database
            .connection_mut()
            .transaction()
            .map_err(|_| CollectSyncStoreError::Backend)?;
        ensure_state(&transaction, account, now_ms)?;
        let current = load_state(&transaction, account)?.ok_or(CollectSyncStoreError::NotFound)?;
        let merged = merge_upload_scopes(current.pending_scope, scope);
        let pending_json = serde_json::to_string(&StoredUploadScope::from(&merged))
            .map_err(|_| CollectSyncStoreError::Backend)?;
        transaction
            .execute(
                "UPDATE collect_sync_state
                 SET pending_scope_json = ?1, updated_at_ms = ?2
                 WHERE user_id = ?3 AND client_device_id = ?4",
                params![
                    pending_json,
                    now_ms,
                    account.user_id,
                    account.client_device_id
                ],
            )
            .map_err(|_| CollectSyncStoreError::Backend)?;
        transaction
            .commit()
            .map_err(|_| CollectSyncStoreError::Backend)?;
        Ok(merged)
    }

    fn merge_pending_scope_for_all_accounts(
        &self,
        scope: &UploadScope,
        now_ms: i64,
    ) -> Result<usize, CollectSyncStoreError> {
        let mut database = self
            .database
            .lock()
            .map_err(|_| CollectSyncStoreError::Backend)?;
        let transaction = database
            .connection_mut()
            .transaction()
            .map_err(|_| CollectSyncStoreError::Backend)?;

        let mut statement = transaction
            .prepare(
                "SELECT user_id, client_device_id, pending_scope_json
                 FROM collect_sync_state",
            )
            .map_err(|_| CollectSyncStoreError::Backend)?;

        let accounts: Vec<(String, String, Option<String>)> = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })
            .map_err(|_| CollectSyncStoreError::Backend)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| CollectSyncStoreError::Backend)?;
        drop(statement);

        let mut updated = 0;
        for (user_id, client_device_id, pending_json) in accounts {
            let current_scope = pending_json
                .as_deref()
                .and_then(|raw| serde_json::from_str::<StoredUploadScope>(raw).ok())
                .and_then(|stored| UploadScope::try_from(stored).ok());

            let merged = merge_upload_scopes(current_scope, scope.clone());
            let updated_pending_json = serde_json::to_string(&StoredUploadScope::from(&merged))
                .map_err(|_| CollectSyncStoreError::Backend)?;

            transaction
                .execute(
                    "UPDATE collect_sync_state
                     SET pending_scope_json = ?1, updated_at_ms = ?2
                     WHERE user_id = ?3 AND client_device_id = ?4",
                    params![updated_pending_json, now_ms, user_id, client_device_id],
                )
                .map_err(|_| CollectSyncStoreError::Backend)?;
            updated += 1;
        }

        transaction
            .commit()
            .map_err(|_| CollectSyncStoreError::Backend)?;
        Ok(updated)
    }

    fn create_generation(
        &self,
        input: CreateGenerationInput,
    ) -> Result<CreateGenerationResult, CollectSyncStoreError> {
        let mut database = self
            .database
            .lock()
            .map_err(|_| CollectSyncStoreError::Backend)?;
        let transaction = database
            .connection_mut()
            .transaction()
            .map_err(|_| CollectSyncStoreError::Backend)?;

        ensure_state(&transaction, &input.account, input.now_ms)?;
        if count_pending_batches(&transaction, &input.account)? > 0 {
            return Err(CollectSyncStoreError::PendingGenerationExists);
        }

        let state =
            load_state(&transaction, &input.account)?.ok_or(CollectSyncStoreError::NotFound)?;
        validate_prepared_revisions(state.next_client_revision, &input.prepared_batches)?;

        let batch_count = input.prepared_batches.len();
        for batch in &input.prepared_batches {
            insert_outbox_batch(
                &transaction,
                &input.account,
                &input.generation_id,
                batch,
                input.now_ms,
            )?;
        }

        let next_revision = state
            .next_client_revision
            .checked_add(i64::try_from(batch_count).map_err(|_| CollectSyncStoreError::Backend)?)
            .ok_or(CollectSyncStoreError::Backend)?;

        let baseline_status = if input.marks_baseline_in_progress {
            BaselineStatus::InProgress
        } else {
            state.baseline_status
        };

        let active_generation_id = if batch_count == 0 {
            None
        } else {
            Some(input.generation_id.clone())
        };

        let pending_scope_json = if input.clear_pending_scope {
            None
        } else {
            state
                .pending_scope
                .as_ref()
                .map(StoredUploadScope::from)
                .map(|scope| serde_json::to_string(&scope))
                .transpose()
                .map_err(|_| CollectSyncStoreError::Backend)?
        };

        transaction
            .execute(
                "UPDATE collect_sync_state
                 SET next_client_revision = ?1,
                     baseline_status = ?2,
                     active_generation_id = ?3,
                     pending_scope_json = ?4,
                     updated_at_ms = ?5
                 WHERE user_id = ?6 AND client_device_id = ?7",
                params![
                    next_revision,
                    baseline_status.as_str(),
                    active_generation_id,
                    pending_scope_json,
                    input.now_ms,
                    input.account.user_id,
                    input.account.client_device_id,
                ],
            )
            .map_err(|_| CollectSyncStoreError::Backend)?;

        let batches = list_pending_batches(&transaction, &input.account)?;
        let state =
            load_state(&transaction, &input.account)?.ok_or(CollectSyncStoreError::NotFound)?;
        transaction
            .commit()
            .map_err(|_| CollectSyncStoreError::Backend)?;

        Ok(CreateGenerationResult { state, batches })
    }

    fn list_pending_batches(
        &self,
        account: &CollectSyncAccountKey,
    ) -> Result<Vec<OutboxBatch>, CollectSyncStoreError> {
        let database = self
            .database
            .lock()
            .map_err(|_| CollectSyncStoreError::Backend)?;
        list_pending_batches(database.connection(), account)
    }

    fn mark_batch_accepted(
        &self,
        account: &CollectSyncAccountKey,
        batch_id: i64,
        accepted_at_ms: i64,
    ) -> Result<OutboxBatch, CollectSyncStoreError> {
        let mut database = self
            .database
            .lock()
            .map_err(|_| CollectSyncStoreError::Backend)?;
        let transaction = database
            .connection_mut()
            .transaction()
            .map_err(|_| CollectSyncStoreError::Backend)?;

        let updated = transaction
            .execute(
                "UPDATE collect_sync_outbox
                 SET status = 'accepted', accepted_at_ms = ?1
                 WHERE id = ?2
                   AND user_id = ?3
                   AND client_device_id = ?4
                   AND status = 'pending'",
                params![
                    accepted_at_ms,
                    batch_id,
                    account.user_id,
                    account.client_device_id
                ],
            )
            .map_err(|_| CollectSyncStoreError::Backend)?;
        if updated != 1 {
            return Err(CollectSyncStoreError::NotFound);
        }

        transaction
            .execute(
                "UPDATE collect_sync_state
                 SET last_accepted_at_ms = ?1,
                     active_generation_id = CASE
                         WHEN (
                             SELECT COUNT(*) FROM collect_sync_outbox
                             WHERE user_id = ?2 AND client_device_id = ?3 AND status = 'pending'
                         ) = 0 THEN NULL
                         ELSE active_generation_id
                     END,
                     updated_at_ms = ?1
                 WHERE user_id = ?2 AND client_device_id = ?3",
                params![accepted_at_ms, account.user_id, account.client_device_id],
            )
            .map_err(|_| CollectSyncStoreError::Backend)?;

        let batch =
            load_batch(&transaction, account, batch_id)?.ok_or(CollectSyncStoreError::NotFound)?;
        transaction
            .execute(
                "DELETE FROM collect_sync_outbox
                 WHERE id = ?1 AND user_id = ?2 AND client_device_id = ?3 AND status = 'accepted'",
                params![batch_id, account.user_id, account.client_device_id],
            )
            .map_err(|_| CollectSyncStoreError::Backend)?;
        transaction
            .commit()
            .map_err(|_| CollectSyncStoreError::Backend)?;
        Ok(batch)
    }

    fn count_pending_batches(
        &self,
        account: &CollectSyncAccountKey,
    ) -> Result<u32, CollectSyncStoreError> {
        let database = self
            .database
            .lock()
            .map_err(|_| CollectSyncStoreError::Backend)?;
        count_pending_batches(database.connection(), account)
    }

    fn record_attempt_result(
        &self,
        account: &CollectSyncAccountKey,
        now_ms: i64,
        error_code: Option<&str>,
        error_message: Option<&str>,
        retryable: Option<bool>,
    ) -> Result<(), CollectSyncStoreError> {
        let mut database = self
            .database
            .lock()
            .map_err(|_| CollectSyncStoreError::Backend)?;
        let connection = database.connection_mut();
        ensure_state(connection, account, now_ms)?;
        let retryable_i64 = retryable.map(|value| if value { 1_i64 } else { 0_i64 });
        connection
            .execute(
                "UPDATE collect_sync_state
                 SET last_attempt_at_ms = ?1,
                     last_error_code = ?2,
                     last_error_message = ?3,
                     last_error_retryable = ?4,
                     updated_at_ms = ?1
                 WHERE user_id = ?5 AND client_device_id = ?6",
                params![
                    now_ms,
                    error_code,
                    error_message,
                    retryable_i64,
                    account.user_id,
                    account.client_device_id
                ],
            )
            .map_err(|_| CollectSyncStoreError::Backend)?;
        Ok(())
    }

    fn mark_baseline_complete(
        &self,
        account: &CollectSyncAccountKey,
        now_ms: i64,
    ) -> Result<(), CollectSyncStoreError> {
        let mut database = self
            .database
            .lock()
            .map_err(|_| CollectSyncStoreError::Backend)?;
        let connection = database.connection_mut();
        ensure_state(connection, account, now_ms)?;
        connection
            .execute(
                "UPDATE collect_sync_state
                 SET baseline_status = 'complete', updated_at_ms = ?1
                 WHERE user_id = ?2 AND client_device_id = ?3",
                params![now_ms, account.user_id, account.client_device_id],
            )
            .map_err(|_| CollectSyncStoreError::Backend)?;
        Ok(())
    }

    fn set_device_registration(
        &self,
        account: &CollectSyncAccountKey,
        fingerprint: &str,
        registered_revision: i64,
        now_ms: i64,
    ) -> Result<(), CollectSyncStoreError> {
        let mut database = self
            .database
            .lock()
            .map_err(|_| CollectSyncStoreError::Backend)?;
        let connection = database.connection_mut();
        ensure_state(connection, account, now_ms)?;
        connection
            .execute(
                "UPDATE collect_sync_state
                 SET device_metadata_fingerprint = ?1,
                     device_registered_revision = ?2,
                     updated_at_ms = ?3
                 WHERE user_id = ?4 AND client_device_id = ?5",
                params![
                    fingerprint,
                    registered_revision,
                    now_ms,
                    account.user_id,
                    account.client_device_id
                ],
            )
            .map_err(|_| CollectSyncStoreError::Backend)?;
        Ok(())
    }
}

fn validate_prepared_revisions(
    first_revision: i64,
    batches: &[PreparedBatch],
) -> Result<(), CollectSyncStoreError> {
    for (index, batch) in batches.iter().enumerate() {
        let expected = first_revision
            .checked_add(i64::try_from(index).map_err(|_| CollectSyncStoreError::Backend)?)
            .ok_or(CollectSyncStoreError::Backend)?;
        if batch.client_revision != expected {
            return Err(CollectSyncStoreError::RevisionMismatch);
        }
        if batch.batch_index as usize != index {
            return Err(CollectSyncStoreError::InvalidState);
        }
    }
    Ok(())
}

fn insert_outbox_batch(
    connection: &Connection,
    account: &CollectSyncAccountKey,
    generation_id: &str,
    batch: &PreparedBatch,
    now_ms: i64,
) -> Result<(), CollectSyncStoreError> {
    connection
        .execute(
            "INSERT INTO collect_sync_outbox (
                user_id, client_device_id, generation_id, batch_index, batch_count,
                client_revision, idempotency_key, request_body, payload_hash,
                window_scope, window_start, window_end, status, created_at_ms, accepted_at_ms
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5,
                ?6, ?7, ?8, ?9,
                ?10, ?11, ?12, 'pending', ?13, NULL
             )",
            params![
                account.user_id,
                account.client_device_id,
                generation_id,
                batch.batch_index,
                batch.batch_count,
                batch.client_revision,
                batch.idempotency_key,
                batch.request_body,
                batch.payload_hash,
                batch.window_scope.as_str(),
                batch.window_start,
                batch.window_end,
                now_ms,
            ],
        )
        .map_err(|_| CollectSyncStoreError::Backend)?;
    Ok(())
}

fn ensure_state(
    connection: &Connection,
    account: &CollectSyncAccountKey,
    now_ms: i64,
) -> Result<CollectSyncState, CollectSyncStoreError> {
    if let Some(state) = load_state(connection, account)? {
        return Ok(state);
    }
    connection
        .execute(
            "INSERT INTO collect_sync_state (
                user_id, client_device_id, next_client_revision, baseline_status,
                pending_scope_json, active_generation_id,
                last_attempt_at_ms, last_accepted_at_ms,
                last_error_code, last_error_message, last_error_retryable,
                device_metadata_fingerprint, device_registered_revision,
                created_at_ms, updated_at_ms
             ) VALUES (
                ?1, ?2, 1, 'none',
                NULL, NULL,
                NULL, NULL,
                NULL, NULL, NULL,
                NULL, NULL,
                ?3, ?3
             )",
            params![account.user_id, account.client_device_id, now_ms],
        )
        .map_err(|_| CollectSyncStoreError::Backend)?;
    load_state(connection, account)?.ok_or(CollectSyncStoreError::NotFound)
}

fn load_state(
    connection: &Connection,
    account: &CollectSyncAccountKey,
) -> Result<Option<CollectSyncState>, CollectSyncStoreError> {
    let row = connection
        .query_row(
            "SELECT
                next_client_revision, baseline_status, pending_scope_json, active_generation_id,
                last_attempt_at_ms, last_accepted_at_ms,
                last_error_code, last_error_message, last_error_retryable,
                device_metadata_fingerprint, device_registered_revision
             FROM collect_sync_state
             WHERE user_id = ?1 AND client_device_id = ?2",
            params![account.user_id, account.client_device_id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<i64>>(4)?,
                    row.get::<_, Option<i64>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, Option<i64>>(8)?,
                    row.get::<_, Option<String>>(9)?,
                    row.get::<_, Option<i64>>(10)?,
                ))
            },
        )
        .optional()
        .map_err(|_| CollectSyncStoreError::Backend)?;

    row.map(
        |(
            next_client_revision,
            baseline_status,
            pending_scope_json,
            active_generation_id,
            last_attempt_at_ms,
            last_accepted_at_ms,
            last_error_code,
            last_error_message,
            last_error_retryable,
            device_metadata_fingerprint,
            device_registered_revision,
        )|
         -> Result<CollectSyncState, CollectSyncStoreError> {
            let pending_scope = match pending_scope_json {
                Some(json) => {
                    let stored: StoredUploadScope = serde_json::from_str(&json)
                        .map_err(|_| CollectSyncStoreError::InvalidState)?;
                    Some(
                        UploadScope::try_from(stored)
                            .map_err(|_| CollectSyncStoreError::InvalidState)?,
                    )
                }
                None => None,
            };
            Ok(CollectSyncState {
                account: account.clone(),
                next_client_revision,
                baseline_status: BaselineStatus::parse(&baseline_status)
                    .ok_or(CollectSyncStoreError::InvalidState)?,
                pending_scope,
                active_generation_id,
                last_attempt_at_ms,
                last_accepted_at_ms,
                last_error_code,
                last_error_message,
                last_error_retryable: last_error_retryable.map(|value| value != 0),
                device_metadata_fingerprint,
                device_registered_revision,
            })
        },
    )
    .transpose()
}

fn count_pending_batches(
    connection: &Connection,
    account: &CollectSyncAccountKey,
) -> Result<u32, CollectSyncStoreError> {
    let count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM collect_sync_outbox
             WHERE user_id = ?1 AND client_device_id = ?2 AND status = 'pending'",
            params![account.user_id, account.client_device_id],
            |row| row.get(0),
        )
        .map_err(|_| CollectSyncStoreError::Backend)?;
    u32::try_from(count).map_err(|_| CollectSyncStoreError::Backend)
}

fn list_pending_batches(
    connection: &Connection,
    account: &CollectSyncAccountKey,
) -> Result<Vec<OutboxBatch>, CollectSyncStoreError> {
    let mut statement = connection
        .prepare(
            "SELECT
                id, generation_id, batch_index, batch_count, client_revision,
                idempotency_key, request_body, payload_hash,
                window_scope, window_start, window_end, status,
                created_at_ms, accepted_at_ms
             FROM collect_sync_outbox
             WHERE user_id = ?1 AND client_device_id = ?2 AND status = 'pending'
             ORDER BY client_revision ASC",
        )
        .map_err(|_| CollectSyncStoreError::Backend)?;
    let rows = statement
        .query_map(params![account.user_id, account.client_device_id], |row| {
            map_outbox_row(account, row)
        })
        .map_err(|_| CollectSyncStoreError::Backend)?;

    let mut batches = Vec::new();
    for row in rows {
        batches.push(row.map_err(|_| CollectSyncStoreError::Backend)?);
    }
    Ok(batches)
}

fn load_batch(
    connection: &Connection,
    account: &CollectSyncAccountKey,
    batch_id: i64,
) -> Result<Option<OutboxBatch>, CollectSyncStoreError> {
    connection
        .query_row(
            "SELECT
                id, generation_id, batch_index, batch_count, client_revision,
                idempotency_key, request_body, payload_hash,
                window_scope, window_start, window_end, status,
                created_at_ms, accepted_at_ms
             FROM collect_sync_outbox
             WHERE id = ?1 AND user_id = ?2 AND client_device_id = ?3",
            params![batch_id, account.user_id, account.client_device_id],
            |row| map_outbox_row(account, row),
        )
        .optional()
        .map_err(|_| CollectSyncStoreError::Backend)
}

fn map_outbox_row(
    account: &CollectSyncAccountKey,
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<OutboxBatch> {
    let window_scope = match row.get::<_, String>(8)?.as_str() {
        "full" => WireUploadScope::Full,
        "incremental" => WireUploadScope::Incremental,
        other => {
            return Err(rusqlite::Error::FromSqlConversionFailure(
                8,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("unknown window scope {other}"),
                )),
            ))
        }
    };
    let status = OutboxBatchStatus::parse(&row.get::<_, String>(11)?).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            11,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "unknown outbox status",
            )),
        )
    })?;

    Ok(OutboxBatch {
        id: row.get(0)?,
        account: account.clone(),
        generation_id: row.get(1)?,
        batch_index: row.get::<_, i64>(2)? as u32,
        batch_count: row.get::<_, i64>(3)? as u32,
        client_revision: row.get(4)?,
        idempotency_key: row.get(5)?,
        request_body: row.get(6)?,
        payload_hash: row.get(7)?,
        window_scope,
        window_start: row.get(9)?,
        window_end: row.get(10)?,
        status,
        created_at_ms: row.get(12)?,
        accepted_at_ms: row.get(13)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::collect_sync::{
        build_prepared_batches, BatchBuildLimits, BatchRequestMeta, DailyUsageCostDto,
        DailyUsageFactDto, UploadScope,
    };
    use crate::application::ports::collect_sync_store::CollectSyncStore;

    fn account(user: &str) -> CollectSyncAccountKey {
        CollectSyncAccountKey {
            user_id: user.to_owned(),
            client_device_id: "dev_1".to_owned(),
        }
    }

    fn open_store() -> SqliteCollectSyncStore {
        let directory = tempfile::TempDir::new().expect("tempdir");
        let path = directory.keep().join("burnly.sqlite3");
        let mut database = Database::open(&path).expect("open");
        database.migrate_to_latest().expect("migrate");
        SqliteCollectSyncStore::new(database)
    }

    fn sample_fact(date: &str) -> DailyUsageFactDto {
        DailyUsageFactDto {
            identity_key: format!("claude-code:daily:v1:UTC:{date}"),
            identity_version: 1,
            source_key: "claude-code".to_owned(),
            usage_date: date.to_owned(),
            aggregation_timezone: "UTC".to_owned(),
            input_tokens: Some(1),
            output_tokens: Some(1),
            cache_creation_tokens: Some(0),
            cache_read_tokens: Some(0),
            total_tokens: 2,
            unclassified_tokens: Some(0),
            cost: DailyUsageCostDto {
                status: "unavailable".to_owned(),
                kind: "unknown".to_owned(),
                amount_micros: None,
                currency: None,
            },
            data_quality: "complete".to_owned(),
            record_state: "active".to_owned(),
            first_seen_at: "2026-07-08T00:00:00.000Z".to_owned(),
            last_seen_at: "2026-07-08T00:00:00.000Z".to_owned(),
            removed_at: None,
            models: vec![],
        }
    }

    #[test]
    fn merges_pending_scopes_per_account() {
        let store = open_store();
        let a = account("user-a");
        let b = account("user-b");
        let first = UploadScope::incremental(
            ["claude-code".to_owned()],
            chrono::NaiveDate::from_ymd_opt(2026, 7, 1).expect("d"),
            chrono::NaiveDate::from_ymd_opt(2026, 7, 2).expect("d"),
        )
        .expect("scope");
        let second = UploadScope::incremental(
            ["codex".to_owned()],
            chrono::NaiveDate::from_ymd_opt(2026, 7, 3).expect("d"),
            chrono::NaiveDate::from_ymd_opt(2026, 7, 4).expect("d"),
        )
        .expect("scope");

        store.merge_pending_scope(&a, first, 10).expect("merge a");
        store
            .merge_pending_scope(&a, second.clone(), 11)
            .expect("merge a2");
        store.merge_pending_scope(&b, second, 12).expect("merge b");

        let state_a = store.load_state(&a).expect("load").expect("state");
        let state_b = store.load_state(&b).expect("load").expect("state");
        assert!(matches!(
            state_a.pending_scope,
            Some(UploadScope::Incremental { ref source_keys, .. })
                if source_keys.len() == 2
        ));
        assert!(matches!(
            state_b.pending_scope,
            Some(UploadScope::Incremental { ref source_keys, .. })
                if source_keys.len() == 1
        ));
    }

    #[test]
    fn create_generation_is_transactional_and_immutable() {
        let store = open_store();
        let a = account("user-a");
        store.ensure_state(&a, 1).expect("ensure");
        let meta = BatchRequestMeta {
            client_device_id: a.client_device_id.clone(),
            app_version: "0.1.20".to_owned(),
            reporting_timezone: "UTC".to_owned(),
            scope: UploadScope::Full,
        };
        let prepared = build_prepared_batches(
            vec![sample_fact("2026-07-08"), sample_fact("2026-07-09")],
            &meta,
            BatchBuildLimits {
                max_facts_per_batch: 1,
                max_models_per_fact: 100,
            },
            1,
        )
        .expect("batches");
        let original_body = prepared[0].request_body.clone();
        let original_key = prepared[0].idempotency_key.clone();

        let created = store
            .create_generation(CreateGenerationInput {
                account: a.clone(),
                generation_id: "gen-1".to_owned(),
                meta,
                prepared_batches: prepared,
                now_ms: 100,
                marks_baseline_in_progress: true,
                clear_pending_scope: true,
            })
            .expect("create");

        assert_eq!(created.batches.len(), 2);
        assert_eq!(created.batches[0].request_body, original_body);
        assert_eq!(created.batches[0].idempotency_key, original_key);
        assert_eq!(created.state.next_client_revision, 3);
        assert_eq!(created.state.baseline_status, BaselineStatus::InProgress);

        let error = store
            .create_generation(CreateGenerationInput {
                account: a.clone(),
                generation_id: "gen-2".to_owned(),
                meta: BatchRequestMeta {
                    client_device_id: a.client_device_id.clone(),
                    app_version: "0.1.20".to_owned(),
                    reporting_timezone: "UTC".to_owned(),
                    scope: UploadScope::Full,
                },
                prepared_batches: vec![],
                now_ms: 200,
                marks_baseline_in_progress: false,
                clear_pending_scope: false,
            })
            .expect_err("pending blocks");
        assert_eq!(error, CollectSyncStoreError::PendingGenerationExists);

        let accepted = store
            .mark_batch_accepted(&a, created.batches[0].id, 300)
            .expect("accept first");
        assert_eq!(accepted.status, OutboxBatchStatus::Accepted);
        assert_eq!(store.count_pending_batches(&a).expect("count"), 1);
        let retained: i64 = store
            .database
            .lock()
            .expect("database")
            .connection()
            .query_row(
                "SELECT COUNT(*) FROM collect_sync_outbox WHERE id = ?1",
                params![accepted.id],
                |row| row.get(0),
            )
            .expect("retained count");
        assert_eq!(retained, 0, "accepted payload must be removed");
    }

    #[test]
    fn full_pending_scope_dominates() {
        let store = open_store();
        let a = account("user-a");
        let incremental = UploadScope::incremental(
            ["claude-code".to_owned()],
            chrono::NaiveDate::from_ymd_opt(2026, 7, 1).expect("d"),
            chrono::NaiveDate::from_ymd_opt(2026, 7, 2).expect("d"),
        )
        .expect("scope");
        store
            .merge_pending_scope(&a, incremental, 1)
            .expect("merge");
        let merged = store
            .merge_pending_scope(&a, UploadScope::Full, 2)
            .expect("full");
        assert_eq!(merged, UploadScope::Full);
    }

    #[test]
    fn merge_pending_scope_for_all_accounts_updates_every_account() {
        let store = open_store();
        let a = account("user-a");
        let b = account("user-b");

        let initial_a = UploadScope::incremental(
            ["antigravity".to_owned()],
            chrono::NaiveDate::from_ymd_opt(2026, 7, 1).expect("d"),
            chrono::NaiveDate::from_ymd_opt(2026, 7, 2).expect("d"),
        )
        .expect("scope");
        store
            .merge_pending_scope(&a, initial_a, 1)
            .expect("merge a");
        store.ensure_state(&b, 1).expect("ensure b");

        let incoming = UploadScope::incremental(
            ["antigravity".to_owned()],
            chrono::NaiveDate::from_ymd_opt(2026, 7, 3).expect("d"),
            chrono::NaiveDate::from_ymd_opt(2026, 7, 4).expect("d"),
        )
        .expect("scope");

        let updated = store
            .merge_pending_scope_for_all_accounts(&incoming, 2)
            .expect("merge all");
        assert_eq!(updated, 2);

        let state_a = store.load_state(&a).expect("load a").expect("state a");
        let state_b = store.load_state(&b).expect("load b").expect("state b");

        assert_eq!(
            state_a.pending_scope,
            Some(
                UploadScope::incremental(
                    ["antigravity".to_owned()],
                    chrono::NaiveDate::from_ymd_opt(2026, 7, 1).expect("d"),
                    chrono::NaiveDate::from_ymd_opt(2026, 7, 4).expect("d"),
                )
                .expect("merged")
            )
        );
        assert_eq!(state_b.pending_scope, Some(incoming));
    }
}
