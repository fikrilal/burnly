use std::sync::Mutex;

use rusqlite::Error as SqliteError;

use crate::application::ports::diagnostics_store::{
    DatabaseDiagnosticRecord, DiagnosticsStore, DiagnosticsStoreError, SourceDiagnosticRecord,
};

use super::Database;

pub(crate) struct SqliteDiagnosticsStore {
    database: Mutex<Database>,
}

impl SqliteDiagnosticsStore {
    pub(crate) fn new(database: Database) -> Self {
        Self {
            database: Mutex::new(database),
        }
    }
}

impl DiagnosticsStore for SqliteDiagnosticsStore {
    fn database(&self) -> Result<DatabaseDiagnosticRecord, DiagnosticsStoreError> {
        let database = self
            .database
            .lock()
            .map_err(|_| DiagnosticsStoreError::Unavailable)?;
        database
            .verify_health()
            .map_err(|_| DiagnosticsStoreError::Unavailable)?;
        let schema_version = database
            .schema_version()
            .map_err(|_| DiagnosticsStoreError::Unavailable)?;
        if schema_version < 0 {
            return Err(DiagnosticsStoreError::InvalidStoredValue);
        }
        Ok(DatabaseDiagnosticRecord { schema_version })
    }

    fn sources(&self) -> Result<SourceDiagnosticRecord, DiagnosticsStoreError> {
        let database = self
            .database
            .lock()
            .map_err(|_| DiagnosticsStoreError::Unavailable)?;
        let row = database
            .connection()
            .query_row(
                "SELECT
                    COUNT(*) AS configured_count,
                    COALESCE(SUM(CASE WHEN enabled != 0 THEN 1 ELSE 0 END), 0) AS enabled_count,
                    COALESCE(SUM(CASE WHEN detection_state = 'available' THEN 1 ELSE 0 END), 0)
                        AS detected_count
                 FROM sources",
                [],
                |row| {
                    Ok(SourceCountRow {
                        configured_count: row.get(0)?,
                        enabled_count: row.get(1)?,
                        detected_count: row.get(2)?,
                    })
                },
            )
            .map_err(|error| match error {
                SqliteError::IntegralValueOutOfRange(_, _) => {
                    DiagnosticsStoreError::InvalidStoredValue
                }
                _ => DiagnosticsStoreError::Unavailable,
            })?;
        source_record(row)
    }
}

struct SourceCountRow {
    configured_count: i64,
    enabled_count: i64,
    detected_count: i64,
}

fn source_record(row: SourceCountRow) -> Result<SourceDiagnosticRecord, DiagnosticsStoreError> {
    Ok(SourceDiagnosticRecord {
        detected_count: u32::try_from(row.detected_count)
            .map_err(|_| DiagnosticsStoreError::InvalidStoredValue)?,
        configured_count: u32::try_from(row.configured_count)
            .map_err(|_| DiagnosticsStoreError::InvalidStoredValue)?,
        enabled_count: u32::try_from(row.enabled_count)
            .map_err(|_| DiagnosticsStoreError::InvalidStoredValue)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::database::test_database::TestDatabase;

    #[test]
    fn reads_database_and_source_diagnostics() {
        let mut test_database = TestDatabase::open();
        test_database
            .database_mut()
            .migrate_to_latest()
            .expect("migrate database");
        test_database
            .database()
            .connection()
            .execute(
                "INSERT INTO sources (
                    id, source_key, display_name, enabled, detection_state,
                    last_available_at_ms, created_at_ms, updated_at_ms
                ) VALUES
                    (1, 'claude-code', 'Claude Code', 1, 'available', 100, 100, 100),
                    (2, 'codex', 'Codex', 0, 'not_found', NULL, 100, 100)",
                [],
            )
            .expect("insert sources");
        let store = SqliteDiagnosticsStore::new(
            Database::open(test_database.path()).expect("reopen diagnostics database"),
        );

        assert_eq!(
            store.database().expect("database diagnostics"),
            DatabaseDiagnosticRecord { schema_version: 3 },
        );
        assert_eq!(
            store.sources().expect("source diagnostics"),
            SourceDiagnosticRecord {
                detected_count: 1,
                configured_count: 2,
                enabled_count: 1,
            },
        );
    }
}
