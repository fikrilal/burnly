use std::collections::HashSet;

use rusqlite::Connection;
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub(crate) enum ZCodeSchemaError {
    #[error("zcode model_usage table is missing")]
    MissingModelUsageTable,
    #[error("zcode model_usage table is missing required column {0}")]
    MissingColumn(&'static str),
    #[error("zcode database schema could not be inspected")]
    QueryFailed,
}

const REQUIRED_MODEL_USAGE_COLUMNS: &[&str] = &[
    "id",
    "session_id",
    "turn_id",
    "query_source",
    "provider_id",
    "model_id",
    "status",
    "started_at",
    "completed_at",
    "input_tokens",
    "output_tokens",
    "reasoning_tokens",
    "cache_creation_input_tokens",
    "cache_read_input_tokens",
    "provider_total_tokens",
    "computed_total_tokens",
];

pub(crate) fn verify_model_usage_schema(connection: &Connection) -> Result<(), ZCodeSchemaError> {
    let exists: bool = connection
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM sqlite_master
                WHERE type = 'table' AND name = 'model_usage'
            )",
            [],
            |row| row.get(0),
        )
        .map_err(|_| ZCodeSchemaError::QueryFailed)?;
    if !exists {
        return Err(ZCodeSchemaError::MissingModelUsageTable);
    }

    let mut statement = connection
        .prepare("PRAGMA table_info(model_usage)")
        .map_err(|_| ZCodeSchemaError::QueryFailed)?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|_| ZCodeSchemaError::QueryFailed)?
        .collect::<Result<HashSet<_>, _>>()
        .map_err(|_| ZCodeSchemaError::QueryFailed)?;

    for column in REQUIRED_MODEL_USAGE_COLUMNS {
        if !columns.contains(*column) {
            return Err(ZCodeSchemaError::MissingColumn(column));
        }
    }

    Ok(())
}
