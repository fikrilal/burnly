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
pub(crate) enum OpenCodeGenerationState {
    Absent,
    Complete,
    Incomplete(OpenCodeSchemaReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OpenCodeSchemaReason {
    MissingSessionTable,
    MissingDetailTable,
    MissingRequiredColumn,
    SchemaQueryFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OpenCodeSchemaInspection {
    v1: OpenCodeGenerationState,
    v2: OpenCodeGenerationState,
}

impl OpenCodeSchemaInspection {
    pub(crate) const fn has_v1(self) -> bool {
        matches!(self.v1, OpenCodeGenerationState::Complete)
    }

    pub(crate) const fn has_v2(self) -> bool {
        matches!(self.v2, OpenCodeGenerationState::Complete)
    }

    pub(crate) const fn ignored_generation(self) -> Option<OpenCodeGeneration> {
        if self.has_v1() && !self.has_v2() {
            Some(OpenCodeGeneration::V2)
        } else if self.has_v2() && !self.has_v1() {
            Some(OpenCodeGeneration::V1)
        } else {
            None
        }
    }

    pub(crate) fn ignored_reason(self) -> Option<OpenCodeSchemaReason> {
        match self.ignored_generation() {
            Some(OpenCodeGeneration::V1) => match self.v1 {
                OpenCodeGenerationState::Incomplete(reason) => Some(reason),
                _ => None,
            },
            Some(OpenCodeGeneration::V2) => match self.v2 {
                OpenCodeGenerationState::Incomplete(reason) => Some(reason),
                _ => None,
            },
            None => None,
        }
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub(crate) enum OpenCodeSchemaError {
    #[error("OpenCode database contains no supported usage schema")]
    Unsupported,
    #[error("OpenCode database has no complete supported usage schema")]
    IncompatibleSchema,
    #[error("OpenCode database schema could not be inspected")]
    QueryFailed,
    #[error("OpenCode usage table is missing a required column")]
    MissingColumn,
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
) -> Result<OpenCodeSchemaInspection, OpenCodeSchemaError> {
    let tables = table_names(connection)?;
    let v1 = inspect_generation(
        connection,
        &tables,
        "session",
        V1_SESSION_COLUMNS,
        "message",
        V1_MESSAGE_COLUMNS,
    )?;
    let v2 = inspect_generation(
        connection,
        &tables,
        "session_v2",
        V2_SESSION_COLUMNS,
        "session_message",
        V2_MESSAGE_COLUMNS,
    )?;

    if v1 == OpenCodeGenerationState::Absent && v2 == OpenCodeGenerationState::Absent {
        return Err(OpenCodeSchemaError::Unsupported);
    }
    if !has_complete(v1) && !has_complete(v2) {
        return Err(OpenCodeSchemaError::IncompatibleSchema);
    }

    Ok(OpenCodeSchemaInspection { v1, v2 })
}

const fn has_complete(state: OpenCodeGenerationState) -> bool {
    matches!(state, OpenCodeGenerationState::Complete)
}

fn inspect_generation(
    connection: &Connection,
    tables: &HashSet<String>,
    session_table: &'static str,
    session_columns: &'static [&'static str],
    message_table: &'static str,
    message_columns: &'static [&'static str],
) -> Result<OpenCodeGenerationState, OpenCodeSchemaError> {
    let has_session = tables.contains(session_table);
    let has_message = tables.contains(message_table);
    match (has_session, has_message) {
        (false, false) => Ok(OpenCodeGenerationState::Absent),
        (true, true) => {
            match (
                verify_columns(connection, session_table, session_columns),
                verify_columns(connection, message_table, message_columns),
            ) {
                (Ok(()), Ok(())) => Ok(OpenCodeGenerationState::Complete),
                (Err(OpenCodeSchemaError::MissingColumn), _)
                | (_, Err(OpenCodeSchemaError::MissingColumn)) => {
                    Ok(OpenCodeGenerationState::Incomplete(
                        OpenCodeSchemaReason::MissingRequiredColumn,
                    ))
                }
                (Err(error), _) | (_, Err(error)) => Err(error),
            }
        }
        (false, true) => Ok(OpenCodeGenerationState::Incomplete(
            OpenCodeSchemaReason::MissingSessionTable,
        )),
        (true, false) => Ok(OpenCodeGenerationState::Incomplete(
            OpenCodeSchemaReason::MissingDetailTable,
        )),
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

/// Proves every detail row of the ignored generation is redundant: its
/// stable message IDs must all exist in the selected generation's detail
/// table. The stable message ID is the cross-generation deduplication key
/// (V2 wins on overlap), so an ID present in the selected table represents
/// the same usage. Returns true when any ignored row has no matching
/// selected row, which means ignoring would silently drop usage.
pub(crate) fn redundancy_exceeded(
    connection: &Connection,
    ignored_message_table: &'static str,
    selected_message_table: &'static str,
) -> Result<bool, OpenCodeSchemaError> {
    let sql = format!(
        "SELECT EXISTS(
            SELECT 1 FROM {ignored_message_table} i
            WHERE NOT EXISTS (
                SELECT 1 FROM {selected_message_table} s WHERE s.id = i.id
            )
        )"
    );
    connection
        .query_row(&sql, [], |row| row.get(0))
        .map_err(|_| OpenCodeSchemaError::QueryFailed)
}

/// Verifies required columns. A SQLite execution error is fatal
/// (`QueryFailed`); a successfully executed PRAGMA that reveals a missing
/// column is structural incompleteness (`MissingRequiredColumn`).
fn verify_columns(
    connection: &Connection,
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
            return Err(OpenCodeSchemaError::MissingColumn);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v1_schema() -> &'static str {
        "CREATE TABLE session (
            id TEXT, cost REAL, tokens_input INTEGER, tokens_output INTEGER,
            tokens_reasoning INTEGER, tokens_cache_read INTEGER,
            tokens_cache_write INTEGER, time_created INTEGER, time_updated INTEGER
        );
        CREATE TABLE message (
            id TEXT, session_id TEXT, time_created INTEGER,
            time_updated INTEGER, data TEXT
        );"
    }

    fn v2_schema() -> &'static str {
        "CREATE TABLE session_v2 (
            id TEXT, cost REAL, tokens_input INTEGER, tokens_output INTEGER,
            tokens_reasoning INTEGER, tokens_cache_read INTEGER,
            tokens_cache_write INTEGER, time_created INTEGER, time_updated INTEGER,
            time_idle INTEGER
        );
        CREATE TABLE session_message (
            id TEXT, session_id TEXT, type TEXT, seq INTEGER,
            time_created INTEGER, time_updated INTEGER, data TEXT
        );"
    }

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
    fn rejects_only_incomplete_generation() {
        let connection = Connection::open_in_memory().expect("database");
        connection
            .execute("CREATE TABLE session (id TEXT)", [])
            .expect("table");

        assert_eq!(
            inspect_schema(&connection),
            Err(OpenCodeSchemaError::IncompatibleSchema)
        );
    }

    #[test]
    fn accepts_v1_only() {
        let connection = Connection::open_in_memory().expect("database");
        connection.execute_batch(v1_schema()).expect("schema");

        let inspection = inspect_schema(&connection).expect("inspection");
        assert!(inspection.has_v1());
        assert!(!inspection.has_v2());
        assert_eq!(
            inspection.ignored_generation(),
            Some(OpenCodeGeneration::V2)
        );
    }

    #[test]
    fn accepts_v2_only() {
        let connection = Connection::open_in_memory().expect("database");
        connection.execute_batch(v2_schema()).expect("schema");

        let inspection = inspect_schema(&connection).expect("inspection");
        assert!(!inspection.has_v1());
        assert!(inspection.has_v2());
        assert_eq!(
            inspection.ignored_generation(),
            Some(OpenCodeGeneration::V1)
        );
    }

    #[test]
    fn accepts_both_complete_generations() {
        let connection = Connection::open_in_memory().expect("database");
        connection
            .execute_batch(&format!("{} {}", v1_schema(), v2_schema()))
            .expect("schema");

        let inspection = inspect_schema(&connection).expect("inspection");
        assert!(inspection.has_v1());
        assert!(inspection.has_v2());
        assert_eq!(inspection.ignored_generation(), None);
    }

    #[test]
    fn accepts_complete_v1_with_residual_session_message() {
        let connection = Connection::open_in_memory().expect("database");
        connection
            .execute_batch(&format!(
                "{} CREATE TABLE session_message (
                    id TEXT, session_id TEXT, type TEXT, seq INTEGER,
                    time_created INTEGER, time_updated INTEGER, data TEXT
                );",
                v1_schema()
            ))
            .expect("schema");
        // Insert a row that satisfies the residual table's constraints so the
        // production shape (V1 complete plus residual V2 detail) has content.
        connection
            .execute(
                "INSERT INTO session_message (
                    id, session_id, type, seq, time_created, time_updated, data
                ) VALUES ('residual-v2', 'session-x', 'assistant', 1, 100, 100, '{}')",
                [],
            )
            .expect("residual V2 message");

        let inspection = inspect_schema(&connection).expect("inspection");
        assert!(inspection.has_v1());
        assert!(!inspection.has_v2());
        assert_eq!(
            inspection.ignored_generation(),
            Some(OpenCodeGeneration::V2)
        );
        assert_eq!(
            inspection.ignored_reason(),
            Some(OpenCodeSchemaReason::MissingSessionTable)
        );
    }

    #[test]
    fn accepts_complete_v2_with_incomplete_v1() {
        let connection = Connection::open_in_memory().expect("database");
        connection
            .execute_batch(&format!("{} CREATE TABLE session (id TEXT);", v2_schema()))
            .expect("schema");

        let inspection = inspect_schema(&connection).expect("inspection");
        assert!(!inspection.has_v1());
        assert!(inspection.has_v2());
        assert_eq!(
            inspection.ignored_generation(),
            Some(OpenCodeGeneration::V1)
        );
        assert_eq!(
            inspection.ignored_reason(),
            Some(OpenCodeSchemaReason::MissingDetailTable)
        );
    }

    #[test]
    fn missing_required_column_makes_generation_incomplete() {
        let connection = Connection::open_in_memory().expect("database");
        connection
            .execute_batch(
                "CREATE TABLE session (
                    id TEXT, cost REAL, tokens_input INTEGER, tokens_output INTEGER,
                    tokens_reasoning INTEGER, tokens_cache_read INTEGER,
                    tokens_cache_write INTEGER, time_created INTEGER, time_updated INTEGER
                );
                CREATE TABLE message (
                    id TEXT, session_id TEXT, time_created INTEGER, time_updated INTEGER
                );
                CREATE TABLE session_v2 (
                    id TEXT, cost REAL, tokens_input INTEGER, tokens_output INTEGER,
                    tokens_reasoning INTEGER, tokens_cache_read INTEGER,
                    tokens_cache_write INTEGER, time_created INTEGER, time_updated INTEGER,
                    time_idle INTEGER
                );
                CREATE TABLE session_message (
                    id TEXT, session_id TEXT, type TEXT, seq INTEGER,
                    time_created INTEGER, time_updated INTEGER, data TEXT
                );",
            )
            .expect("schema");

        let inspection = inspect_schema(&connection).expect("inspection");
        assert!(!inspection.has_v1());
        assert!(inspection.has_v2());
        assert_eq!(
            inspection.ignored_generation(),
            Some(OpenCodeGeneration::V1)
        );
        assert_eq!(
            inspection.ignored_reason(),
            Some(OpenCodeSchemaReason::MissingRequiredColumn)
        );
    }

    #[test]
    fn missing_required_column_on_only_generation_is_incompatible_schema() {
        let connection = Connection::open_in_memory().expect("database");
        connection
            .execute_batch(
                "CREATE TABLE session (
                    id TEXT, cost REAL, tokens_input INTEGER, tokens_output INTEGER,
                    tokens_reasoning INTEGER, tokens_cache_read INTEGER,
                    tokens_cache_write INTEGER, time_created INTEGER, time_updated INTEGER
                );
                CREATE TABLE message (
                    id TEXT, session_id TEXT, time_created INTEGER, time_updated INTEGER
                );",
            )
            .expect("schema");

        assert_eq!(
            inspect_schema(&connection),
            Err(OpenCodeSchemaError::IncompatibleSchema)
        );
    }
}
