use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};

use crate::application::ports::database_maintenance::{
    CheckpointOutcome, DatabaseAccess, DatabaseMaintenanceStore, DatabaseMaintenanceStoreError,
    IntegrityOutcome, RecoveryRecord,
};

use super::{migration_backup_path, restore_verified_migration_backup, Database};

pub(crate) struct SqliteDatabaseMaintenanceStore {
    path: PathBuf,
}

impl SqliteDatabaseMaintenanceStore {
    pub(crate) fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

impl DatabaseMaintenanceStore for SqliteDatabaseMaintenanceStore {
    fn recovery_status(&self) -> Result<RecoveryRecord, DatabaseMaintenanceStoreError> {
        let backup_available = migration_backup_path(&self.path).is_file();
        match Database::open(&self.path) {
            Ok(database) => Ok(RecoveryRecord {
                access: DatabaseAccess::ReadWrite,
                schema_version: database.schema_version().ok(),
                backup_available,
            }),
            Err(_) => match open_read_only(&self.path) {
                Ok(connection) => Ok(RecoveryRecord {
                    access: DatabaseAccess::ReadOnly,
                    schema_version: connection
                        .pragma_query_value(None, "user_version", |row| row.get(0))
                        .ok(),
                    backup_available,
                }),
                Err(_) => Ok(RecoveryRecord {
                    access: DatabaseAccess::Unavailable,
                    schema_version: None,
                    backup_available,
                }),
            },
        }
    }

    fn integrity_check(&self) -> Result<IntegrityOutcome, DatabaseMaintenanceStoreError> {
        let connection = open_read_only(&self.path)?;
        let result: String = connection
            .pragma_query_value(None, "integrity_check", |row| row.get(0))
            .map_err(map_error)?;
        Ok(if result == "ok" {
            IntegrityOutcome::Healthy
        } else {
            IntegrityOutcome::Corrupt
        })
    }

    fn checkpoint(&self) -> Result<CheckpointOutcome, DatabaseMaintenanceStoreError> {
        let database =
            Database::open(&self.path).map_err(|_| DatabaseMaintenanceStoreError::Unavailable)?;
        let row = database
            .connection()
            .query_row("PRAGMA wal_checkpoint(PASSIVE)", [], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })
            .map_err(map_error)?;
        Ok(CheckpointOutcome {
            busy: value(row.0)?,
            log_frames: value(row.1)?,
            checkpointed_frames: value(row.2)?,
        })
    }

    fn vacuum(&self) -> Result<(), DatabaseMaintenanceStoreError> {
        let database =
            Database::open(&self.path).map_err(|_| DatabaseMaintenanceStoreError::Unavailable)?;
        database
            .connection()
            .execute_batch("VACUUM")
            .map_err(map_error)
    }

    fn restore_migration_backup(&self) -> Result<(), DatabaseMaintenanceStoreError> {
        restore_verified_migration_backup(&self.path)
            .map_err(|_| DatabaseMaintenanceStoreError::Unavailable)
    }
}

fn open_read_only(path: &Path) -> Result<Connection, DatabaseMaintenanceStoreError> {
    Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )
    .map_err(map_error)
}

fn value(value: i64) -> Result<u32, DatabaseMaintenanceStoreError> {
    u32::try_from(value).map_err(|_| DatabaseMaintenanceStoreError::InvalidStoredValue)
}

fn map_error(error: rusqlite::Error) -> DatabaseMaintenanceStoreError {
    match error.sqlite_error_code() {
        Some(rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked) => {
            DatabaseMaintenanceStoreError::Busy
        }
        Some(rusqlite::ErrorCode::ReadOnly) => DatabaseMaintenanceStoreError::ReadOnly,
        _ => DatabaseMaintenanceStoreError::Unavailable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::ports::database_maintenance::DatabaseMaintenanceStore;

    #[test]
    fn healthy_database_supports_explicit_maintenance() {
        let directory = tempfile::TempDir::new().expect("temporary directory");
        let path = directory.path().join("burnly.sqlite3");
        let mut database = Database::open(&path).expect("open database");
        database.migrate_to_latest().expect("migrate database");
        drop(database);
        let store = SqliteDatabaseMaintenanceStore::new(path);

        let status = store.recovery_status().expect("maintenance status");
        assert_eq!(status.access, DatabaseAccess::ReadWrite);
        assert_eq!(
            store.integrity_check().expect("integrity check"),
            IntegrityOutcome::Healthy
        );
        let checkpoint = store.checkpoint().expect("checkpoint");
        assert!(checkpoint.checkpointed_frames <= checkpoint.log_frames);
        store.vacuum().expect("vacuum");
    }
}
