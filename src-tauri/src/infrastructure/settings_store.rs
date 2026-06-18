use std::sync::Mutex;

use rusqlite::params;

use crate::application::ports::settings_store::{SettingsStore, SettingsStoreError};
use crate::domain::settings::{Settings, SettingsDocument};

use super::database::Database;

pub(crate) struct SqliteSettingsStore {
    database: Mutex<Database>,
}

impl SqliteSettingsStore {
    pub(crate) fn new(database: Database) -> Self {
        Self {
            database: Mutex::new(database),
        }
    }
}

impl SettingsStore for SqliteSettingsStore {
    fn get(&self) -> Result<SettingsDocument, SettingsStoreError> {
        let database = self
            .database
            .lock()
            .map_err(|_| SettingsStoreError::Unavailable)?;
        read_document(&database)
    }

    fn replace(
        &self,
        expected_revision: i64,
        settings: &Settings,
        updated_at_ms: i64,
    ) -> Result<SettingsDocument, SettingsStoreError> {
        let database = self
            .database
            .lock()
            .map_err(|_| SettingsStoreError::Unavailable)?;
        let changed = database
            .connection()
            .execute(
                "UPDATE app_settings SET
                    reporting_timezone = ?1,
                    background_refresh_enabled = ?2,
                    refresh_interval_minutes = ?3,
                    launch_at_login = ?4,
                    close_behavior = ?5,
                    notifications_enabled = ?6,
                    store_project_paths = ?7,
                    updated_at_ms = ?8,
                    revision = revision + 1
                 WHERE id = 1 AND revision = ?9",
                params![
                    settings.reporting_timezone(),
                    settings.background_refresh_enabled(),
                    settings.refresh_interval_minutes(),
                    settings.launch_at_login(),
                    settings.close_behavior().as_str(),
                    settings.notifications_enabled(),
                    settings.store_project_paths(),
                    updated_at_ms,
                    expected_revision,
                ],
            )
            .map_err(|_| SettingsStoreError::Unavailable)?;
        if changed == 0 {
            return Err(SettingsStoreError::Conflict);
        }
        read_document(&database)
    }
}

fn read_document(database: &Database) -> Result<SettingsDocument, SettingsStoreError> {
    let stored = database
        .connection()
        .query_row(
            "SELECT reporting_timezone, background_refresh_enabled,
                    refresh_interval_minutes, launch_at_login, close_behavior,
                    notifications_enabled, store_project_paths, revision
             FROM app_settings WHERE id = 1",
            [],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, bool>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, bool>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, bool>(5)?,
                    row.get::<_, bool>(6)?,
                    row.get::<_, i64>(7)?,
                ))
            },
        )
        .map_err(|_| SettingsStoreError::Unavailable)?;
    let settings = Settings::new(
        stored.0, stored.1, stored.2, stored.3, &stored.4, stored.5, stored.6,
    )
    .map_err(|_| SettingsStoreError::InvalidStoredValue)?;
    SettingsDocument::new(settings, stored.7).map_err(|_| SettingsStoreError::InvalidStoredValue)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> SqliteSettingsStore {
        let directory = tempfile::TempDir::new().expect("temporary directory");
        let path = directory.keep().join("burnly.sqlite3");
        let mut database = Database::open(path).expect("open database");
        database.migrate_to_latest().expect("migrate database");
        database
            .ensure_app_settings("UTC", 100)
            .expect("seed settings");
        SqliteSettingsStore::new(database)
    }

    fn updated_settings() -> Settings {
        Settings::new(
            "Asia/Jakarta".to_owned(),
            true,
            30,
            false,
            "hide",
            false,
            false,
        )
        .expect("valid settings")
    }

    #[test]
    fn replaces_document_and_increments_revision() {
        let store = store();

        let updated = store
            .replace(1, &updated_settings(), 200)
            .expect("replace settings");

        assert_eq!(updated.revision(), 2);
        assert_eq!(updated.settings(), &updated_settings());
    }

    #[test]
    fn rejects_stale_revision_without_overwriting_settings() {
        let store = store();
        store
            .replace(1, &updated_settings(), 200)
            .expect("first replacement");

        assert_eq!(
            store.replace(1, &updated_settings(), 300),
            Err(SettingsStoreError::Conflict)
        );
        assert_eq!(store.get().expect("read settings").revision(), 2);
    }

    #[test]
    fn updated_settings_survive_database_reopen() {
        let directory = tempfile::TempDir::new().expect("temporary directory");
        let path = directory.path().join("burnly.sqlite3");
        let mut database = Database::open(&path).expect("open database");
        database.migrate_to_latest().expect("migrate database");
        database
            .ensure_app_settings("UTC", 100)
            .expect("seed settings");
        let store = SqliteSettingsStore::new(database);
        store
            .replace(1, &updated_settings(), 200)
            .expect("replace settings");
        drop(store);

        let reopened = SqliteSettingsStore::new(Database::open(path).expect("reopen database"));
        let document = reopened.get().expect("read reopened settings");

        assert_eq!(document.revision(), 2);
        assert_eq!(document.settings(), &updated_settings());
    }
}
