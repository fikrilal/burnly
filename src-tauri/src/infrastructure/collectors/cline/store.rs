use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde::Deserialize;
use thiserror::Error;

use super::super::support::open_external_read_only;
use super::schema::{verify_sessions_schema, ClineSchemaError};
use super::ClineUsageMetrics;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClineSessionRow {
    pub session_id: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub status: String,
    pub provider: String,
    pub model: String,
    pub cwd: String,
    pub workspace_root: String,
    pub usage: Option<ClineUsageMetrics>,
    pub aggregate_usage: Option<ClineUsageMetrics>,
    pub messages_path: PathBuf,
    pub updated_at: String,
}

#[derive(Debug, Error)]
pub(crate) enum ClineStoreError {
    #[error("cline database could not be opened")]
    Open(#[source] rusqlite::Error),
    #[error("cline database schema is incompatible")]
    Schema(#[source] ClineSchemaError),
    #[error("cline sessions could not be read")]
    Query(#[source] rusqlite::Error),
    #[error("cline session metadata is incompatible")]
    Metadata(#[source] serde_json::Error),
    #[error("cline session row is incompatible")]
    Incompatible,
}

#[derive(Debug)]
pub(crate) struct ClineStore {
    connection: Connection,
}

impl ClineStore {
    pub(crate) fn open_read_only(path: impl AsRef<Path>) -> Result<Self, ClineStoreError> {
        let connection = open_external_read_only(path).map_err(ClineStoreError::Open)?;
        verify_sessions_schema(&connection).map_err(ClineStoreError::Schema)?;
        Ok(Self { connection })
    }

    pub(crate) fn read_sessions(&self) -> Result<Vec<ClineSessionRow>, ClineStoreError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT
                    session_id,
                    started_at,
                    ended_at,
                    status,
                    provider,
                    model,
                    cwd,
                    workspace_root,
                    metadata_json,
                    messages_path,
                    updated_at
                FROM sessions
                ORDER BY started_at, session_id",
            )
            .map_err(ClineStoreError::Query)?;

        let rows = statement
            .query_map([], |row| {
                Ok(RawSessionRow {
                    session_id: row.get(0)?,
                    started_at: row.get(1)?,
                    ended_at: row.get(2)?,
                    status: row.get(3)?,
                    provider: row.get(4)?,
                    model: row.get(5)?,
                    cwd: row.get(6)?,
                    workspace_root: row.get(7)?,
                    metadata_json: row.get(8)?,
                    messages_path: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            })
            .map_err(ClineStoreError::Query)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(ClineStoreError::Query)?;

        rows.into_iter().map(ClineSessionRow::try_from).collect()
    }
}

struct RawSessionRow {
    session_id: String,
    started_at: String,
    ended_at: Option<String>,
    status: String,
    provider: String,
    model: String,
    cwd: String,
    workspace_root: String,
    metadata_json: String,
    messages_path: String,
    updated_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Metadata {
    usage: Option<RawUsage>,
    aggregate_usage: Option<RawUsage>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawUsage {
    input_tokens: i64,
    output_tokens: i64,
    cache_read_tokens: i64,
    cache_write_tokens: i64,
    total_cost: f64,
}

impl TryFrom<RawSessionRow> for ClineSessionRow {
    type Error = ClineStoreError;

    fn try_from(value: RawSessionRow) -> Result<Self, Self::Error> {
        if value.session_id.trim().is_empty()
            || value.started_at.trim().is_empty()
            || value.status.trim().is_empty()
            || value.provider.trim().is_empty()
            || value.model.trim().is_empty()
            || value.messages_path.trim().is_empty()
            || value.updated_at.trim().is_empty()
        {
            return Err(ClineStoreError::Incompatible);
        }

        let metadata = serde_json::from_str::<Metadata>(&value.metadata_json)
            .map_err(ClineStoreError::Metadata)?;

        Ok(Self {
            session_id: value.session_id,
            started_at: value.started_at,
            ended_at: value.ended_at,
            status: value.status,
            provider: value.provider,
            model: value.model,
            cwd: value.cwd,
            workspace_root: value.workspace_root,
            usage: metadata
                .usage
                .map(ClineUsageMetrics::try_from)
                .transpose()?,
            aggregate_usage: metadata
                .aggregate_usage
                .map(ClineUsageMetrics::try_from)
                .transpose()?,
            messages_path: PathBuf::from(value.messages_path),
            updated_at: value.updated_at,
        })
    }
}

impl TryFrom<RawUsage> for ClineUsageMetrics {
    type Error = ClineStoreError;

    fn try_from(value: RawUsage) -> Result<Self, Self::Error> {
        if value.input_tokens < 0
            || value.output_tokens < 0
            || value.cache_read_tokens < 0
            || value.cache_write_tokens < 0
            || !value.total_cost.is_finite()
            || value.total_cost < 0.0
        {
            return Err(ClineStoreError::Incompatible);
        }

        Ok(Self {
            input_tokens: value.input_tokens as u64,
            output_tokens: value.output_tokens as u64,
            cache_read_tokens: value.cache_read_tokens as u64,
            cache_write_tokens: value.cache_write_tokens as u64,
            cost_micros: cost_micros(value.total_cost)?,
        })
    }
}

fn cost_micros(cost: f64) -> Result<u64, ClineStoreError> {
    let micros = cost * 1_000_000.0;
    if !micros.is_finite() || micros < 0.0 || micros > u64::MAX as f64 {
        return Err(ClineStoreError::Incompatible);
    }
    Ok(micros.round() as u64)
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;
    use tempfile::NamedTempFile;

    use super::*;

    #[test]
    fn reads_usage_safe_session_rows_from_read_only_database() {
        let database = fixture_database();
        let metadata_json = metadata_from_session_fixture(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tests/fixtures/collectors/cline/sessions/valid-session.json"
        )));
        seed_session(database.path(), &metadata_json);

        let store = ClineStore::open_read_only(database.path()).expect("store");
        let sessions = store.read_sessions().expect("sessions");

        assert_eq!(sessions.len(), 1);
        let session = &sessions[0];
        assert_eq!(session.session_id, "cline-session-1");
        assert_eq!(session.provider, "cline-pass");
        assert_eq!(session.model, "cline-pass/glm-5.2");
        assert_eq!(
            session.messages_path,
            PathBuf::from("/fixture/cline-session-1.messages.json")
        );
        assert_eq!(
            session.usage,
            Some(ClineUsageMetrics {
                input_tokens: 12_000,
                output_tokens: 800,
                cache_read_tokens: 3_000,
                cache_write_tokens: 0,
                cost_micros: 11_500,
            })
        );
        assert_eq!(session.aggregate_usage, session.usage);
    }

    #[test]
    fn empty_sessions_table_is_valid() {
        let database = fixture_database();

        let store = ClineStore::open_read_only(database.path()).expect("store");

        assert!(store.read_sessions().expect("sessions").is_empty());
    }

    #[test]
    fn rejects_incompatible_schema() {
        let database = NamedTempFile::new().expect("temp database");
        let connection = Connection::open(database.path()).expect("database");
        connection
            .execute("CREATE TABLE sessions (session_id TEXT)", [])
            .expect("create incompatible schema");
        drop(connection);

        let error = ClineStore::open_read_only(database.path()).expect_err("schema error");

        assert!(matches!(
            error,
            ClineStoreError::Schema(ClineSchemaError::MissingColumn("started_at"))
        ));
    }

    #[test]
    fn rejects_invalid_metadata_usage() {
        let database = fixture_database();
        seed_session(
            database.path(),
            r#"{
              "usage": {
                "inputTokens": -1,
                "outputTokens": 0,
                "cacheReadTokens": 0,
                "cacheWriteTokens": 0,
                "totalCost": 0
              }
            }"#,
        );

        let store = ClineStore::open_read_only(database.path()).expect("store");
        let error = store.read_sessions().expect_err("invalid usage");

        assert!(matches!(error, ClineStoreError::Incompatible));
    }

    fn fixture_database() -> NamedTempFile {
        let database = NamedTempFile::new().expect("temp database");
        create_schema(database.path());
        database
    }

    fn create_schema(path: &Path) {
        let connection = Connection::open(path).expect("database");
        connection
            .execute(
                "CREATE TABLE sessions (
                    session_id TEXT PRIMARY KEY,
                    started_at TEXT NOT NULL,
                    ended_at TEXT,
                    status TEXT NOT NULL,
                    provider TEXT NOT NULL,
                    model TEXT NOT NULL,
                    cwd TEXT NOT NULL,
                    workspace_root TEXT NOT NULL,
                    metadata_json TEXT,
                    messages_path TEXT,
                    updated_at TEXT NOT NULL
                )",
                [],
            )
            .expect("schema");
    }

    fn seed_session(path: &Path, metadata_json: &str) {
        let connection = Connection::open(path).expect("database");
        connection
            .execute(
                "INSERT INTO sessions (
                    session_id,
                    started_at,
                    ended_at,
                    status,
                    provider,
                    model,
                    cwd,
                    workspace_root,
                    metadata_json,
                    messages_path,
                    updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                (
                    "cline-session-1",
                    "2026-06-30T01:15:00.000Z",
                    Some("2026-06-30T01:25:00.000Z"),
                    "idle",
                    "cline-pass",
                    "cline-pass/glm-5.2",
                    "/fixture/workspace",
                    "/fixture/workspace",
                    metadata_json,
                    "/fixture/cline-session-1.messages.json",
                    "2026-06-30T01:25:00.000Z",
                ),
            )
            .expect("seed session");
    }

    fn metadata_from_session_fixture(input: &str) -> String {
        let value = serde_json::from_str::<serde_json::Value>(input).expect("session fixture");
        value.get("metadata").expect("metadata").to_string()
    }
}
