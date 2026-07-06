//! SQLite connection ownership and policy enforcement.

use std::fs;
use std::path::Path;
use std::time::Duration;

use rusqlite::{backup::Backup, Connection};

use super::PersistenceError;

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);
const BUSY_TIMEOUT_MS: i64 = 5_000;
const JOURNAL_MODE: &str = "wal";
const SYNCHRONOUS_FULL: i64 = 2;
const DEFAULT_REFRESH_INTERVAL_MINUTES: i64 = 15;

pub struct Database {
    pub(super) connection: Connection,
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, PersistenceError> {
        let path = path.as_ref();
        ensure_parent_directory(path)?;

        let connection = Connection::open(path)
            .map_err(|source| PersistenceError::open(path.to_path_buf(), source))?;

        configure_connection(&connection)?;

        Ok(Self { connection })
    }

    pub fn migrate_to_latest(&mut self) -> Result<(), PersistenceError> {
        super::migrations::to_latest(&mut self.connection)
    }

    pub fn needs_migration(&self) -> Result<bool, PersistenceError> {
        let version = self.schema_version()?;
        Ok(version > 0 && version < Self::latest_supported_schema_version())
    }

    pub fn create_verified_migration_backup(
        &self,
        database_path: &Path,
    ) -> Result<(), PersistenceError> {
        let final_path = migration_backup_path(database_path);
        let temporary_path = final_path.with_extension("sqlite3.tmp");
        let _ = fs::remove_file(&temporary_path);
        let mut destination =
            Connection::open(&temporary_path).map_err(PersistenceError::backup)?;
        {
            let backup = Backup::new(&self.connection, &mut destination)
                .map_err(PersistenceError::backup)?;
            backup
                .run_to_completion(128, Duration::from_millis(5), None)
                .map_err(PersistenceError::backup)?;
        }
        drop(destination);
        let backup = Database::open(&temporary_path)?;
        backup.verify_health()?;
        if backup.schema_version()? != self.schema_version()? {
            return Err(PersistenceError::unhealthy(
                "backup_schema_version",
                "schema version mismatch",
            ));
        }
        drop(backup);
        let _ = fs::remove_file(&final_path);
        fs::rename(&temporary_path, &final_path).map_err(PersistenceError::backup_publish)
    }
    pub fn verify_health(&self) -> Result<(), PersistenceError> {
        let integrity: String = self
            .connection
            .pragma_query_value(None, "quick_check", |row| row.get(0))
            .map_err(|source| PersistenceError::health_check("quick_check", source))?;

        if integrity != "ok" {
            return Err(PersistenceError::unhealthy("quick_check", integrity));
        }

        let has_foreign_key_violation: bool = self
            .connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM pragma_foreign_key_check)",
                [],
                |row| row.get(0),
            )
            .map_err(|source| PersistenceError::health_check("foreign_key_check", source))?;

        if has_foreign_key_violation {
            return Err(PersistenceError::unhealthy(
                "foreign_key_check",
                "constraint violation",
            ));
        }

        Ok(())
    }

    pub fn ensure_app_settings(
        &self,
        reporting_timezone: &str,
        created_at_ms: i64,
    ) -> Result<(), PersistenceError> {
        self.connection
            .execute(
                "INSERT INTO app_settings (
                    id, reporting_timezone, background_refresh_enabled,
                    refresh_interval_minutes, launch_at_login, close_behavior,
                    notifications_enabled, store_project_paths,
                    created_at_ms, updated_at_ms
                ) VALUES (1, ?1, 0, ?2, 0, 'quit', 0, 0, ?3, ?3)
                ON CONFLICT(id) DO NOTHING",
                (
                    reporting_timezone,
                    DEFAULT_REFRESH_INTERVAL_MINUTES,
                    created_at_ms,
                ),
            )
            .map_err(PersistenceError::seed)?;

        Ok(())
    }

    pub fn read_settings(&self) -> Result<(bool, String), PersistenceError> {
        self.connection
            .query_row(
                "SELECT launch_at_login, close_behavior
                 FROM app_settings WHERE id = 1",
                [],
                |row| Ok((row.get::<_, i32>(0)? != 0, row.get(1)?)),
            )
            .map_err(|source| PersistenceError::read("app_settings", source))
    }

    pub fn schema_version(&self) -> Result<i64, PersistenceError> {
        self.connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(|source| PersistenceError::read("user_version", source))
    }

    pub fn latest_supported_schema_version() -> i64 {
        super::migrations::LATEST_SCHEMA_VERSION
    }

    pub(crate) fn connection(&self) -> &Connection {
        &self.connection
    }

    pub(crate) fn connection_mut(&mut self) -> &mut Connection {
        &mut self.connection
    }
}

fn migration_backup_path(database_path: &Path) -> std::path::PathBuf {
    database_path.with_file_name("burnly.pre-migration.sqlite3")
}

fn ensure_parent_directory(path: &Path) -> Result<(), PersistenceError> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| PersistenceError::invalid_path(path.to_path_buf()))?;

    fs::create_dir_all(parent)
        .map_err(|source| PersistenceError::create_directory(parent.to_path_buf(), source))
}

fn configure_connection(connection: &Connection) -> Result<(), PersistenceError> {
    connection
        .pragma_update(None, "foreign_keys", true)
        .map_err(|source| PersistenceError::configure("foreign_keys", source))?;
    connection
        .busy_timeout(BUSY_TIMEOUT)
        .map_err(|source| PersistenceError::configure("busy_timeout", source))?;

    let journal_mode: String = connection
        .pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get(0))
        .map_err(|source| PersistenceError::configure("journal_mode", source))?;

    connection
        .pragma_update(None, "synchronous", "FULL")
        .map_err(|source| PersistenceError::configure("synchronous", source))?;

    verify_policy(connection, journal_mode)
}

fn verify_policy(connection: &Connection, journal_mode: String) -> Result<(), PersistenceError> {
    verify_value(
        "foreign_keys",
        "1",
        query_integer_policy(connection, "foreign_keys")?.to_string(),
    )?;
    verify_value(
        "busy_timeout",
        &BUSY_TIMEOUT_MS.to_string(),
        query_integer_policy(connection, "busy_timeout")?.to_string(),
    )?;
    verify_value("journal_mode", JOURNAL_MODE, journal_mode.to_lowercase())?;
    verify_value(
        "synchronous",
        &SYNCHRONOUS_FULL.to_string(),
        query_integer_policy(connection, "synchronous")?.to_string(),
    )
}

fn query_integer_policy(
    connection: &Connection,
    setting: &'static str,
) -> Result<i64, PersistenceError> {
    connection
        .pragma_query_value(None, setting, |row| row.get(0))
        .map_err(|source| PersistenceError::configure(setting, source))
}

fn verify_value(
    setting: &'static str,
    expected: &str,
    actual: String,
) -> Result<(), PersistenceError> {
    if actual == expected {
        return Ok(());
    }

    Err(PersistenceError::policy_mismatch(
        setting,
        expected.to_owned(),
        actual,
    ))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::super::error;
    use super::super::test_database::TestDatabase;
    use super::*;

    #[test]
    fn opens_file_database_with_required_policy() {
        let test_database = TestDatabase::open();
        let database = test_database.database();

        assert!(test_database.path().is_file());
        assert_eq!(pragma_i64(&database.connection, "foreign_keys"), 1);
        assert_eq!(pragma_i64(&database.connection, "busy_timeout"), 5_000);
        assert_eq!(pragma_text(&database.connection, "journal_mode"), "wal");
        assert_eq!(pragma_i64(&database.connection, "synchronous"), 2);
    }

    #[test]
    fn repeat_open_preserves_required_policy() {
        let temp_dir = tempfile::TempDir::new().expect("create temporary directory");
        let database_path = temp_dir.path().join("burnly.sqlite3");

        drop(Database::open(&database_path).expect("first database open"));
        let database = Database::open(&database_path).expect("second database open");

        assert_eq!(pragma_i64(&database.connection, "foreign_keys"), 1);
        assert_eq!(pragma_text(&database.connection, "journal_mode"), "wal");
    }

    #[test]
    fn classifies_parent_directory_creation_failure() {
        let temp_dir = tempfile::TempDir::new().expect("create temporary directory");
        let occupied_path = temp_dir.path().join("occupied");
        fs::write(&occupied_path, b"not a directory").expect("create occupied path");

        let error = match Database::open(occupied_path.join("burnly.sqlite3")) {
            Ok(_) => panic!("database open should fail"),
            Err(error) => error,
        };

        assert_eq!(error.kind(), error::PersistenceErrorKind::CreateDirectory);
    }

    #[test]
    fn classifies_database_open_failure() {
        let temp_dir = tempfile::TempDir::new().expect("create temporary directory");

        let error = match Database::open(temp_dir.path()) {
            Ok(_) => panic!("opening a directory as a database should fail"),
            Err(error) => error,
        };

        assert_eq!(error.kind(), error::PersistenceErrorKind::Open);
    }

    #[test]
    fn classifies_policy_mismatch() {
        let error = verify_value("foreign_keys", "1", "0".to_owned())
            .expect_err("mismatched policy should fail");

        assert_eq!(error.kind(), error::PersistenceErrorKind::PolicyMismatch);
    }

    #[test]
    fn reads_seeded_settings_and_schema_version() {
        let mut test_database = TestDatabase::open();
        let database = test_database.database_mut();
        database.migrate_to_latest().expect("migrate database");
        database
            .ensure_app_settings("Asia/Jakarta", 100)
            .expect("seed settings");

        let settings = database.read_settings().expect("read settings");
        assert!(!settings.0); // launch_at_login
        assert_eq!(settings.1, "quit"); // close_behavior

        assert_eq!(
            database.schema_version().expect("schema version"),
            Database::latest_supported_schema_version()
        );
    }

    #[test]
    fn fresh_database_does_not_create_a_migration_backup() {
        let test_database = TestDatabase::open();

        assert!(!test_database
            .database()
            .needs_migration()
            .expect("read migration requirement"));
    }

    fn pragma_i64(connection: &Connection, name: &str) -> i64 {
        connection
            .pragma_query_value(None, name, |row| row.get(0))
            .expect("query integer pragma")
    }

    fn pragma_text(connection: &Connection, name: &str) -> String {
        connection
            .pragma_query_value(None, name, |row| row.get(0))
            .expect("query text pragma")
    }
}
