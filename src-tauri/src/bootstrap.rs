//! Application composition root.
//!
//! This module selects concrete infrastructure and platform integrations. Other
//! modules receive constructed dependencies instead of constructing their own.

use std::path::Path;
use std::sync::Arc;

use iana_time_zone::GetTimezoneError;
use tauri::{Manager, Runtime};
use thiserror::Error;

use crate::application::bootstrap::BootstrapService;
use crate::application::collection::CollectorFailure;
use crate::application::refresh::RefreshCoordinator;
use crate::infrastructure::bootstrap_store::SqliteBootstrapStore;
use crate::infrastructure::collectors::ccusage::CcusageCollector;
use crate::infrastructure::database::{
    Database, PersistenceError, PersistenceErrorKind, SqliteReconciliationStore,
};
use crate::ipc::CONTRACT_VERSION;
use crate::platform::system_clock::SystemClock;
use crate::platform::{database_path, system_clock, system_timezone};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StartupErrorKind {
    DatabasePath,
    Timezone,
    Clock,
    ResourceDir,
    Collector,
    Persistence(PersistenceErrorKind),
}

#[derive(Debug, Error)]
pub(crate) enum StartupError {
    #[error("failed to resolve the database path")]
    DatabasePath(#[source] database_path::DatabasePathError),

    #[error("failed to resolve the system timezone")]
    Timezone(#[source] GetTimezoneError),

    #[error("failed to read the system clock")]
    Clock(#[source] system_clock::ClockError),

    #[error("failed to resolve the application resource directory")]
    ResourceDir(#[source] tauri::Error),

    #[error("failed to initialize the usage collector")]
    Collector(#[source] CollectorFailure),

    #[error("failed to initialize persistence")]
    Persistence(#[source] PersistenceError),
}

impl StartupError {
    pub(crate) fn kind(&self) -> StartupErrorKind {
        match self {
            Self::DatabasePath(_) => StartupErrorKind::DatabasePath,
            Self::Timezone(_) => StartupErrorKind::Timezone,
            Self::Clock(_) => StartupErrorKind::Clock,
            Self::ResourceDir(_) => StartupErrorKind::ResourceDir,
            Self::Collector(_) => StartupErrorKind::Collector,
            Self::Persistence(error) => StartupErrorKind::Persistence(error.kind()),
        }
    }
}

pub(crate) fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(crate::ipc::invoke_handler())
        .setup(|app| {
            setup_runtime(app).map_err(|error| {
                eprintln!("Burnly startup failed ({:?})", error.kind());
                Box::new(error) as Box<dyn std::error::Error>
            })
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn setup_runtime<R: Runtime>(app: &mut tauri::App<R>) -> Result<(), StartupError> {
    let database_path = database_path::resolve(app.handle()).map_err(StartupError::DatabasePath)?;
    let reporting_timezone = system_timezone::resolve().map_err(StartupError::Timezone)?;
    let created_at_ms = system_clock::now_epoch_ms().map_err(StartupError::Clock)?;
    let database = initialize(&database_path, &reporting_timezone, created_at_ms)?;

    app.manage(build_refresh_coordinator(app, &database_path)?);
    app.manage(BootstrapService::new(
        env!("CARGO_PKG_VERSION"),
        CONTRACT_VERSION,
        SqliteBootstrapStore::new(database),
    ));
    Ok(())
}

fn build_refresh_coordinator<R: Runtime>(
    app: &tauri::App<R>,
    database_path: &Path,
) -> Result<RefreshCoordinator, StartupError> {
    let write_database = Database::open(database_path).map_err(StartupError::Persistence)?;
    let aggregation_timezone = write_database
        .reporting_timezone()
        .map_err(StartupError::Persistence)?;
    let store = Arc::new(SqliteReconciliationStore::new(write_database));
    let clock = Arc::new(SystemClock);
    let resource_directory = app
        .path()
        .resource_dir()
        .map_err(StartupError::ResourceDir)?;
    let collector =
        Arc::new(CcusageCollector::packaged(resource_directory).map_err(StartupError::Collector)?);

    Ok(RefreshCoordinator::new(
        collector,
        store.clone(),
        store,
        clock,
        env!("CARGO_PKG_VERSION"),
        aggregation_timezone,
    ))
}

fn initialize(
    database_path: &Path,
    reporting_timezone: &str,
    created_at_ms: i64,
) -> Result<Database, StartupError> {
    let mut database = Database::open(database_path).map_err(StartupError::Persistence)?;
    database
        .migrate_to_latest()
        .map_err(StartupError::Persistence)?;
    database
        .verify_health()
        .map_err(StartupError::Persistence)?;
    database
        .ensure_app_settings(reporting_timezone, created_at_ms)
        .map_err(StartupError::Persistence)?;

    Ok(database)
}

#[cfg(test)]
mod tests {
    use crate::application::bootstrap::{BootstrapError, BootstrapStorage, BootstrapStore};

    use rusqlite::Connection;
    use serde_json::Value;
    use tauri::webview::InvokeRequest;

    use super::*;

    struct FixedBootstrapStore;

    impl BootstrapStore for FixedBootstrapStore {
        fn read_bootstrap_storage(&self) -> Result<BootstrapStorage, BootstrapError> {
            Ok(BootstrapStorage {
                reporting_timezone: "Asia/Jakarta".to_owned(),
                schema_version: 1,
            })
        }
    }

    #[test]
    fn fresh_startup_creates_migrates_and_seeds_database() {
        let directory = tempfile::TempDir::new().expect("create app data directory");
        let database_path = directory.path().join("nested").join("burnly.sqlite3");

        drop(initialize(&database_path, "Asia/Jakarta", 100).expect("initialize application"));

        let connection = Connection::open(database_path).expect("reopen database");
        assert_eq!(pragma_i64(&connection, "user_version"), 1);
        assert_eq!(settings_count(&connection), 1);
        assert_eq!(
            setting_text(&connection, "reporting_timezone"),
            "Asia/Jakarta"
        );
        assert_eq!(setting_i64(&connection, "created_at_ms"), 100);
    }

    #[test]
    fn repeated_startup_preserves_existing_settings() {
        let directory = tempfile::TempDir::new().expect("create app data directory");
        let database_path = directory.path().join("burnly.sqlite3");

        drop(initialize(&database_path, "UTC", 100).expect("first startup"));
        drop(initialize(&database_path, "Asia/Jakarta", 200).expect("second startup"));

        let connection = Connection::open(database_path).expect("reopen database");
        assert_eq!(settings_count(&connection), 1);
        assert_eq!(setting_text(&connection, "reporting_timezone"), "UTC");
        assert_eq!(setting_i64(&connection, "created_at_ms"), 100);
    }

    #[test]
    fn unsupported_newer_schema_fails_with_stable_category() {
        let directory = tempfile::TempDir::new().expect("create app data directory");
        let database_path = directory.path().join("burnly.sqlite3");
        let connection = Connection::open(&database_path).expect("create database");
        connection
            .pragma_update(None, "user_version", 2)
            .expect("set newer version");
        drop(connection);

        let error = expect_startup_error(
            initialize(&database_path, "UTC", 100),
            "newer schema must prevent startup",
        );

        assert_eq!(
            error.kind(),
            StartupErrorKind::Persistence(PersistenceErrorKind::Migration)
        );
        let connection = Connection::open(database_path).expect("reopen database");
        assert_eq!(pragma_i64(&connection, "user_version"), 2);
    }

    #[test]
    fn foreign_key_violation_prevents_startup() {
        let directory = tempfile::TempDir::new().expect("create app data directory");
        let database_path = directory.path().join("burnly.sqlite3");
        drop(initialize(&database_path, "UTC", 100).expect("initial startup"));

        let connection = Connection::open(&database_path).expect("open database");
        connection
            .pragma_update(None, "foreign_keys", false)
            .expect("disable foreign keys for corruption fixture");
        connection
            .execute(
                "INSERT INTO budget_thresholds (budget_id, threshold_bps, enabled)
                 VALUES (999, 8000, 1)",
                [],
            )
            .expect("insert foreign key violation");
        drop(connection);

        let error = expect_startup_error(
            initialize(&database_path, "UTC", 100),
            "unhealthy database must prevent startup",
        );

        assert_eq!(
            error.kind(),
            StartupErrorKind::Persistence(PersistenceErrorKind::HealthCheck)
        );
    }

    #[test]
    fn invalid_seed_value_fails_with_stable_category() {
        let directory = tempfile::TempDir::new().expect("create app data directory");
        let database_path = directory.path().join("burnly.sqlite3");

        let error = expect_startup_error(
            initialize(&database_path, "", 100),
            "invalid settings seed must prevent startup",
        );

        assert_eq!(
            error.kind(),
            StartupErrorKind::Persistence(PersistenceErrorKind::Seed)
        );
    }

    #[test]
    fn tauri_bridge_invokes_bootstrap_command_with_real_envelope() {
        let response = invoke("app_get_bootstrap");

        assert_eq!(response["ok"], true);
        assert_eq!(response["data"]["appVersion"], env!("CARGO_PKG_VERSION"));
        assert_eq!(response["data"]["contractVersion"], CONTRACT_VERSION);
        assert_eq!(response["data"]["database"]["status"], "ready");
        assert_eq!(
            response["data"]["settings"]["reportingTimezone"],
            "Asia/Jakarta"
        );
        assert_eq!(response["meta"]["contractVersion"], CONTRACT_VERSION);
    }

    #[test]
    fn tauri_bridge_invokes_capabilities_command_with_explicit_states() {
        let response = invoke("app_get_capabilities");

        assert_eq!(response["ok"], true);
        assert_eq!(response["data"]["tray"]["supported"], false);
        assert_eq!(response["data"]["tray"]["status"], "not_implemented");
        assert_eq!(response["data"]["diagnostics"]["desktopEvidence"], true);
        assert_eq!(response["meta"]["contractVersion"], CONTRACT_VERSION);
    }

    fn settings_count(connection: &Connection) -> i64 {
        connection
            .query_row("SELECT COUNT(*) FROM app_settings", [], |row| row.get(0))
            .expect("count settings")
    }

    fn expect_startup_error(result: Result<Database, StartupError>, message: &str) -> StartupError {
        match result {
            Ok(_) => panic!("{message}"),
            Err(error) => error,
        }
    }

    fn setting_text(connection: &Connection, column: &str) -> String {
        connection
            .query_row(
                &format!("SELECT {column} FROM app_settings WHERE id = 1"),
                [],
                |row| row.get(0),
            )
            .expect("query text setting")
    }

    fn setting_i64(connection: &Connection, column: &str) -> i64 {
        connection
            .query_row(
                &format!("SELECT {column} FROM app_settings WHERE id = 1"),
                [],
                |row| row.get(0),
            )
            .expect("query integer setting")
    }

    fn pragma_i64(connection: &Connection, name: &str) -> i64 {
        connection
            .pragma_query_value(None, name, |row| row.get(0))
            .expect("query pragma")
    }

    fn invoke(command: &str) -> Value {
        let app = tauri::test::mock_builder()
            .invoke_handler(crate::ipc::invoke_handler())
            .manage(BootstrapService::new(
                env!("CARGO_PKG_VERSION"),
                CONTRACT_VERSION,
                FixedBootstrapStore,
            ))
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("build mock tauri app");
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("build mock webview");

        tauri::test::get_ipc_response(&webview, request(command))
            .expect("invoke command")
            .deserialize::<Value>()
            .expect("deserialize command response")
    }

    fn request(command: &str) -> InvokeRequest {
        InvokeRequest {
            cmd: command.into(),
            callback: tauri::ipc::CallbackFn(0),
            error: tauri::ipc::CallbackFn(1),
            url: if cfg!(any(windows, target_os = "android")) {
                "http://tauri.localhost"
            } else {
                "tauri://localhost"
            }
            .parse()
            .expect("parse tauri url"),
            body: tauri::ipc::InvokeBody::default(),
            headers: Default::default(),
            invoke_key: tauri::test::INVOKE_KEY.to_owned(),
        }
    }
}
