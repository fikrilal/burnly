use std::sync::Mutex;

use rusqlite::{params, Transaction};

use crate::application::ports::settings_store::{
    ProjectPathRetentionResult, SettingsStore, SettingsStoreError,
};
use crate::domain::settings::{Settings, SettingsDocument};

use super::database::Database;
use super::project_identity::ProjectPathIdentity;

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

    fn replace_project_path_retention(
        &self,
        expected_revision: i64,
        retain_paths: bool,
        updated_at_ms: i64,
    ) -> Result<ProjectPathRetentionResult, SettingsStoreError> {
        let mut database = self
            .database
            .lock()
            .map_err(|_| SettingsStoreError::Unavailable)?;
        let transaction = database
            .connection_mut()
            .transaction()
            .map_err(|_| SettingsStoreError::Unavailable)?;
        let current_revision: i64 = transaction
            .query_row(
                "SELECT revision FROM app_settings WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .map_err(|_| SettingsStoreError::Unavailable)?;
        if current_revision != expected_revision {
            return Err(SettingsStoreError::Conflict);
        }

        let cleared_paths = apply_project_path_policy(&transaction, retain_paths)?;
        transaction
            .execute(
                "UPDATE app_settings
                 SET store_project_paths = ?1,
                     updated_at_ms = ?2,
                     revision = revision + 1
                 WHERE id = 1 AND revision = ?3",
                params![retain_paths, updated_at_ms, expected_revision],
            )
            .map_err(|_| SettingsStoreError::Unavailable)?;
        transaction
            .commit()
            .map_err(|_| SettingsStoreError::Unavailable)?;

        Ok(ProjectPathRetentionResult {
            settings: read_document(&database)?,
            cleared_paths,
        })
    }
}

impl SqliteSettingsStore {
    pub(crate) fn enforce_current_project_path_policy(&self) -> Result<u32, SettingsStoreError> {
        let mut database = self
            .database
            .lock()
            .map_err(|_| SettingsStoreError::Unavailable)?;
        let transaction = database
            .connection_mut()
            .transaction()
            .map_err(|_| SettingsStoreError::Unavailable)?;
        let retains_paths: bool = transaction
            .query_row(
                "SELECT store_project_paths FROM app_settings WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .map_err(|_| SettingsStoreError::Unavailable)?;
        let cleared = apply_project_path_policy(&transaction, retains_paths)?;
        transaction
            .commit()
            .map_err(|_| SettingsStoreError::Unavailable)?;
        Ok(cleared)
    }
}

fn apply_project_path_policy(
    transaction: &Transaction<'_>,
    retain_paths: bool,
) -> Result<u32, SettingsStoreError> {
    let projects = {
        let mut statement = transaction
            .prepare(
                "SELECT id, identity_key, raw_path
                 FROM projects
                 WHERE identity_kind = 'path'",
            )
            .map_err(|_| SettingsStoreError::Unavailable)?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })
            .map_err(|_| SettingsStoreError::Unavailable)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| SettingsStoreError::Unavailable)?;
        rows
    };

    let mut cleared = 0_u32;
    for (id, identity_key, raw_path) in projects {
        let legacy_path =
            (!ProjectPathIdentity::is_key(&identity_key)).then_some(identity_key.as_str());
        let source_path = raw_path.as_deref().or(legacy_path);
        if let Some(path) = source_path {
            let identity = ProjectPathIdentity::from_path(path);
            let retained_path = retain_paths.then_some(path);
            transaction
                .execute(
                    "UPDATE projects
                     SET identity_key = ?1, path_fingerprint = ?2, raw_path = ?3
                     WHERE id = ?4",
                    params![
                        identity.key(),
                        identity.fingerprint().as_slice(),
                        retained_path,
                        id
                    ],
                )
                .map_err(|_| SettingsStoreError::Unavailable)?;
        }
        if !retain_paths && (raw_path.is_some() || legacy_path.is_some()) {
            cleared = cleared
                .checked_add(1)
                .ok_or(SettingsStoreError::Unavailable)?;
        }
    }
    Ok(cleared)
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

    #[test]
    fn disabling_retention_rekeys_legacy_identity_and_clears_raw_path_atomically() {
        let store = store();
        insert_legacy_project(&store, "/home/dante/secret-project");

        let result = store
            .replace_project_path_retention(1, false, 200)
            .expect("disable retention");

        assert_eq!(result.cleared_paths, 1);
        assert!(!result.settings.settings().store_project_paths());
        assert_eq!(result.settings.revision(), 2);
        let (identity_key, raw_path, fingerprint) = read_project(&store);
        assert!(ProjectPathIdentity::is_key(&identity_key));
        assert!(!identity_key.contains("secret-project"));
        assert_eq!(raw_path, None);
        assert_eq!(fingerprint.len(), 32);
    }

    #[test]
    fn stale_privacy_revision_rolls_back_without_clearing_paths() {
        let store = store();
        insert_legacy_project(&store, "/home/dante/secret-project");

        assert_eq!(
            store.replace_project_path_retention(2, false, 200),
            Err(SettingsStoreError::Conflict)
        );
        let (identity_key, raw_path, _) = read_project(&store);
        assert_eq!(identity_key, "/home/dante/secret-project");
        assert_eq!(raw_path.as_deref(), Some("/home/dante/secret-project"));
    }

    #[test]
    fn enabling_retention_does_not_reconstruct_deleted_paths() {
        let store = store();
        insert_legacy_project(&store, "/home/dante/secret-project");
        store
            .replace_project_path_retention(1, false, 200)
            .expect("disable retention");

        let result = store
            .replace_project_path_retention(2, true, 300)
            .expect("enable retention");

        assert!(result.settings.settings().store_project_paths());
        assert_eq!(result.cleared_paths, 0);
        assert_eq!(read_project(&store).1, None);
    }

    #[test]
    fn startup_policy_normalizes_legacy_identity_while_retaining_opted_in_path() {
        let store = store();
        insert_legacy_project(&store, "/home/dante/secret-project");
        {
            let database = store.database.lock().expect("database lock");
            database
                .connection()
                .execute(
                    "UPDATE app_settings SET store_project_paths = 1 WHERE id = 1",
                    [],
                )
                .expect("enable path retention");
        }

        let cleared = store
            .enforce_current_project_path_policy()
            .expect("enforce policy");
        let (identity_key, raw_path, fingerprint) = read_project(&store);

        assert_eq!(cleared, 0);
        assert!(ProjectPathIdentity::is_key(&identity_key));
        assert_eq!(raw_path.as_deref(), Some("/home/dante/secret-project"));
        assert_eq!(fingerprint.len(), 32);
    }

    fn insert_legacy_project(store: &SqliteSettingsStore, path: &str) {
        let database = store.database.lock().expect("database lock");
        database
            .connection()
            .execute(
                "INSERT INTO sources (
                    id, source_key, display_name, enabled, detection_state,
                    created_at_ms, updated_at_ms
                 ) VALUES (1, 'claude-code', 'Claude Code', 1, 'available', 0, 0)",
                [],
            )
            .expect("insert source");
        database
            .connection()
            .execute(
                "INSERT INTO projects (
                    source_id, identity_key, identity_kind, raw_path,
                    path_fingerprint, first_seen_at_ms, last_seen_at_ms
                 ) VALUES (1, ?1, 'path', ?1, X'00', 0, 0)",
                [path],
            )
            .expect("insert legacy project");
    }

    fn read_project(store: &SqliteSettingsStore) -> (String, Option<String>, Vec<u8>) {
        let database = store.database.lock().expect("database lock");
        database
            .connection()
            .query_row(
                "SELECT identity_key, raw_path, path_fingerprint FROM projects",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("read project")
    }
}
