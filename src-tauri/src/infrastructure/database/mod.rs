//! SQLite connection ownership and policy enforcement.

mod error;
mod migrations;
mod run_store;
#[cfg(test)]
mod test_database;

use std::fs;
use std::path::Path;
use std::time::Duration;

use rusqlite::Connection;

pub use error::{PersistenceError, PersistenceErrorKind};

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);
const BUSY_TIMEOUT_MS: i64 = 5_000;
const JOURNAL_MODE: &str = "wal";
const SYNCHRONOUS_FULL: i64 = 2;
const DEFAULT_REFRESH_INTERVAL_MINUTES: i64 = 15;

pub struct Database {
    connection: Connection,
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
        migrations::to_latest(&mut self.connection)
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

    pub fn reporting_timezone(&self) -> Result<String, PersistenceError> {
        self.connection
            .query_row(
                "SELECT reporting_timezone FROM app_settings WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .map_err(|source| PersistenceError::read("app_settings.reporting_timezone", source))
    }

    pub fn schema_version(&self) -> Result<i64, PersistenceError> {
        self.connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(|source| PersistenceError::read("user_version", source))
    }

    pub(super) fn connection(&self) -> &Connection {
        &self.connection
    }
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

    use super::*;
    use test_database::TestDatabase;

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
    fn reads_seeded_reporting_timezone_and_schema_version() {
        let mut test_database = TestDatabase::open();
        let database = test_database.database_mut();
        database.migrate_to_latest().expect("migrate database");
        database
            .ensure_app_settings("Asia/Jakarta", 100)
            .expect("seed settings");

        assert_eq!(
            database.reporting_timezone().expect("timezone"),
            "Asia/Jakarta"
        );
        assert_eq!(database.schema_version().expect("schema version"), 1);
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
