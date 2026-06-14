use std::sync::Mutex;

use crate::application::bootstrap::{BootstrapError, BootstrapStorage, BootstrapStore};

use super::database::{Database, PersistenceError};

pub(crate) struct SqliteBootstrapStore {
    database: Mutex<Database>,
}

impl SqliteBootstrapStore {
    pub(crate) fn new(database: Database) -> Self {
        Self {
            database: Mutex::new(database),
        }
    }
}

impl BootstrapStore for SqliteBootstrapStore {
    fn read_bootstrap_storage(&self) -> Result<BootstrapStorage, BootstrapError> {
        let database = self
            .database
            .lock()
            .map_err(|_| BootstrapError::storage_unavailable())?;

        read_storage(&database).map_err(|_| BootstrapError::storage_unavailable())
    }
}

fn read_storage(database: &Database) -> Result<BootstrapStorage, PersistenceError> {
    Ok(BootstrapStorage {
        reporting_timezone: database.reporting_timezone()?,
        schema_version: database.schema_version()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_seeded_bootstrap_storage_from_database() {
        let directory = tempfile::TempDir::new().expect("create temporary directory");
        let database_path = directory.path().join("burnly.sqlite3");
        let mut database = Database::open(database_path).expect("open database");
        database.migrate_to_latest().expect("migrate database");
        database
            .ensure_app_settings("Asia/Jakarta", 100)
            .expect("seed settings");

        let store = SqliteBootstrapStore::new(database);
        let storage = store
            .read_bootstrap_storage()
            .expect("read bootstrap storage");

        assert_eq!(storage.reporting_timezone, "Asia/Jakarta");
        assert_eq!(storage.schema_version, 1);
    }
}
