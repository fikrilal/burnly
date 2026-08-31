//! Bounded, privacy-safe OpenCode V1/V2 usage reader.

use std::num::NonZeroUsize;
use std::path::Path;
use std::time::Duration;

use rusqlite::{params, Connection, Transaction, TransactionBehavior};
use thiserror::Error;

use super::super::support::open_external_read_only;
use super::schema::{
    inspect_schema, OpenCodeGeneration, OpenCodeSchemaError, OpenCodeSchemaInspection,
};
use crate::application::collection::CollectorFailureCode;

const MAX_PAGE_SIZE: usize = 1_000;
const BUSY_TIMEOUT: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OpenCodePageSize(NonZeroUsize);

impl OpenCodePageSize {
    pub(crate) fn new(value: usize) -> Result<Self, OpenCodePageSizeError> {
        let value = NonZeroUsize::new(value).ok_or(OpenCodePageSizeError::OutOfRange)?;
        if value.get() > MAX_PAGE_SIZE {
            return Err(OpenCodePageSizeError::OutOfRange);
        }
        Ok(Self(value))
    }

    fn sqlite_limit(self) -> i64 {
        i64::try_from(self.0.get()).expect("bounded page size fits in i64")
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OpenCodePageSizeError {
    #[error("OpenCode page size must be between 1 and 1000")]
    OutOfRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OpenCodeTokenCounters {
    pub(crate) input: u64,
    pub(crate) output: u64,
    pub(crate) reasoning: u64,
    pub(crate) cache_read: u64,
    pub(crate) cache_write: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct OpenCodeSessionHeader {
    pub(crate) id: String,
    pub(crate) generation: OpenCodeGeneration,
    pub(crate) created_at_ms: i64,
    pub(crate) updated_at_ms: i64,
    pub(crate) idle_at_ms: Option<i64>,
    pub(crate) tokens: OpenCodeTokenCounters,
    pub(crate) cost_usd: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct OpenCodeMessageUsage {
    pub(crate) id: String,
    pub(crate) session_id: String,
    pub(crate) generation: OpenCodeGeneration,
    pub(crate) created_at_ms: i64,
    pub(crate) completed_at_ms: Option<i64>,
    pub(crate) provider_id: String,
    pub(crate) model_id: String,
    pub(crate) tokens: OpenCodeTokenCounters,
    pub(crate) cost_usd: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct OpenCodeMessagePage {
    pub(crate) messages: Vec<OpenCodeMessageUsage>,
    pub(crate) last_row_id: Option<String>,
    pub(crate) non_usage_error_rows: u64,
}

impl OpenCodeMessagePage {
    pub(crate) fn has_rows(&self) -> bool {
        self.last_row_id.is_some()
    }
}

impl std::ops::Deref for OpenCodeMessagePage {
    type Target = [OpenCodeMessageUsage];

    fn deref(&self) -> &Self::Target {
        &self.messages
    }
}

impl IntoIterator for OpenCodeMessagePage {
    type Item = OpenCodeMessageUsage;
    type IntoIter = std::vec::IntoIter<OpenCodeMessageUsage>;

    fn into_iter(self) -> Self::IntoIter {
        self.messages.into_iter()
    }
}

#[derive(Debug, Error)]
pub(crate) enum OpenCodeStoreError {
    #[error("OpenCode database could not be opened")]
    Open(#[source] rusqlite::Error),
    #[error("OpenCode database connection could not be configured")]
    Configure(#[source] rusqlite::Error),
    #[error("OpenCode database schema is incompatible")]
    Schema(#[source] OpenCodeSchemaError),
    #[error("OpenCode database snapshot could not be started")]
    Snapshot(#[source] rusqlite::Error),
    #[error("OpenCode usage data could not be read")]
    Query(#[source] rusqlite::Error),
    #[error("OpenCode usage row is incompatible")]
    Incompatible,
    #[error("OpenCode pagination cursor is invalid")]
    InvalidCursor,
}

impl OpenCodeStoreError {
    /// Classifies an open/schema failure into the source-level collector
    /// failure code the adapter should surface. The `Open` arm is
    /// path-dependent (permission vs invalid location) and is classified by
    /// `open_failure_code` where the path is available.
    pub(crate) fn source_failure_code(&self) -> CollectorFailureCode {
        match self {
            Self::Open(_) => CollectorFailureCode::SourceInvalidLocation,
            Self::Configure(_) | Self::Schema(_) => CollectorFailureCode::IncompatibleEnvelope,
            Self::Snapshot(_) | Self::Query(_) => CollectorFailureCode::IncompatibleEnvelope,
            Self::Incompatible | Self::InvalidCursor => CollectorFailureCode::IncompatibleEnvelope,
        }
    }
}

/// Classifies a failed read-only open of an existing path. SQLite reports an
/// unreadable file as `SQLITE_CANTOPEN` rather than `SQLITE_PERM`, so probe the
/// same read access SQLite needs: a `PermissionDenied` from `File::open` means
/// the user cannot read the source artifact.
pub(crate) fn open_failure_code(path: &Path) -> CollectorFailureCode {
    match std::fs::File::open(path) {
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            CollectorFailureCode::SourcePermissionDenied
        }
        _ => CollectorFailureCode::SourceInvalidLocation,
    }
}

#[derive(Debug)]
pub(crate) struct OpenCodeStore {
    connection: Connection,
    inspection: OpenCodeSchemaInspection,
}

impl OpenCodeStore {
    pub(crate) fn open_read_only(path: impl AsRef<Path>) -> Result<Self, OpenCodeStoreError> {
        let connection = open_external_read_only(path).map_err(OpenCodeStoreError::Open)?;
        connection
            .busy_timeout(BUSY_TIMEOUT)
            .map_err(OpenCodeStoreError::Configure)?;
        connection
            .pragma_update(None, "query_only", true)
            .map_err(OpenCodeStoreError::Configure)?;
        let inspection = inspect_schema(&connection).map_err(OpenCodeStoreError::Schema)?;
        Ok(Self {
            connection,
            inspection,
        })
    }

    pub(crate) const fn capabilities(&self) -> OpenCodeSchemaInspection {
        self.inspection
    }

    pub(crate) fn begin_snapshot(
        &mut self,
    ) -> Result<OpenCodeReadSnapshot<'_>, OpenCodeStoreError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(OpenCodeStoreError::Snapshot)?;
        Ok(OpenCodeReadSnapshot {
            transaction,
            inspection: self.inspection,
        })
    }
}

pub(crate) struct OpenCodeReadSnapshot<'connection> {
    transaction: Transaction<'connection>,
    inspection: OpenCodeSchemaInspection,
}

impl OpenCodeReadSnapshot<'_> {
    pub(crate) fn read_sessions_page(
        &self,
        after_id: Option<&str>,
        page_size: OpenCodePageSize,
    ) -> Result<Vec<OpenCodeSessionHeader>, OpenCodeStoreError> {
        validate_cursor(after_id)?;
        let sql = session_query(self.inspection);
        let mut statement = self
            .transaction
            .prepare(sql)
            .map_err(OpenCodeStoreError::Query)?;
        let rows = statement
            .query_map(params![after_id, page_size.sqlite_limit()], |row| {
                Ok(RawSessionHeader {
                    id: row.get(0)?,
                    generation: row.get(1)?,
                    created_at_ms: row.get(2)?,
                    updated_at_ms: row.get(3)?,
                    idle_at_ms: row.get(4)?,
                    input: row.get(5)?,
                    output: row.get(6)?,
                    reasoning: row.get(7)?,
                    cache_read: row.get(8)?,
                    cache_write: row.get(9)?,
                    cost_usd: row.get(10)?,
                })
            })
            .map_err(OpenCodeStoreError::Query)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(OpenCodeStoreError::Query)?;
        rows.into_iter()
            .map(OpenCodeSessionHeader::try_from)
            .collect()
    }

    pub(crate) fn read_messages_page(
        &self,
        session_id: &str,
        after_id: Option<&str>,
        page_size: OpenCodePageSize,
    ) -> Result<OpenCodeMessagePage, OpenCodeStoreError> {
        if session_id.trim().is_empty() {
            return Err(OpenCodeStoreError::InvalidCursor);
        }
        validate_cursor(after_id)?;
        let sql = message_query(self.inspection);
        let mut statement = self
            .transaction
            .prepare(sql)
            .map_err(OpenCodeStoreError::Query)?;
        let rows = statement
            .query_map(
                params![session_id, after_id, page_size.sqlite_limit()],
                |row| {
                    Ok(RawMessageUsage {
                        id: row.get(0)?,
                        session_id: row.get(1)?,
                        generation: row.get(2)?,
                        created_at_ms: row.get(3)?,
                        completed_at_ms: row.get(4)?,
                        provider_id: row.get(5)?,
                        model_id: row.get(6)?,
                        input: row.get(7)?,
                        output: row.get(8)?,
                        reasoning: row.get(9)?,
                        cache_read: row.get(10)?,
                        cache_write: row.get(11)?,
                        cost_usd: row.get(12)?,
                        has_error_object: row.get::<_, i64>(13)? != 0,
                    })
                },
            )
            .map_err(OpenCodeStoreError::Query)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(OpenCodeStoreError::Query)?;
        let last_row_id = rows.last().map(|row| row.id.clone());
        let mut messages = Vec::with_capacity(rows.len());
        let mut non_usage_error_rows = 0_u64;
        for row in rows {
            match row.into_usage()? {
                Some(message) => messages.push(message),
                None => non_usage_error_rows = non_usage_error_rows.saturating_add(1),
            }
        }
        Ok(OpenCodeMessagePage {
            messages,
            last_row_id,
            non_usage_error_rows,
        })
    }
}

struct RawSessionHeader {
    id: String,
    generation: i64,
    created_at_ms: i64,
    updated_at_ms: i64,
    idle_at_ms: Option<i64>,
    input: i64,
    output: i64,
    reasoning: i64,
    cache_read: i64,
    cache_write: i64,
    cost_usd: f64,
}

impl TryFrom<RawSessionHeader> for OpenCodeSessionHeader {
    type Error = OpenCodeStoreError;

    fn try_from(value: RawSessionHeader) -> Result<Self, Self::Error> {
        if value.id.trim().is_empty()
            || value.created_at_ms < 0
            || value.updated_at_ms < 0
            || value.idle_at_ms.is_some_and(|timestamp| timestamp < 0)
            || !valid_cost(value.cost_usd)
        {
            return Err(OpenCodeStoreError::Incompatible);
        }
        Ok(Self {
            id: value.id,
            generation: generation(value.generation)?,
            created_at_ms: value.created_at_ms,
            updated_at_ms: value.updated_at_ms,
            idle_at_ms: value.idle_at_ms,
            tokens: counters(
                value.input,
                value.output,
                value.reasoning,
                value.cache_read,
                value.cache_write,
            )?,
            cost_usd: value.cost_usd,
        })
    }
}

struct RawMessageUsage {
    id: String,
    session_id: String,
    generation: i64,
    created_at_ms: Option<i64>,
    completed_at_ms: Option<i64>,
    provider_id: Option<String>,
    model_id: Option<String>,
    input: Option<i64>,
    output: Option<i64>,
    reasoning: Option<i64>,
    cache_read: Option<i64>,
    cache_write: Option<i64>,
    cost_usd: Option<f64>,
    has_error_object: bool,
}

impl RawMessageUsage {
    fn into_usage(self) -> Result<Option<OpenCodeMessageUsage>, OpenCodeStoreError> {
        let is_non_usage_error = self.generation == 2
            && self.has_error_object
            && self.input.is_none()
            && self.output.is_none()
            && self.reasoning.is_none()
            && self.cache_read.is_none()
            && self.cache_write.is_none()
            && self.cost_usd.is_none();
        if is_non_usage_error {
            let created_at_ms = self.created_at_ms.ok_or(OpenCodeStoreError::Incompatible)?;
            generation(self.generation)?;
            required_text(Some(self.id))?;
            required_text(Some(self.session_id))?;
            required_text(self.provider_id)?;
            required_text(self.model_id)?;
            if created_at_ms < 0
                || self
                    .completed_at_ms
                    .is_some_and(|timestamp| timestamp < created_at_ms)
            {
                return Err(OpenCodeStoreError::Incompatible);
            }
            return Ok(None);
        }
        OpenCodeMessageUsage::try_from(self).map(Some)
    }
}

impl TryFrom<RawMessageUsage> for OpenCodeMessageUsage {
    type Error = OpenCodeStoreError;

    fn try_from(value: RawMessageUsage) -> Result<Self, Self::Error> {
        let created_at_ms = value
            .created_at_ms
            .ok_or(OpenCodeStoreError::Incompatible)?;
        let provider_id = required_text(value.provider_id)?;
        let model_id = required_text(value.model_id)?;
        if value.id.trim().is_empty()
            || value.session_id.trim().is_empty()
            || created_at_ms < 0
            || value.completed_at_ms.is_some_and(|timestamp| timestamp < 0)
            || value.cost_usd.is_some_and(|cost| !valid_cost(cost))
        {
            return Err(OpenCodeStoreError::Incompatible);
        }
        Ok(Self {
            id: value.id,
            session_id: value.session_id,
            generation: generation(value.generation)?,
            created_at_ms,
            completed_at_ms: value.completed_at_ms,
            provider_id,
            model_id,
            tokens: counters(
                required_integer(value.input)?,
                required_integer(value.output)?,
                required_integer(value.reasoning)?,
                required_integer(value.cache_read)?,
                required_integer(value.cache_write)?,
            )?,
            cost_usd: value.cost_usd,
        })
    }
}

fn required_text(value: Option<String>) -> Result<String, OpenCodeStoreError> {
    value
        .filter(|text| !text.trim().is_empty())
        .ok_or(OpenCodeStoreError::Incompatible)
}

fn required_integer(value: Option<i64>) -> Result<i64, OpenCodeStoreError> {
    value.ok_or(OpenCodeStoreError::Incompatible)
}

fn counters(
    input: i64,
    output: i64,
    reasoning: i64,
    cache_read: i64,
    cache_write: i64,
) -> Result<OpenCodeTokenCounters, OpenCodeStoreError> {
    Ok(OpenCodeTokenCounters {
        input: non_negative(input)?,
        output: non_negative(output)?,
        reasoning: non_negative(reasoning)?,
        cache_read: non_negative(cache_read)?,
        cache_write: non_negative(cache_write)?,
    })
}

fn non_negative(value: i64) -> Result<u64, OpenCodeStoreError> {
    u64::try_from(value).map_err(|_| OpenCodeStoreError::Incompatible)
}

fn generation(value: i64) -> Result<OpenCodeGeneration, OpenCodeStoreError> {
    match value {
        1 => Ok(OpenCodeGeneration::V1),
        2 => Ok(OpenCodeGeneration::V2),
        _ => Err(OpenCodeStoreError::Incompatible),
    }
}

fn valid_cost(value: f64) -> bool {
    value.is_finite() && value >= 0.0
}

fn validate_cursor(cursor: Option<&str>) -> Result<(), OpenCodeStoreError> {
    if cursor.is_some_and(|value| value.trim().is_empty()) {
        Err(OpenCodeStoreError::InvalidCursor)
    } else {
        Ok(())
    }
}

fn session_query(inspection: OpenCodeSchemaInspection) -> &'static str {
    match (inspection.has_v1(), inspection.has_v2()) {
        (true, false) => V1_SESSION_QUERY,
        (false, true) => V2_SESSION_QUERY,
        (true, true) => COMBINED_SESSION_QUERY,
        (false, false) => unreachable!("schema inspection rejects unsupported databases"),
    }
}

fn message_query(inspection: OpenCodeSchemaInspection) -> &'static str {
    match (inspection.has_v1(), inspection.has_v2()) {
        (true, false) => V1_MESSAGE_QUERY,
        (false, true) => V2_MESSAGE_QUERY,
        (true, true) => COMBINED_MESSAGE_QUERY,
        (false, false) => unreachable!("schema inspection rejects unsupported databases"),
    }
}

const V1_SESSION_QUERY: &str = "
    SELECT
        s.id, 1 AS generation, s.time_created, s.time_updated, NULL AS time_idle,
        s.tokens_input, s.tokens_output, s.tokens_reasoning,
        s.tokens_cache_read, s.tokens_cache_write, s.cost
    FROM session s
    WHERE (?1 IS NULL OR s.id > ?1)
    ORDER BY s.id ASC
    LIMIT ?2";
const V2_SESSION_QUERY: &str = "
    SELECT
        v.id, 2 AS generation, v.time_created, v.time_updated, v.time_idle,
        v.tokens_input, v.tokens_output, v.tokens_reasoning,
        v.tokens_cache_read, v.tokens_cache_write, v.cost
    FROM session_v2 v
    WHERE (?1 IS NULL OR v.id > ?1)
    ORDER BY v.id ASC
    LIMIT ?2";
const COMBINED_SESSION_QUERY: &str = "
    SELECT
        v.id, 2 AS generation, v.time_created, v.time_updated, v.time_idle,
        v.tokens_input, v.tokens_output, v.tokens_reasoning,
        v.tokens_cache_read, v.tokens_cache_write, v.cost
    FROM session_v2 v
    WHERE (?1 IS NULL OR v.id > ?1)
    UNION ALL
    SELECT
        s.id, 1 AS generation, s.time_created, s.time_updated, NULL AS time_idle,
        s.tokens_input, s.tokens_output, s.tokens_reasoning,
        s.tokens_cache_read, s.tokens_cache_write, s.cost
    FROM session s
    WHERE (?1 IS NULL OR s.id > ?1)
      AND NOT EXISTS (SELECT 1 FROM session_v2 v WHERE v.id = s.id)
    ORDER BY id ASC
    LIMIT ?2";

const V1_MESSAGE_QUERY: &str = "
    SELECT
        m.id, m.session_id, 1 AS generation,
        json_extract(m.data, '$.time.created'),
        json_extract(m.data, '$.time.completed'),
        json_extract(m.data, '$.providerID'),
        json_extract(m.data, '$.modelID'),
        json_extract(m.data, '$.tokens.input'),
        json_extract(m.data, '$.tokens.output'),
        json_extract(m.data, '$.tokens.reasoning'),
        json_extract(m.data, '$.tokens.cache.read'),
        json_extract(m.data, '$.tokens.cache.write'),
        json_extract(m.data, '$.cost'),
        0
    FROM message m
    WHERE m.session_id = ?1
      AND json_extract(m.data, '$.role') = 'assistant'
      AND (?2 IS NULL OR m.id > ?2)
    ORDER BY m.id ASC
    LIMIT ?3";
const V2_MESSAGE_QUERY: &str = "
    SELECT
        m.id, m.session_id, 2 AS generation,
        json_extract(m.data, '$.time.created'),
        json_extract(m.data, '$.time.completed'),
        json_extract(m.data, '$.model.providerID'),
        json_extract(m.data, '$.model.id'),
        json_extract(m.data, '$.tokens.input'),
        json_extract(m.data, '$.tokens.output'),
        json_extract(m.data, '$.tokens.reasoning'),
        json_extract(m.data, '$.tokens.cache.read'),
        json_extract(m.data, '$.tokens.cache.write'),
        json_extract(m.data, '$.cost'),
        CASE WHEN json_type(m.data, '$.error') = 'object' THEN 1 ELSE 0 END
    FROM session_message m
    WHERE m.session_id = ?1
      AND m.type = 'assistant'
      AND (?2 IS NULL OR m.id > ?2)
    ORDER BY m.id ASC
    LIMIT ?3";
const COMBINED_MESSAGE_QUERY: &str = "
    SELECT
        m.id, m.session_id, 2 AS generation,
        json_extract(m.data, '$.time.created'),
        json_extract(m.data, '$.time.completed'),
        json_extract(m.data, '$.model.providerID'),
        json_extract(m.data, '$.model.id'),
        json_extract(m.data, '$.tokens.input'),
        json_extract(m.data, '$.tokens.output'),
        json_extract(m.data, '$.tokens.reasoning'),
        json_extract(m.data, '$.tokens.cache.read'),
        json_extract(m.data, '$.tokens.cache.write'),
        json_extract(m.data, '$.cost'),
        CASE WHEN json_type(m.data, '$.error') = 'object' THEN 1 ELSE 0 END
    FROM session_message m
    WHERE m.session_id = ?1
      AND m.type = 'assistant'
      AND (?2 IS NULL OR m.id > ?2)
    UNION ALL
    SELECT
        m.id, m.session_id, 1 AS generation,
        json_extract(m.data, '$.time.created'),
        json_extract(m.data, '$.time.completed'),
        json_extract(m.data, '$.providerID'),
        json_extract(m.data, '$.modelID'),
        json_extract(m.data, '$.tokens.input'),
        json_extract(m.data, '$.tokens.output'),
        json_extract(m.data, '$.tokens.reasoning'),
        json_extract(m.data, '$.tokens.cache.read'),
        json_extract(m.data, '$.tokens.cache.write'),
        json_extract(m.data, '$.cost'),
        0
    FROM message m
    WHERE m.session_id = ?1
      AND json_extract(m.data, '$.role') = 'assistant'
      AND (?2 IS NULL OR m.id > ?2)
      AND NOT EXISTS (SELECT 1 FROM session_message v WHERE v.id = m.id)
    ORDER BY id ASC
    LIMIT ?3";

#[cfg(test)]
mod tests {
    use rusqlite::Connection;
    use serde_json::json;
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn page_size_rejects_zero_and_unbounded_values() {
        assert_eq!(
            OpenCodePageSize::new(0),
            Err(OpenCodePageSizeError::OutOfRange)
        );
        assert_eq!(
            OpenCodePageSize::new(MAX_PAGE_SIZE + 1),
            Err(OpenCodePageSizeError::OutOfRange)
        );
        assert_eq!(
            OpenCodePageSize::new(MAX_PAGE_SIZE)
                .expect("maximum")
                .sqlite_limit(),
            1_000
        );
    }

    #[test]
    fn reads_v1_only_usage() {
        let database = FixtureDatabase::new(true, false);
        let connection = database.write();
        insert_v1_session(&connection, "session-v1", 10);
        insert_v1_message(&connection, "message-v1", "session-v1", 11, 7);
        drop(connection);

        let mut store = OpenCodeStore::open_read_only(&database.path).expect("store");
        assert!(store.capabilities().has_v1());
        assert!(!store.capabilities().has_v2());
        let snapshot = store.begin_snapshot().expect("snapshot");
        let sessions = snapshot
            .read_sessions_page(None, page_size(10))
            .expect("sessions");
        let messages = snapshot
            .read_messages_page("session-v1", None, page_size(10))
            .expect("messages");

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].generation, OpenCodeGeneration::V1);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].generation, OpenCodeGeneration::V1);
        assert_eq!(messages[0].provider_id, "provider-v1");
        assert_eq!(messages[0].model_id, "shared-model");
        assert_eq!(messages[0].tokens.input, 7);
    }

    #[test]
    fn reads_v2_only_usage_and_preserves_incomplete_state() {
        let database = FixtureDatabase::new(false, true);
        let connection = database.write();
        insert_v2_session(&connection, "session-v2", 20);
        insert_v2_message(&connection, "message-v2", "session-v2", 21, 9, false);
        drop(connection);

        let mut store = OpenCodeStore::open_read_only(&database.path).expect("store");
        assert!(!store.capabilities().has_v1());
        assert!(store.capabilities().has_v2());
        let snapshot = store.begin_snapshot().expect("snapshot");
        let messages = snapshot
            .read_messages_page("session-v2", None, page_size(10))
            .expect("messages");

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].generation, OpenCodeGeneration::V2);
        assert_eq!(messages[0].completed_at_ms, None);
        assert_eq!(messages[0].provider_id, "provider-v2");
    }

    #[test]
    fn error_only_page_advances_cursor_and_keeps_later_exact_usage() {
        let database = FixtureDatabase::new(false, true);
        let connection = database.write();
        insert_v2_session(&connection, "session-v2", 20);
        let error_payload = json!({
            "model": {"providerID": "provider-v2", "id": "shared-model"},
            "time": {"created": 21, "completed": 22},
            "error": {"name": "ProviderError"}
        })
        .to_string();
        connection
            .execute(
                "INSERT INTO session_message
                    (id, session_id, type, seq, time_created, time_updated, data)
                 VALUES ('message-a-error', 'session-v2', 'assistant', 1, 21, 22, ?1)",
                [error_payload],
            )
            .expect("error message");
        insert_v2_message(&connection, "message-b-usage", "session-v2", 23, 9, true);
        drop(connection);

        let mut store = OpenCodeStore::open_read_only(&database.path).expect("store");
        let snapshot = store.begin_snapshot().expect("snapshot");
        let first = snapshot
            .read_messages_page("session-v2", None, page_size(1))
            .expect("error page");
        let second = snapshot
            .read_messages_page("session-v2", first.last_row_id.as_deref(), page_size(1))
            .expect("usage page");

        assert!(first.has_rows());
        assert!(first.messages.is_empty());
        assert_eq!(first.non_usage_error_rows, 1);
        assert_eq!(second.messages.len(), 1);
        assert_eq!(second.messages[0].id, "message-b-usage");
    }

    #[test]
    fn missing_usage_without_structured_error_remains_incompatible() {
        let database = FixtureDatabase::new(false, true);
        let connection = database.write();
        insert_v2_session(&connection, "session-v2", 20);
        let payload = json!({
            "model": {"providerID": "provider-v2", "id": "shared-model"},
            "time": {"created": 21, "completed": 22}
        })
        .to_string();
        connection
            .execute(
                "INSERT INTO session_message
                    (id, session_id, type, seq, time_created, time_updated, data)
                 VALUES ('message-invalid', 'session-v2', 'assistant', 1, 21, 22, ?1)",
                [payload],
            )
            .expect("message");
        drop(connection);

        let mut store = OpenCodeStore::open_read_only(&database.path).expect("store");
        let snapshot = store.begin_snapshot().expect("snapshot");
        let error = snapshot
            .read_messages_page("session-v2", None, page_size(10))
            .expect_err("missing usage remains incompatible");

        assert!(matches!(error, OpenCodeStoreError::Incompatible));
    }

    #[test]
    fn combined_schema_prefers_v2_and_keeps_v1_only_rows_across_pages() {
        let database = FixtureDatabase::new(true, true);
        let connection = database.write();
        insert_v1_session(&connection, "session-legacy", 10);
        insert_v1_session(&connection, "session-shared", 20);
        insert_v2_session(&connection, "session-modern", 30);
        insert_v2_session(&connection, "session-shared", 40);
        insert_v1_message(&connection, "message-legacy", "session-shared", 21, 3);
        insert_v1_message(&connection, "message-overlap", "session-shared", 22, 4);
        insert_v2_message(
            &connection,
            "message-overlap",
            "session-shared",
            23,
            40,
            true,
        );
        insert_v2_message(&connection, "message-v2", "session-shared", 24, 5, true);
        drop(connection);

        let mut store = OpenCodeStore::open_read_only(&database.path).expect("store");
        let snapshot = store.begin_snapshot().expect("snapshot");

        let first_sessions = snapshot
            .read_sessions_page(None, page_size(2))
            .expect("first sessions");
        let second_sessions = snapshot
            .read_sessions_page(
                first_sessions.last().map(|session| session.id.as_str()),
                page_size(2),
            )
            .expect("second sessions");
        let sessions = first_sessions
            .into_iter()
            .chain(second_sessions)
            .collect::<Vec<_>>();
        assert_eq!(
            sessions
                .iter()
                .map(|session| session.id.as_str())
                .collect::<Vec<_>>(),
            ["session-legacy", "session-modern", "session-shared"]
        );
        assert_eq!(sessions[2].generation, OpenCodeGeneration::V2);
        assert_eq!(sessions[2].tokens.input, 40);

        let first_messages = snapshot
            .read_messages_page("session-shared", None, page_size(2))
            .expect("first messages");
        let second_messages = snapshot
            .read_messages_page(
                "session-shared",
                first_messages.last().map(|message| message.id.as_str()),
                page_size(2),
            )
            .expect("second messages");
        let messages = first_messages
            .into_iter()
            .chain(second_messages)
            .collect::<Vec<_>>();
        assert_eq!(
            messages
                .iter()
                .map(|message| message.id.as_str())
                .collect::<Vec<_>>(),
            ["message-legacy", "message-overlap", "message-v2"]
        );
        assert_eq!(messages[0].generation, OpenCodeGeneration::V1);
        assert_eq!(messages[1].generation, OpenCodeGeneration::V2);
        assert_eq!(messages[1].tokens.input, 40);
    }

    #[test]
    fn mixed_generation_exposes_v1_only_and_never_reads_residual_v2() {
        let database = FixtureDatabase::new(true, true);
        let connection = database.write();
        insert_v1_session(&connection, "session-v1", 10);
        insert_v1_message(&connection, "message-v1", "session-v1", 11, 7);
        insert_v2_message(&connection, "residual-v2", "session-v1", 12, 9, true);
        drop(connection);
        // Reproduce the reported production shape: `session_message` exists
        // while `session_v2` does not.
        database
            .write()
            .execute("DROP TABLE session_v2", [])
            .expect("drop V2 session table");

        let mut store = OpenCodeStore::open_read_only(&database.path).expect("store");
        assert!(store.capabilities().has_v1());
        assert!(!store.capabilities().has_v2());
        assert_eq!(
            store.capabilities().ignored_generation(),
            Some(OpenCodeGeneration::V2)
        );
        let snapshot = store.begin_snapshot().expect("snapshot");
        let sessions = snapshot
            .read_sessions_page(None, page_size(10))
            .expect("sessions");
        let messages = snapshot
            .read_messages_page("session-v1", None, page_size(10))
            .expect("messages");

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].generation, OpenCodeGeneration::V1);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].id, "message-v1");
        assert_eq!(messages[0].generation, OpenCodeGeneration::V1);
    }

    #[test]
    fn no_complete_generation_fails_as_schema_error() {
        let database = FixtureDatabase::new(true, true);
        let connection = database.write();
        connection
            .execute("DROP TABLE session_v2", [])
            .expect("drop V2 session table");
        connection
            .execute("DROP TABLE message", [])
            .expect("drop V1 detail table");
        drop(connection);

        let error = OpenCodeStore::open_read_only(&database.path).expect_err("no complete schema");

        assert!(matches!(error, OpenCodeStoreError::Schema(_)));
        assert_eq!(
            error.source_failure_code(),
            CollectorFailureCode::IncompatibleEnvelope
        );
    }

    #[test]
    fn missing_database_classifies_as_invalid_location() {
        let directory = TempDir::new().expect("directory");
        let missing_path = directory.path().join("missing.db");

        let error = OpenCodeStore::open_read_only(&missing_path).expect_err("missing database");

        assert!(matches!(error, OpenCodeStoreError::Open(_)));
        assert_eq!(
            open_failure_code(&missing_path),
            CollectorFailureCode::SourceInvalidLocation
        );
    }

    #[test]
    fn permission_denied_open_classifies_as_permission_denied() {
        #[cfg(unix)]
        use std::os::unix::fs::PermissionsExt;

        let directory = TempDir::new().expect("directory");
        let path = directory.path().join("opencode.db");
        {
            let connection = Connection::open(&path).expect("database");
            connection
                .execute("CREATE TABLE session (id TEXT)", [])
                .expect("schema");
        }
        #[cfg(unix)]
        {
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000))
                .expect("remove permissions");
        }

        let error = OpenCodeStore::open_read_only(&path).expect_err("permission denied");

        assert!(matches!(error, OpenCodeStoreError::Open(_)));
        assert_eq!(
            open_failure_code(&path),
            CollectorFailureCode::SourcePermissionDenied
        );
        #[cfg(unix)]
        {
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
                .expect("restore permissions");
        }
    }

    #[test]
    fn rejects_negative_usage_without_exposing_source_values() {
        let database = FixtureDatabase::new(false, true);
        let connection = database.write();
        insert_v2_session(&connection, "session-v2", 20);
        let payload = v2_payload(21, -1, true).to_string();
        connection
            .execute(
                "INSERT INTO session_message
                    (id, session_id, type, seq, time_created, time_updated, data)
                 VALUES ('message-negative', 'session-v2', 'assistant', 1, 21, 21, ?1)",
                [&payload],
            )
            .expect("message");
        drop(connection);

        let mut store = OpenCodeStore::open_read_only(&database.path).expect("store");
        let snapshot = store.begin_snapshot().expect("snapshot");
        let error = snapshot
            .read_messages_page("session-v2", None, page_size(10))
            .expect_err("negative token");

        assert!(matches!(error, OpenCodeStoreError::Incompatible));
        assert_eq!(error.to_string(), "OpenCode usage row is incompatible");
    }

    #[test]
    fn rejects_invalid_identity_timestamp_and_cost_without_echoing_values() {
        let invalid_session = RawSessionHeader {
            id: " ".to_owned(),
            generation: 2,
            created_at_ms: -1,
            updated_at_ms: 0,
            idle_at_ms: None,
            input: 0,
            output: 0,
            reasoning: 0,
            cache_read: 0,
            cache_write: 0,
            cost_usd: f64::NAN,
        };
        let session_error =
            OpenCodeSessionHeader::try_from(invalid_session).expect_err("invalid session fields");

        let invalid_message = RawMessageUsage {
            id: "message-private".to_owned(),
            session_id: "session-private".to_owned(),
            generation: 2,
            created_at_ms: Some(-1),
            completed_at_ms: None,
            provider_id: Some(" ".to_owned()),
            model_id: Some("model-private".to_owned()),
            input: Some(0),
            output: Some(0),
            reasoning: Some(0),
            cache_read: Some(0),
            cache_write: Some(0),
            cost_usd: Some(f64::INFINITY),
            has_error_object: false,
        };
        let message_error =
            OpenCodeMessageUsage::try_from(invalid_message).expect_err("invalid message fields");

        for error in [session_error, message_error] {
            assert!(matches!(error, OpenCodeStoreError::Incompatible));
            assert_eq!(error.to_string(), "OpenCode usage row is incompatible");
        }
    }

    #[test]
    fn privacy_projection_does_not_return_content_bearing_fields() {
        let database = FixtureDatabase::new(false, true);
        let connection = database.write();
        insert_v2_session(&connection, "session-private", 20);
        let mut payload = v2_payload(21, 9, true);
        payload["content"] = json!({
            "prompt": "PRIVATE_PROMPT_SENTINEL",
            "response": "PRIVATE_RESPONSE_SENTINEL",
            "tool": "PRIVATE_TOOL_SENTINEL"
        });
        let payload = payload.to_string();
        connection
            .execute(
                "INSERT INTO session_message
                    (id, session_id, type, seq, time_created, time_updated, data)
                 VALUES ('message-private', 'session-private', 'assistant', 1, 21, 21, ?1)",
                [&payload],
            )
            .expect("message");
        drop(connection);

        let mut store = OpenCodeStore::open_read_only(&database.path).expect("store");
        let snapshot = store.begin_snapshot().expect("snapshot");
        let messages = snapshot
            .read_messages_page("session-private", None, page_size(10))
            .expect("messages");
        let debug = format!("{messages:?}");

        assert_eq!(messages.len(), 1);
        assert!(!debug.contains("PRIVATE_PROMPT_SENTINEL"));
        assert!(!debug.contains("PRIVATE_RESPONSE_SENTINEL"));
        assert!(!debug.contains("PRIVATE_TOOL_SENTINEL"));
        for sql in [V1_MESSAGE_QUERY, V2_MESSAGE_QUERY, COMBINED_MESSAGE_QUERY] {
            assert!(!sql.contains("$.content"));
            assert!(!sql.contains("$.title"));
            assert!(!sql.contains("$.directory"));
            let projection = sql.split("FROM").next().expect("projection");
            assert!(!projection.lines().any(|line| line.trim() == "data,"));
        }
    }

    #[test]
    fn connection_is_query_only_and_does_not_create_missing_database() {
        let missing_dir = TempDir::new().expect("directory");
        let missing_path = missing_dir.path().join("missing.db");
        let error = OpenCodeStore::open_read_only(&missing_path).expect_err("missing");
        assert!(matches!(error, OpenCodeStoreError::Open(_)));
        assert!(!missing_path.exists());

        let database = FixtureDatabase::new(true, false);
        let store = OpenCodeStore::open_read_only(&database.path).expect("store");
        let query_only: i64 = store
            .connection
            .pragma_query_value(None, "query_only", |row| row.get(0))
            .expect("query_only");
        assert_eq!(query_only, 1);
        let write_error = store
            .connection
            .execute("INSERT INTO session (id) VALUES ('forbidden')", [])
            .expect_err("read-only write");
        assert!(matches!(
            write_error.sqlite_error_code(),
            Some(rusqlite::ErrorCode::ReadOnly)
        ));
    }

    #[test]
    fn read_snapshot_is_stable_while_wal_writer_commits() {
        let database = FixtureDatabase::new(false, true);
        let writer = database.write();
        writer
            .pragma_update(None, "journal_mode", "WAL")
            .expect("WAL");
        insert_v2_session(&writer, "session-a", 10);

        let mut store = OpenCodeStore::open_read_only(&database.path).expect("store");
        let snapshot = store.begin_snapshot().expect("snapshot");
        let before = snapshot
            .read_sessions_page(None, page_size(10))
            .expect("before");
        insert_v2_session(&writer, "session-b", 20);
        let same_snapshot = snapshot
            .read_sessions_page(None, page_size(10))
            .expect("same snapshot");
        assert_eq!(before.len(), 1);
        assert_eq!(same_snapshot.len(), 1);
        drop(snapshot);

        let next_snapshot = store.begin_snapshot().expect("next snapshot");
        let after = next_snapshot
            .read_sessions_page(None, page_size(10))
            .expect("after");
        assert_eq!(after.len(), 2);
    }

    #[test]
    fn invalid_cursors_fail_before_querying() {
        let database = FixtureDatabase::new(true, false);
        let mut store = OpenCodeStore::open_read_only(&database.path).expect("store");
        let snapshot = store.begin_snapshot().expect("snapshot");

        assert!(matches!(
            snapshot.read_sessions_page(Some(" "), page_size(1)),
            Err(OpenCodeStoreError::InvalidCursor)
        ));
        assert!(matches!(
            snapshot.read_messages_page("", None, page_size(1)),
            Err(OpenCodeStoreError::InvalidCursor)
        ));
    }

    fn page_size(value: usize) -> OpenCodePageSize {
        OpenCodePageSize::new(value).expect("page size")
    }

    struct FixtureDatabase {
        _directory: TempDir,
        path: std::path::PathBuf,
    }

    impl FixtureDatabase {
        fn new(v1: bool, v2: bool) -> Self {
            let directory = TempDir::new().expect("directory");
            let path = directory.path().join("opencode.db");
            let connection = Connection::open(&path).expect("database");
            if v1 {
                create_v1_schema(&connection);
            }
            if v2 {
                create_v2_schema(&connection);
            }
            drop(connection);
            Self {
                _directory: directory,
                path,
            }
        }

        fn write(&self) -> Connection {
            Connection::open(&self.path).expect("writable database")
        }
    }

    fn create_v1_schema(connection: &Connection) {
        connection
            .execute_batch(
                "CREATE TABLE session (
                    id TEXT PRIMARY KEY,
                    cost REAL NOT NULL DEFAULT 0,
                    tokens_input INTEGER NOT NULL DEFAULT 0,
                    tokens_output INTEGER NOT NULL DEFAULT 0,
                    tokens_reasoning INTEGER NOT NULL DEFAULT 0,
                    tokens_cache_read INTEGER NOT NULL DEFAULT 0,
                    tokens_cache_write INTEGER NOT NULL DEFAULT 0,
                    time_created INTEGER NOT NULL,
                    time_updated INTEGER NOT NULL,
                    title TEXT NOT NULL DEFAULT 'PRIVATE_TITLE_SENTINEL',
                    directory TEXT NOT NULL DEFAULT 'PRIVATE_DIRECTORY_SENTINEL'
                );
                CREATE TABLE message (
                    id TEXT PRIMARY KEY,
                    session_id TEXT NOT NULL,
                    time_created INTEGER NOT NULL,
                    time_updated INTEGER NOT NULL,
                    data TEXT NOT NULL
                );",
            )
            .expect("V1 schema");
    }

    fn create_v2_schema(connection: &Connection) {
        connection
            .execute_batch(
                "CREATE TABLE session_v2 (
                    id TEXT PRIMARY KEY,
                    cost REAL NOT NULL DEFAULT 0,
                    tokens_input INTEGER NOT NULL DEFAULT 0,
                    tokens_output INTEGER NOT NULL DEFAULT 0,
                    tokens_reasoning INTEGER NOT NULL DEFAULT 0,
                    tokens_cache_read INTEGER NOT NULL DEFAULT 0,
                    tokens_cache_write INTEGER NOT NULL DEFAULT 0,
                    time_created INTEGER NOT NULL,
                    time_updated INTEGER NOT NULL,
                    time_idle INTEGER,
                    title TEXT,
                    directory TEXT NOT NULL DEFAULT 'PRIVATE_DIRECTORY_SENTINEL'
                );
                CREATE TABLE session_message (
                    id TEXT PRIMARY KEY,
                    session_id TEXT NOT NULL,
                    type TEXT NOT NULL,
                    seq INTEGER NOT NULL,
                    time_created INTEGER NOT NULL,
                    time_updated INTEGER NOT NULL,
                    data TEXT NOT NULL
                );",
            )
            .expect("V2 schema");
    }

    fn insert_v1_session(connection: &Connection, id: &str, input: i64) {
        connection
            .execute(
                "INSERT INTO session (
                    id, cost, tokens_input, tokens_output, tokens_reasoning,
                    tokens_cache_read, tokens_cache_write, time_created, time_updated
                 ) VALUES (?1, 1.5, ?2, 2, 3, 4, 5, 100, 200)",
                params![id, input],
            )
            .expect("V1 session");
    }

    fn insert_v2_session(connection: &Connection, id: &str, input: i64) {
        connection
            .execute(
                "INSERT INTO session_v2 (
                    id, cost, tokens_input, tokens_output, tokens_reasoning,
                    tokens_cache_read, tokens_cache_write, time_created, time_updated, time_idle
                 ) VALUES (?1, 2.5, ?2, 2, 3, 4, 5, 100, 200, 210)",
                params![id, input],
            )
            .expect("V2 session");
    }

    fn insert_v1_message(
        connection: &Connection,
        id: &str,
        session_id: &str,
        created_at_ms: i64,
        input: i64,
    ) {
        let payload = json!({
            "role": "assistant",
            "providerID": "provider-v1",
            "modelID": "shared-model",
            "time": {"created": created_at_ms, "completed": created_at_ms + 1},
            "tokens": {
                "input": input,
                "output": 2,
                "reasoning": 3,
                "cache": {"read": 4, "write": 5}
            },
            "cost": 0.25,
            "content": {"prompt": "PRIVATE_PROMPT_SENTINEL"}
        })
        .to_string();
        connection
            .execute(
                "INSERT INTO message (id, session_id, time_created, time_updated, data)
                 VALUES (?1, ?2, ?3, ?3, ?4)",
                params![id, session_id, created_at_ms, payload],
            )
            .expect("V1 message");
    }

    fn insert_v2_message(
        connection: &Connection,
        id: &str,
        session_id: &str,
        created_at_ms: i64,
        input: i64,
        completed: bool,
    ) {
        let payload = v2_payload(created_at_ms, input, completed).to_string();
        connection
            .execute(
                "INSERT INTO session_message
                    (id, session_id, type, seq, time_created, time_updated, data)
                 VALUES (?1, ?2, 'assistant', ?3, ?3, ?3, ?4)",
                params![id, session_id, created_at_ms, payload],
            )
            .expect("V2 message");
    }

    fn v2_payload(created_at_ms: i64, input: i64, completed: bool) -> serde_json::Value {
        let mut time = json!({"created": created_at_ms});
        if completed {
            time["completed"] = json!(created_at_ms + 1);
        }
        json!({
            "model": {
                "providerID": "provider-v2",
                "id": "shared-model",
                "variant": "high"
            },
            "time": time,
            "tokens": {
                "input": input,
                "output": 2,
                "reasoning": 3,
                "cache": {"read": 4, "write": 5}
            },
            "cost": 0.5,
            "content": {"response": "PRIVATE_RESPONSE_SENTINEL"}
        })
    }
}
