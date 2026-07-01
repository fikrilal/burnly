use std::collections::HashSet;

use rusqlite::Connection;
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub(crate) enum ClineSchemaError {
    #[error("cline sessions table is missing")]
    MissingSessionsTable,
    #[error("cline sessions table is missing required column {0}")]
    MissingColumn(&'static str),
    #[error("cline database schema could not be inspected")]
    QueryFailed,
}

const REQUIRED_SESSION_COLUMNS: &[&str] = &[
    "session_id",
    "started_at",
    "ended_at",
    "status",
    "provider",
    "model",
    "cwd",
    "workspace_root",
    "metadata_json",
    "messages_path",
    "updated_at",
];

pub(crate) fn verify_sessions_schema(connection: &Connection) -> Result<(), ClineSchemaError> {
    let exists: bool = connection
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM sqlite_master
                WHERE type = 'table' AND name = 'sessions'
            )",
            [],
            |row| row.get(0),
        )
        .map_err(|_| ClineSchemaError::QueryFailed)?;
    if !exists {
        return Err(ClineSchemaError::MissingSessionsTable);
    }

    let mut statement = connection
        .prepare("PRAGMA table_info(sessions)")
        .map_err(|_| ClineSchemaError::QueryFailed)?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|_| ClineSchemaError::QueryFailed)?
        .collect::<Result<HashSet<_>, _>>()
        .map_err(|_| ClineSchemaError::QueryFailed)?;

    for column in REQUIRED_SESSION_COLUMNS {
        if !columns.contains(*column) {
            return Err(ClineSchemaError::MissingColumn(column));
        }
    }

    Ok(())
}
