use std::sync::Mutex;

use rusqlite::{params, Transaction};

use crate::application::ports::settings_store::{SettingsStore, SettingsStoreError};
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
                    launch_at_login = ?1,
                    close_behavior = ?2,
                    updated_at_ms = ?3,
                    revision = revision + 1
                 WHERE id = 1 AND revision = ?4",
                params![
                    settings.launch_at_login(),
                    settings.close_behavior().as_str(),
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
        let cleared = apply_project_path_policy(&transaction, false)?;
        transaction
            .execute(
                "UPDATE app_settings SET store_project_paths = 0 WHERE id = 1",
                [],
            )
            .map_err(|_| SettingsStoreError::Unavailable)?;
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
            "SELECT launch_at_login, close_behavior, revision
             FROM app_settings WHERE id = 1",
            [],
            |row| {
                Ok((
                    row.get::<_, bool>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .map_err(|_| SettingsStoreError::Unavailable)?;
    let settings =
        Settings::new(stored.0, &stored.1).map_err(|_| SettingsStoreError::InvalidStoredValue)?;
    SettingsDocument::new(settings, stored.2).map_err(|_| SettingsStoreError::InvalidStoredValue)
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
        Settings::new(false, "hide").expect("valid settings")
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
    fn startup_policy_clears_legacy_retained_project_paths() {
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

        assert_eq!(cleared, 1);
        assert!(ProjectPathIdentity::is_key(&identity_key));
        assert_eq!(raw_path, None);
        assert_eq!(fingerprint.len(), 32);
        let database = store.database.lock().expect("database lock");
        let retain_paths: bool = database
            .connection()
            .query_row(
                "SELECT store_project_paths FROM app_settings WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .expect("read retention policy");
        assert!(!retain_paths);
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
