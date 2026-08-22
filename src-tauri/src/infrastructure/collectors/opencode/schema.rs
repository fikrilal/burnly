//! Strict OpenCode V1/V2 schema capability detection.

use std::collections::HashSet;

use rusqlite::Connection;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OpenCodeGeneration {
    V1,
    V2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OpenCodeSchemaCapabilities {
    v1: bool,
    v2: bool,
}

impl OpenCodeSchemaCapabilities {
    pub(crate) const fn has_v1(self) -> bool {
        self.v1
    }

    pub(crate) const fn has_v2(self) -> bool {
        self.v2
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub(crate) enum OpenCodeSchemaError {
    #[error("OpenCode database contains no supported usage schema")]
    Unsupported,
    #[error("OpenCode {generation:?} database schema is incomplete")]
    IncompleteGeneration { generation: OpenCodeGeneration },
    #[error("OpenCode {generation:?} table {table} is missing required column {column}")]
    MissingColumn {
        generation: OpenCodeGeneration,
        table: &'static str,
        column: &'static str,
    },
    #[error("OpenCode database schema could not be inspected")]
    QueryFailed,
}

const V1_SESSION_COLUMNS: &[&str] = &[
    "id",
    "cost",
    "tokens_input",
    "tokens_output",
    "tokens_reasoning",
    "tokens_cache_read",
    "tokens_cache_write",
    "time_created",
    "time_updated",
];
const V1_MESSAGE_COLUMNS: &[&str] = &["id", "session_id", "time_created", "time_updated", "data"];
const V2_SESSION_COLUMNS: &[&str] = &[
    "id",
    "cost",
    "tokens_input",
    "tokens_output",
    "tokens_reasoning",
    "tokens_cache_read",
    "tokens_cache_write",
    "time_created",
    "time_updated",
    "time_idle",
];
const V2_MESSAGE_COLUMNS: &[&str] = &[
    "id",
    "session_id",
    "type",
    "seq",
    "time_created",
    "time_updated",
    "data",
];

pub(crate) fn inspect_schema(
    connection: &Connection,
) -> Result<OpenCodeSchemaCapabilities, OpenCodeSchemaError> {
    let tables = table_names(connection)?;
    let v1 = inspect_generation(
        connection,
        &tables,
        OpenCodeGeneration::V1,
        "session",
        V1_SESSION_COLUMNS,
        "message",
        V1_MESSAGE_COLUMNS,
    )?;
    let v2 = inspect_generation(
        connection,
        &tables,
        OpenCodeGeneration::V2,
        "session_v2",
        V2_SESSION_COLUMNS,
        "session_message",
        V2_MESSAGE_COLUMNS,
    )?;

    if !v1 && !v2 {
        return Err(OpenCodeSchemaError::Unsupported);
    }

    Ok(OpenCodeSchemaCapabilities { v1, v2 })
}

fn inspect_generation(
    connection: &Connection,
    tables: &HashSet<String>,
    generation: OpenCodeGeneration,
    session_table: &'static str,
    session_columns: &'static [&'static str],
    message_table: &'static str,
    message_columns: &'static [&'static str],
) -> Result<bool, OpenCodeSchemaError> {
    let has_session = tables.contains(session_table);
    let has_message = tables.contains(message_table);
    match (has_session, has_message) {
        (false, false) => Ok(false),
        (true, true) => {
            verify_columns(connection, generation, session_table, session_columns)?;
            verify_columns(connection, generation, message_table, message_columns)?;
            Ok(true)
        }
        _ => Err(OpenCodeSchemaError::IncompleteGeneration { generation }),
    }
}

fn table_names(connection: &Connection) -> Result<HashSet<String>, OpenCodeSchemaError> {
    let mut statement = connection
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table'")
        .map_err(|_| OpenCodeSchemaError::QueryFailed)?;
    let tables = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|_| OpenCodeSchemaError::QueryFailed)?
        .collect::<Result<HashSet<_>, _>>()
        .map_err(|_| OpenCodeSchemaError::QueryFailed)?;
    Ok(tables)
}

fn verify_columns(
    connection: &Connection,
    generation: OpenCodeGeneration,
    table: &'static str,
    required: &'static [&'static str],
) -> Result<(), OpenCodeSchemaError> {
    let sql = format!("PRAGMA table_info({table})");
    let mut statement = connection
        .prepare(&sql)
        .map_err(|_| OpenCodeSchemaError::QueryFailed)?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|_| OpenCodeSchemaError::QueryFailed)?
        .collect::<Result<HashSet<_>, _>>()
        .map_err(|_| OpenCodeSchemaError::QueryFailed)?;

    for column in required {
        if !columns.contains(*column) {
            return Err(OpenCodeSchemaError::MissingColumn {
                generation,
                table,
                column,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_database_without_supported_tables() {
        let connection = Connection::open_in_memory().expect("database");
        connection
            .execute("CREATE TABLE unrelated (id TEXT)", [])
            .expect("table");

        assert_eq!(
            inspect_schema(&connection),
            Err(OpenCodeSchemaError::Unsupported)
        );
    }

    #[test]
    fn rejects_partial_generation() {
        let connection = Connection::open_in_memory().expect("database");
        connection
            .execute("CREATE TABLE session (id TEXT)", [])
            .expect("table");

        assert_eq!(
            inspect_schema(&connection),
            Err(OpenCodeSchemaError::IncompleteGeneration {
                generation: OpenCodeGeneration::V1
            })
        );
    }

    #[test]
    fn reports_missing_required_column() {
        let connection = Connection::open_in_memory().expect("database");
        connection
            .execute_batch(
                "CREATE TABLE session (
                    id TEXT, cost REAL, tokens_input INTEGER,
                    tokens_output INTEGER, tokens_reasoning INTEGER,
                    tokens_cache_read INTEGER, tokens_cache_write INTEGER,
                    time_created INTEGER, time_updated INTEGER
                );
                CREATE TABLE message (
                    id TEXT, session_id TEXT, time_created INTEGER,
                    time_updated INTEGER
                );",
            )
            .expect("schema");

        assert_eq!(
            inspect_schema(&connection),
            Err(OpenCodeSchemaError::MissingColumn {
                generation: OpenCodeGeneration::V1,
                table: "message",
                column: "data"
            })
        );
    }
}
