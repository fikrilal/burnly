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

    fn update_settings(
        &self,
        settings: &crate::application::bootstrap::SettingsState,
    ) -> Result<(), BootstrapError> {
        let database = self
            .database
            .lock()
            .map_err(|_| BootstrapError::storage_unavailable())?;

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        database
            .update_settings(
                &settings.reporting_timezone,
                settings.background_refresh_enabled,
                settings.refresh_interval_minutes,
                settings.launch_at_login,
                &settings.close_behavior,
                settings.notifications_enabled,
                settings.store_project_paths,
                now_ms,
            )
            .map_err(|_| BootstrapError::storage_unavailable())
    }
}

fn read_storage(database: &Database) -> Result<BootstrapStorage, PersistenceError> {
    let (
        reporting_timezone,
        background_refresh_enabled,
        refresh_interval_minutes,
        launch_at_login,
        close_behavior,
        notifications_enabled,
        store_project_paths,
    ) = database.read_settings()?;

    Ok(BootstrapStorage {
        reporting_timezone,
        background_refresh_enabled,
        refresh_interval_minutes,
        launch_at_login,
        close_behavior,
        notifications_enabled,
        store_project_paths,
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
        assert!(!storage.background_refresh_enabled);
        assert_eq!(storage.refresh_interval_minutes, 15);

        // Test updating
        let settings = crate::application::bootstrap::SettingsState {
            reporting_timezone: "UTC".to_owned(),
            background_refresh_enabled: true,
            refresh_interval_minutes: 30,
            launch_at_login: true,
            close_behavior: "hide".to_owned(),
            notifications_enabled: true,
            store_project_paths: true,
        };
        store.update_settings(&settings).expect("update settings");

        let updated = store
            .read_bootstrap_storage()
            .expect("read updated storage");
        assert_eq!(updated.reporting_timezone, "UTC");
        assert!(updated.background_refresh_enabled);
        assert_eq!(updated.refresh_interval_minutes, 30);
        assert!(updated.launch_at_login);
        assert_eq!(updated.close_behavior, "hide");
        assert!(updated.notifications_enabled);
        assert!(updated.store_project_paths);
    }
}
