use std::path::Path;

use rusqlite::Connection;
use thiserror::Error;

use super::super::support::open_external_read_only;
use super::schema::{verify_model_usage_schema, ZCodeSchemaError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ZCodeModelUsageRow {
    pub id: String,
    pub session_id: String,
    pub turn_id: Option<String>,
    pub query_source: String,
    pub provider_id: String,
    pub model_id: String,
    pub status: String,
    pub started_at_ms: i64,
    pub completed_at_ms: Option<i64>,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_tokens: u64,
    pub cache_creation_input_tokens: u64,
    pub cache_read_input_tokens: u64,
    pub provider_total_tokens: Option<u64>,
    pub computed_total_tokens: u64,
}

#[derive(Debug, Error)]
pub(crate) enum ZCodeStoreError {
    #[error("zcode database could not be opened")]
    Open(#[source] rusqlite::Error),
    #[error("zcode database schema is incompatible")]
    Schema(#[source] ZCodeSchemaError),
    #[error("zcode model usage could not be read")]
    Query(#[source] rusqlite::Error),
    #[error("zcode model usage row is incompatible")]
    Incompatible,
}

#[derive(Debug)]
pub(crate) struct ZCodeStore {
    connection: Connection,
}

impl ZCodeStore {
    pub(crate) fn open_read_only(path: impl AsRef<Path>) -> Result<Self, ZCodeStoreError> {
        let connection = open_external_read_only(path).map_err(ZCodeStoreError::Open)?;
        verify_model_usage_schema(&connection).map_err(ZCodeStoreError::Schema)?;
        Ok(Self { connection })
    }

    pub(crate) fn read_model_usage_between(
        &self,
        start_inclusive_ms: i64,
        end_exclusive_ms: i64,
    ) -> Result<Vec<ZCodeModelUsageRow>, ZCodeStoreError> {
        if start_inclusive_ms >= end_exclusive_ms {
            return Ok(Vec::new());
        }

        let mut statement = self
            .connection
            .prepare(
                "SELECT
                    id,
                    session_id,
                    turn_id,
                    query_source,
                    provider_id,
                    model_id,
                    status,
                    started_at,
                    completed_at,
                    input_tokens,
                    output_tokens,
                    reasoning_tokens,
                    cache_creation_input_tokens,
                    cache_read_input_tokens,
                    provider_total_tokens,
                    computed_total_tokens
                FROM model_usage
                WHERE started_at >= ?1 AND started_at < ?2
                ORDER BY started_at ASC, id ASC",
            )
            .map_err(ZCodeStoreError::Query)?;

        let rows = statement
            .query_map([start_inclusive_ms, end_exclusive_ms], |row| {
                Ok(RawModelUsageRow {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    turn_id: row.get(2)?,
                    query_source: row.get(3)?,
                    provider_id: row.get(4)?,
                    model_id: row.get(5)?,
                    status: row.get(6)?,
                    started_at_ms: row.get(7)?,
                    completed_at_ms: row.get(8)?,
                    input_tokens: row.get(9)?,
                    output_tokens: row.get(10)?,
                    reasoning_tokens: row.get(11)?,
                    cache_creation_input_tokens: row.get(12)?,
                    cache_read_input_tokens: row.get(13)?,
                    provider_total_tokens: row.get(14)?,
                    computed_total_tokens: row.get(15)?,
                })
            })
            .map_err(ZCodeStoreError::Query)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(ZCodeStoreError::Query)?;

        rows.into_iter().map(ZCodeModelUsageRow::try_from).collect()
    }
}

struct RawModelUsageRow {
    id: String,
    session_id: String,
    turn_id: Option<String>,
    query_source: String,
    provider_id: String,
    model_id: String,
    status: String,
    started_at_ms: i64,
    completed_at_ms: Option<i64>,
    input_tokens: i64,
    output_tokens: i64,
    reasoning_tokens: i64,
    cache_creation_input_tokens: i64,
    cache_read_input_tokens: i64,
    provider_total_tokens: Option<i64>,
    computed_total_tokens: i64,
}

impl TryFrom<RawModelUsageRow> for ZCodeModelUsageRow {
    type Error = ZCodeStoreError;

    fn try_from(value: RawModelUsageRow) -> Result<Self, Self::Error> {
        if value.id.trim().is_empty()
            || value.session_id.trim().is_empty()
            || value.query_source.trim().is_empty()
            || value.provider_id.trim().is_empty()
            || value.model_id.trim().is_empty()
            || value.status.trim().is_empty()
            || value.started_at_ms < 0
            || value.completed_at_ms.is_some_and(|completed| completed < 0)
        {
            return Err(ZCodeStoreError::Incompatible);
        }

        Ok(Self {
            id: value.id,
            session_id: value.session_id,
            turn_id: value.turn_id.filter(|turn_id| !turn_id.trim().is_empty()),
            query_source: value.query_source,
            provider_id: value.provider_id,
            model_id: value.model_id,
            status: value.status,
            started_at_ms: value.started_at_ms,
            completed_at_ms: value.completed_at_ms,
            input_tokens: non_negative(value.input_tokens)?,
            output_tokens: non_negative(value.output_tokens)?,
            reasoning_tokens: non_negative(value.reasoning_tokens)?,
            cache_creation_input_tokens: non_negative(value.cache_creation_input_tokens)?,
            cache_read_input_tokens: non_negative(value.cache_read_input_tokens)?,
            provider_total_tokens: value.provider_total_tokens.map(non_negative).transpose()?,
            computed_total_tokens: non_negative(value.computed_total_tokens)?,
        })
    }
}

fn non_negative(value: i64) -> Result<u64, ZCodeStoreError> {
    u64::try_from(value).map_err(|_| ZCodeStoreError::Incompatible)
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;
    use tempfile::NamedTempFile;

    use super::*;

    #[test]
    fn reads_usage_safe_model_usage_rows_from_read_only_database() {
        let database = fixture_database(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tests/fixtures/collectors/zcode/db/valid.sql"
        )));

        let store = ZCodeStore::open_read_only(database.path()).expect("store");
        let rows = store
            .read_model_usage_between(1_782_952_270_000, 1_782_952_340_000)
            .expect("rows");

        assert_eq!(rows.len(), 3);
        assert_eq!(
            rows[0],
            ZCodeModelUsageRow {
                id: "usage-main-1".to_owned(),
                session_id: "sess-main".to_owned(),
                turn_id: Some("turn-main-1".to_owned()),
                query_source: "interactive".to_owned(),
                provider_id: "builtin:zai-start-plan".to_owned(),
                model_id: "GLM-5.2".to_owned(),
                status: "completed".to_owned(),
                started_at_ms: 1_782_952_270_000,
                completed_at_ms: Some(1_782_952_275_000),
                input_tokens: 8_488,
                output_tokens: 122,
                reasoning_tokens: 0,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 7_360,
                provider_total_tokens: Some(8_610),
                computed_total_tokens: 8_610,
            }
        );
        assert_eq!(rows[1].model_id, "GLM-5-Turbo");
        assert_eq!(rows[2].status, "running");
    }

    #[test]
    fn applies_requested_time_window() {
        let database = fixture_database(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tests/fixtures/collectors/zcode/db/valid.sql"
        )));
        let store = ZCodeStore::open_read_only(database.path()).expect("store");

        let rows = store
            .read_model_usage_between(1_782_952_300_000, 1_782_952_325_000)
            .expect("rows");

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "usage-subagent-1");
    }

    #[test]
    fn empty_model_usage_table_is_valid() {
        let database = fixture_database(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tests/fixtures/collectors/zcode/db/empty.sql"
        )));

        let store = ZCodeStore::open_read_only(database.path()).expect("store");

        assert!(store
            .read_model_usage_between(1, 2)
            .expect("rows")
            .is_empty());
    }

    #[test]
    fn rejects_missing_model_usage_table() {
        let database = fixture_database(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tests/fixtures/collectors/zcode/db/missing-model-usage.sql"
        )));

        let error = ZCodeStore::open_read_only(database.path()).expect_err("schema error");

        assert!(matches!(
            error,
            ZCodeStoreError::Schema(ZCodeSchemaError::MissingModelUsageTable)
        ));
    }

    #[test]
    fn rejects_incompatible_schema() {
        let database = fixture_database(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tests/fixtures/collectors/zcode/db/incompatible-schema.sql"
        )));

        let error = ZCodeStore::open_read_only(database.path()).expect_err("schema error");

        assert!(matches!(
            error,
            ZCodeStoreError::Schema(ZCodeSchemaError::MissingColumn("turn_id"))
        ));
    }

    #[test]
    fn rejects_negative_token_values() {
        let database = fixture_database(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tests/fixtures/collectors/zcode/db/invalid-negative.sql"
        )));
        let store = ZCodeStore::open_read_only(database.path()).expect("store");

        let error = store
            .read_model_usage_between(1_782_952_270_000, 1_782_952_280_000)
            .expect_err("invalid row");

        assert!(matches!(error, ZCodeStoreError::Incompatible));
    }

    fn fixture_database(sql: &str) -> NamedTempFile {
        let database = NamedTempFile::new().expect("temp database");
        let connection = Connection::open(database.path()).expect("database");
        connection.execute_batch(sql).expect("fixture schema");
        drop(connection);
        database
    }
}
