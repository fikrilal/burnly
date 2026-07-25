//! Application composition root.
//!
//! This module selects concrete infrastructure and platform integrations. Other
//! modules receive constructed dependencies instead of constructing their own.

mod account_runtime;
mod collect_sync_runtime;
mod collectors;
mod resources;
mod runtime_events;
mod settings_runtime;
mod startup;
#[cfg(test)]
mod test_support;
mod tray_runtime;

use std::path::Path;
use std::sync::{Arc, Mutex};

use iana_time_zone::GetTimezoneError;
use tauri::{Manager, Runtime, WindowEvent};
use thiserror::Error;

use crate::application::bootstrap::{BootstrapService, RuntimeCapabilities, RuntimeSettings};

use crate::application::collection::CollectorFailure;
use crate::application::diagnostics::DiagnosticsService;
use crate::application::ports::collector::Collector;
use crate::application::ports::run_store::RunStoreError;
use crate::application::ports::window_actions::WindowActions;
use crate::application::refresh::{
    RefreshCoordinator, RefreshEventSink, RefreshPolicy, RefreshScheduler, RefreshSchedulerError,
};
use crate::application::settings::SettingsService;
use crate::application::update::UpdateService;
use crate::application::usage::TraySummaryQuery;
use crate::domain::settings::CloseBehavior;
use crate::infrastructure::database::{
    Database, PersistenceError, PersistenceErrorKind, SqliteBootstrapStore, SqliteDiagnosticStore,
    SqliteReconciliationStore, SqliteSettingsStore, SqliteTraySummaryStore,
};
use crate::ipc::CONTRACT_VERSION;
use crate::platform::lifecycle;
#[cfg(not(debug_assertions))]
use crate::platform::single_instance;
use crate::platform::system_clock::SystemClock;
use crate::platform::{database_path, system_clock, system_timezone, tray, updater};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StartupErrorKind {
    DatabasePath,
    Timezone,
    Clock,
    ResourceDir,
    Collector,
    RefreshScheduler,
    RunRecovery,
    Tray,
    TrayPanel,
    PrivacyPolicy,
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

    #[error("failed to initialize refresh scheduler")]
    RefreshScheduler(#[source] RefreshSchedulerError),

    #[error("failed to recover interrupted refresh runs")]
    RunRecovery(#[source] RunStoreError),

    #[error("failed to initialize the system tray")]
    Tray(#[source] tauri::Error),

    #[error("failed to initialize the tray panel")]
    TrayPanel(#[source] lifecycle::WindowActivationError),

    #[error("failed to enforce the project-path privacy policy")]
    PrivacyPolicy,

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
            Self::RefreshScheduler(_) => StartupErrorKind::RefreshScheduler,
            Self::RunRecovery(_) => StartupErrorKind::RunRecovery,
            Self::Tray(_) => StartupErrorKind::Tray,
            Self::TrayPanel(_) => StartupErrorKind::TrayPanel,
            Self::PrivacyPolicy => StartupErrorKind::PrivacyPolicy,
            Self::Persistence(error) => StartupErrorKind::Persistence(error.kind()),
        }
    }
}

pub(crate) fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .invoke_handler(crate::ipc::invoke_handler())
        .on_window_event(|window, event| {
            if let WindowEvent::Focused(false) = event {
                lifecycle::handle_tray_panel_blur(window);
            }
        })
        .setup(|app| {
            setup_runtime(app).map_err(|error| {
                eprintln!("Burnly startup failed ({:?})", error.kind());
                Box::new(error) as Box<dyn std::error::Error>
            })
        });

    #[cfg(not(debug_assertions))]
    let builder = builder.plugin(single_instance::plugin());

    let app = builder
        .build(tauri::generate_context!())
        .expect("error while running tauri application");

    app.run(runtime_events::handle_run_event);
}

fn setup_runtime<R: Runtime>(app: &mut tauri::App<R>) -> Result<(), StartupError> {
    app.manage(runtime_events::ExitGuard::default());
    // Burnly is a menu-bar-first app; keep it out of the macOS Dock and app
    // switcher so the only entry point is the status-bar icon.
    #[cfg(target_os = "macos")]
    app.set_activation_policy(tauri::ActivationPolicy::Accessory);
    let database_path = database_path::resolve(app.handle()).map_err(StartupError::DatabasePath)?;
    let reporting_timezone = system_timezone::resolve().map_err(StartupError::Timezone)?;
    let created_at_ms = system_clock::now_epoch_ms().map_err(StartupError::Clock)?;
    let database =
        startup::initialize_database(&database_path, &reporting_timezone, created_at_ms)?;
    startup::recover_interrupted_runs(&database_path, created_at_ms)?;
    let (launch_at_login, close_behavior) = database
        .read_settings()
        .map_err(StartupError::Persistence)?;
    let refresh_policy = automatic_refresh_policy();
    let tray_state = Arc::new(Mutex::new(None));
    let settings_database = Database::open(&database_path).map_err(StartupError::Persistence)?;
    let settings_store = Arc::new(SqliteSettingsStore::new(settings_database));
    settings_store
        .enforce_current_project_path_policy()
        .map_err(|_| StartupError::PrivacyPolicy)?;
    let tray_summary_query = build_tray_summary_query(&database_path)?;
    let refresh_event_sink = tray_runtime::runtime_refresh_event_sink(
        app.handle().clone(),
        tray_state.clone(),
        tray_summary_query.clone(),
    );
    let refresh_coordinator = build_refresh_coordinator(
        app,
        &database_path,
        refresh_event_sink,
        reporting_timezone.clone(),
    )?;
    let refresh_scheduler =
        RefreshScheduler::start(refresh_policy, Arc::new(refresh_coordinator.clone()))
            .map_err(StartupError::RefreshScheduler)?;
    let runtime_settings = parse_runtime_settings(&close_behavior)?;
    let tray_controller = install_tray_controller(
        app,
        tray_state.clone(),
        &refresh_coordinator,
        &tray_summary_query,
        &reporting_timezone,
    )?;
    app.manage(tray_controller);
    lifecycle::prepare_tray_panel(app.handle()).map_err(StartupError::TrayPanel)?;
    tray_runtime::install_tray_invalidation_listener(
        app.handle().clone(),
        tray_summary_query.clone(),
    );
    let runtime_capabilities = build_runtime_capabilities();

    app.manage(
        Arc::new(lifecycle::DesktopWindowActions::new(app.handle().clone()))
            as Arc<dyn WindowActions>,
    );
    app.manage(refresh_coordinator.clone());
    app.manage(refresh_scheduler);
    app.manage(runtime_settings.clone());
    let tray_open_refresh = tray_runtime::TrayOpenRefreshController::new(
        reporting_timezone.clone(),
        tray_summary_query.clone(),
        refresh_coordinator.clone(),
        Arc::new(SystemClock),
    );
    app.manage(tray_summary_query.clone());
    app.manage(UpdateService::new(Arc::new(
        updater::TauriUpdateRuntime::new(app.handle().clone()),
    )));
    let diagnostics_database = Database::open(&database_path).map_err(StartupError::Persistence)?;
    app.manage(DiagnosticsService::new(
        Arc::new(SqliteDiagnosticStore::new(diagnostics_database)),
        env!("CARGO_PKG_VERSION").to_owned(),
        reporting_timezone.clone(),
    ));

    app.manage(BootstrapService::new(
        env!("CARGO_PKG_VERSION"),
        CONTRACT_VERSION,
        SqliteBootstrapStore::new(database),
        runtime_capabilities,
    ));
    let runtime = Arc::new(settings_runtime::DesktopSettingsRuntime::new(
        app.handle().clone(),
        runtime_settings,
    ));
    if let Err(error) = runtime.reconcile_launch_at_login_on_startup(launch_at_login) {
        eprintln!("Burnly launch-at-login reconciliation failed: {error:?}");
    }
    app.manage(SettingsService::new(
        settings_store.clone(),
        runtime,
        Arc::new(SystemClock),
    ));
    let installed_account =
        account_runtime::install_account_service(app.handle(), env!("CARGO_PKG_VERSION"));
    let _ =
        collect_sync_runtime::install_collect_sync(collect_sync_runtime::CollectSyncInstallArgs {
            app: app.handle(),
            database_path: &database_path,
            reporting_timezone: &reporting_timezone,
            app_version: env!("CARGO_PKG_VERSION"),
            session: installed_account.session,
            authenticated_client: installed_account.authenticated_client,
            account: &installed_account.service,
            refresh_coordinator: &refresh_coordinator,
            device_id: installed_account.device_id,
            device_name: installed_account.device_name,
        });
    app.manage(installed_account.service);
    // Install the committed-upload sink before startup can launch a refresh.
    tray_open_refresh.request_startup_refresh_if_stale();
    app.manage(tray_open_refresh);

    Ok(())
}

fn parse_runtime_settings(close_behavior: &str) -> Result<RuntimeSettings, StartupError> {
    Ok(RuntimeSettings::new(
        CloseBehavior::parse(close_behavior).map_err(|_| {
            StartupError::Persistence(PersistenceError::invalid_stored_value(
                "app_settings.close_behavior",
            ))
        })?,
    ))
}

fn install_tray_controller<R: Runtime>(
    app: &tauri::App<R>,
    tray_state: Arc<Mutex<Option<tray::TrayController<R>>>>,
    refresh_coordinator: &RefreshCoordinator,
    tray_summary_query: &TraySummaryQuery,
    reporting_timezone: &str,
) -> Result<tray::TrayController<R>, StartupError> {
    let tray_controller = tray::TrayController::install(
        app.handle(),
        &tray_runtime::tray_snapshot(
            &refresh_coordinator.snapshot(),
            tray_summary_query,
            reporting_timezone,
        ),
    )
    .map_err(StartupError::Tray)?;
    *tray_state.lock().expect("tray state lock is poisoned") = Some(tray_controller.clone());
    tray_controller.update(&tray_runtime::tray_snapshot(
        &refresh_coordinator.snapshot(),
        tray_summary_query,
        reporting_timezone,
    ));

    Ok(tray_controller)
}

fn build_runtime_capabilities() -> RuntimeCapabilities {
    RuntimeCapabilities::new(
        RuntimeCapabilities::tray_available(),
        settings_runtime::launch_at_login_capability(),
        RuntimeCapabilities::update_available(),
    )
}

fn automatic_refresh_policy() -> RefreshPolicy {
    RefreshPolicy::enabled_minutes(15)
}

fn build_tray_summary_query(database_path: &Path) -> Result<TraySummaryQuery, StartupError> {
    let database = Database::open(database_path).map_err(StartupError::Persistence)?;
    Ok(TraySummaryQuery::new(
        Arc::new(SqliteTraySummaryStore::new(database)),
        Arc::new(SystemClock),
    ))
}

fn build_refresh_coordinator<R: Runtime>(
    app: &tauri::App<R>,
    database_path: &Path,
    refresh_event_sink: Arc<dyn RefreshEventSink>,
    reporting_timezone: String,
) -> Result<RefreshCoordinator, StartupError> {
    let resource_directory = app
        .path()
        .resource_dir()
        .map_err(StartupError::ResourceDir)?;
    let collector = collectors::build_collector_graph(resource_directory, database_path)?;

    compose_refresh_coordinator(
        database_path,
        collector,
        refresh_event_sink,
        reporting_timezone,
    )
}

fn compose_refresh_coordinator(
    database_path: &Path,
    collector: Arc<dyn Collector>,
    refresh_event_sink: Arc<dyn RefreshEventSink>,
    reporting_timezone: String,
) -> Result<RefreshCoordinator, StartupError> {
    let write_database = Database::open(database_path).map_err(StartupError::Persistence)?;
    let store = Arc::new(SqliteReconciliationStore::new(write_database));
    let clock = Arc::new(SystemClock);
    let diagnostics_database = Database::open(database_path).map_err(StartupError::Persistence)?;
    let diagnostic_recorder = Arc::new(SqliteDiagnosticStore::new(diagnostics_database));

    let coordinator = RefreshCoordinator::with_event_sink(
        collector,
        store.clone(),
        store,
        clock,
        refresh_event_sink,
        env!("CARGO_PKG_VERSION"),
        reporting_timezone,
    );
    coordinator.set_diagnostic_recorder(diagnostic_recorder);
    Ok(coordinator)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::domain::settings::{Settings, SettingsDocument};
    use crate::ipc::refresh_event_sink;

    use rusqlite::Connection;
    use serde_json::{json, Value};

    use super::test_support::*;
    use super::*;

    #[test]
    fn home_data_dir_prefers_home_when_available() {
        assert_eq!(
            resources::home_data_dir(
                Some(PathBuf::from("/home/dante")),
                Some(PathBuf::from("C:/Users/fikrilal")),
                ".zcode",
            ),
            Some(PathBuf::from("/home/dante").join(".zcode"))
        );
    }

    #[test]
    fn home_data_dir_falls_back_to_userprofile_on_windows() {
        assert_eq!(
            resources::home_data_dir(None, Some(PathBuf::from("C:/Users/fikrilal")), ".cline"),
            Some(PathBuf::from("C:/Users/fikrilal").join(".cline"))
        );
    }

    #[test]
    fn packaged_resource_resolver_prefers_tauri_resource_directory_when_valid() {
        let workspace = tempfile::tempdir().expect("workspace");
        let resource_directory = workspace.path().join("usr").join("lib").join("burnly");
        write_packaged_sidecar_manifest(&resource_directory);

        let resolved = resources::resolve_packaged_resource_directory_for_appdir(
            resource_directory.clone(),
            None,
        );

        assert_eq!(resolved, resource_directory);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn packaged_resource_resolver_uses_product_directory_for_appimage_case_mismatch() {
        let workspace = tempfile::tempdir().expect("workspace");
        let appdir = workspace.path().join("squashfs-root");
        let tauri_resource_directory = appdir.join("usr").join("lib").join("burnly");
        let product_resource_directory = appdir.join("usr").join("lib").join("Burnly");
        write_packaged_sidecar_manifest(&product_resource_directory);

        let resolved = resources::resolve_packaged_resource_directory_for_appdir(
            tauri_resource_directory,
            Some(&appdir),
        );

        assert_eq!(resolved, product_resource_directory);
    }

    #[test]
    fn fresh_startup_creates_migrates_and_seeds_database() {
        let directory = tempfile::TempDir::new().expect("create app data directory");
        let database_path = directory.path().join("nested").join("burnly.sqlite3");

        drop(
            startup::initialize_database(&database_path, "Asia/Jakarta", 100)
                .expect("initialize application"),
        );

        let connection = Connection::open(database_path).expect("reopen database");
        assert_eq!(
            pragma_i64(&connection, "user_version"),
            crate::infrastructure::database::Database::latest_supported_schema_version()
        );
        assert_eq!(settings_count(&connection), 1);
        assert_eq!(
            setting_text(&connection, "reporting_timezone"),
            "Asia/Jakarta"
        );
        assert_eq!(setting_i64(&connection, "launch_at_login"), 1);
        assert_eq!(setting_i64(&connection, "created_at_ms"), 100);
    }

    #[test]
    fn repeated_startup_preserves_existing_settings() {
        let directory = tempfile::TempDir::new().expect("create app data directory");
        let database_path = directory.path().join("burnly.sqlite3");

        drop(startup::initialize_database(&database_path, "UTC", 100).expect("first startup"));
        drop(
            startup::initialize_database(&database_path, "Asia/Jakarta", 200)
                .expect("second startup"),
        );

        let connection = Connection::open(database_path).expect("reopen database");
        assert_eq!(settings_count(&connection), 1);
        assert_eq!(setting_text(&connection, "reporting_timezone"), "UTC");
        assert_eq!(setting_i64(&connection, "created_at_ms"), 100);
    }

    #[test]
    fn startup_recovery_terminalizes_interrupted_runs() {
        let directory = tempfile::TempDir::new().expect("create app data directory");
        let database_path = directory.path().join("burnly.sqlite3");

        drop(
            startup::initialize_database(&database_path, "UTC", 100)
                .expect("initialize application"),
        );
        let connection = Connection::open(&database_path).expect("open database");
        connection
            .execute(
                "INSERT INTO sources (
                    id, source_key, display_name, enabled, detection_state,
                    created_at_ms, updated_at_ms
                ) VALUES (1, 'claude-code', 'claude-code', 1, 'unknown', 100, 100)",
                [],
            )
            .expect("insert source");
        connection
            .execute(
                "INSERT INTO refresh_runs (
                    id, job_key, trigger, status, started_at_ms,
                    requested_by_app_version, created_at_ms
                ) VALUES (1, 'startup-recovery', 'launch', 'running', 110, '0.1.0', 110)",
                [],
            )
            .expect("insert refresh run");
        connection
            .execute(
                "INSERT INTO import_runs (
                    refresh_run_id, source_id, collector_key, collector_version,
                    profile_version, projection, scope_kind, scope_start_date,
                    scope_end_date, aggregation_timezone, status, records_seen,
                    records_rejected, started_at_ms
                ) VALUES (1, 1, 'ccusage', '20.0.14', 1, 'daily', 'full',
                    NULL, NULL, 'UTC', 'running', 0, 0, 120)",
                [],
            )
            .expect("insert import run");
        drop(connection);

        startup::recover_interrupted_runs(&database_path, 200).expect("recover interrupted runs");

        let connection = Connection::open(database_path).expect("reopen database");
        let refresh: (String, Option<i64>, Option<String>) = connection
            .query_row(
                "SELECT status, finished_at_ms, error_code FROM refresh_runs WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("read refresh run");
        let import: (String, Option<i64>, Option<String>) = connection
            .query_row(
                "SELECT status, finished_at_ms, error_code FROM import_runs WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("read import run");

        assert_eq!(
            refresh,
            (
                "failed".to_owned(),
                Some(200),
                Some("refresh.interrupted".to_owned())
            )
        );
        assert_eq!(
            import,
            (
                "failed".to_owned(),
                Some(200),
                Some("import.interrupted".to_owned())
            )
        );
        let diagnostic: (String, String, String, i64) = connection
            .query_row(
                "SELECT area, severity, code, created_at_ms
                 FROM diagnostic_events",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("read recovery diagnostic");
        assert_eq!(
            diagnostic,
            (
                "refresh".to_owned(),
                "warning".to_owned(),
                "refresh.interrupted_recovered".to_owned(),
                200,
            )
        );
    }

    #[test]
    fn unsupported_newer_schema_fails_with_stable_category() {
        let directory = tempfile::TempDir::new().expect("create app data directory");
        let database_path = directory.path().join("burnly.sqlite3");
        let connection = Connection::open(&database_path).expect("create database");
        connection
            .pragma_update(
                None,
                "user_version",
                crate::infrastructure::database::Database::latest_supported_schema_version() + 1,
            )
            .expect("set newer version");
        drop(connection);

        let error = expect_startup_error(
            startup::initialize_database(&database_path, "UTC", 100),
            "newer schema must prevent startup",
        );

        assert_eq!(
            error.kind(),
            StartupErrorKind::Persistence(PersistenceErrorKind::Migration)
        );
        let connection = Connection::open(database_path).expect("reopen database");
        assert_eq!(
            pragma_i64(&connection, "user_version"),
            crate::infrastructure::database::Database::latest_supported_schema_version() + 1
        );
    }

    #[test]
    fn foreign_key_violation_prevents_startup() {
        let directory = tempfile::TempDir::new().expect("create app data directory");
        let database_path = directory.path().join("burnly.sqlite3");
        drop(startup::initialize_database(&database_path, "UTC", 100).expect("initial startup"));

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
            startup::initialize_database(&database_path, "UTC", 100),
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
            startup::initialize_database(&database_path, "", 100),
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
        assert_eq!(response["data"]["settings"]["closeBehavior"], "quit");
        assert_eq!(response["meta"]["contractVersion"], CONTRACT_VERSION);
    }

    #[test]
    fn tauri_bridge_allows_tray_panel_bootstrap_ipc() {
        let response = invoke_from_window(lifecycle::TRAY_PANEL_WINDOW_LABEL, "app_get_bootstrap");

        assert_eq!(response["ok"], true);
        assert_eq!(response["data"]["contractVersion"], CONTRACT_VERSION);
        assert_eq!(response["data"]["settings"]["closeBehavior"], "quit");
    }

    #[test]
    fn tauri_bridge_invokes_capabilities_command_with_explicit_states() {
        let response = invoke("app_get_capabilities");

        assert_eq!(response["ok"], true);
        assert_eq!(response["data"]["tray"]["supported"], false);
        assert_eq!(response["data"]["tray"]["status"], "not_implemented");
        assert_eq!(response["data"]["update"]["supported"], false);
        assert_eq!(response["data"]["update"]["status"], "not_implemented");
        assert_eq!(response["data"]["diagnostics"]["desktopEvidence"], true);
        assert_eq!(
            response["data"]["diagnostics"]["sendReport"]["supported"],
            false
        );
        assert_eq!(
            response["data"]["diagnostics"]["sendReport"]["status"],
            "not_implemented"
        );
        assert_eq!(response["meta"]["contractVersion"], CONTRACT_VERSION);
    }

    #[test]
    fn tauri_bridge_reports_unavailable_update_state() {
        let response = invoke("update_get_state");

        assert_eq!(response["ok"], true);
        assert_eq!(response["data"]["status"], "unavailable");
        assert_eq!(response["data"]["availableVersion"], Value::Null);
        assert_eq!(response["data"]["downloadedVersion"], Value::Null);
        assert_eq!(response["data"]["lastCheckedAt"], Value::Null);
        assert_eq!(response["data"]["error"]["code"], "update.unavailable");
        assert_eq!(response["data"]["error"]["retryable"], false);
        assert_eq!(response["meta"]["contractVersion"], CONTRACT_VERSION);
    }

    #[test]
    fn tauri_bridge_rejects_update_check_when_unavailable() {
        let response = invoke("update_check");

        assert_eq!(response["ok"], false);
        assert_eq!(response["error"]["code"], "update.unavailable");
        assert_eq!(response["error"]["category"], "unavailable");
        assert_eq!(response["error"]["retryable"], false);
        assert_eq!(response["meta"]["contractVersion"], CONTRACT_VERSION);
    }

    #[test]
    fn tauri_bridge_updates_settings() {
        let initial = Settings::new(false, "quit").expect("valid settings");
        let app = tauri::test::mock_builder()
            .invoke_handler(crate::ipc::invoke_handler())
            .manage(BootstrapService::new(
                env!("CARGO_PKG_VERSION"),
                CONTRACT_VERSION,
                FixedBootstrapStore,
                capabilities_without_tray(),
            ))
            .manage(SettingsService::new(
                Arc::new(TestSettingsStore {
                    document: Mutex::new(
                        SettingsDocument::new(initial, 1).expect("settings document"),
                    ),
                }),
                Arc::new(TestSettingsRuntime),
                Arc::new(SystemClock),
            ))
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("build mock tauri app");
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("build mock webview");

        let response = tauri::test::get_ipc_response(
            &webview,
            request_with_body(
                "settings_update",
                json!({
                    "request": {
                        "expectedRevision": 1,
                        "launchAtLogin": false,
                        "closeBehavior": "hide"
                    }
                }),
            ),
        )
        .expect("invoke settings update")
        .deserialize::<Value>()
        .expect("deserialize settings response");

        assert_eq!(response["ok"], true);
        assert_eq!(response["data"]["closeBehavior"], "hide");
        assert_eq!(response["data"]["revision"], 2);
    }

    #[test]
    fn tauri_bridge_invokes_refresh_state_command() {
        let directory = tempfile::TempDir::new().expect("create app data directory");
        let database_path = directory.path().join("burnly.sqlite3");
        let mut database = Database::open(&database_path).expect("open database");
        database.migrate_to_latest().expect("migrate database");
        database
            .ensure_app_settings("UTC", 100)
            .expect("seed settings");

        let store = Arc::new(SqliteReconciliationStore::new(database));
        let coordinator = RefreshCoordinator::new(
            fake_ccusage_collector(),
            store.clone(),
            store,
            Arc::new(SystemClock),
            env!("CARGO_PKG_VERSION"),
            "UTC",
        );

        let app = tauri::test::mock_builder()
            .invoke_handler(crate::ipc::invoke_handler())
            .manage(coordinator)
            .manage(BootstrapService::new(
                env!("CARGO_PKG_VERSION"),
                CONTRACT_VERSION,
                FixedBootstrapStore,
                capabilities_without_tray(),
            ))
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("build mock tauri app");
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("build mock webview");

        let response = tauri::test::get_ipc_response(&webview, request("refresh_get_state"))
            .expect("invoke command")
            .deserialize::<Value>()
            .expect("deserialize command response");

        assert_eq!(response["ok"], true);
        assert_eq!(response["data"]["status"], "idle");
        assert_eq!(response["data"]["jobId"], Value::Null);
        assert_eq!(response["meta"]["contractVersion"], CONTRACT_VERSION);
    }

    #[cfg(unix)]
    #[test]
    fn tauri_bridge_executes_composed_refresh_and_persists_usage() {
        use std::{
            thread,
            time::{Duration, Instant},
        };

        let directory = tempfile::TempDir::new().expect("create app data directory");
        let database_path = directory.path().join("burnly.sqlite3");
        drop(
            startup::initialize_database(&database_path, "UTC", 100)
                .expect("initialize application"),
        );

        let app = tauri::test::mock_builder()
            .invoke_handler(crate::ipc::invoke_handler())
            .manage(BootstrapService::new(
                env!("CARGO_PKG_VERSION"),
                CONTRACT_VERSION,
                FixedBootstrapStore,
                capabilities_without_tray(),
            ))
            .manage(unavailable_update_service())
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("build mock tauri app");
        let collector = composed_refresh_collector(directory.path());
        let coordinator = compose_refresh_coordinator(
            &database_path,
            collector,
            refresh_event_sink(app.handle().clone()),
            "UTC".to_owned(),
        )
        .expect("coordinator");
        assert!(app.manage(coordinator));
        assert!(app.manage(build_tray_summary_query(&database_path).expect("tray summary query")));
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("build mock webview");

        let submitted = tauri::test::get_ipc_response(&webview, request("refresh_request"))
            .expect("invoke refresh request")
            .deserialize::<Value>()
            .expect("deserialize refresh response");
        assert_eq!(submitted["ok"], true);
        assert_eq!(submitted["data"]["status"], "running");

        let deadline = Instant::now() + Duration::from_secs(10);
        let terminal = loop {
            let response = tauri::test::get_ipc_response(&webview, request("refresh_get_state"))
                .expect("invoke refresh state")
                .deserialize::<Value>()
                .expect("deserialize refresh state");
            if response["data"]["status"] != "running" {
                break response;
            }
            if Instant::now() >= deadline {
                panic!("refresh reaches terminal state; last response: {response}");
            }
            thread::sleep(Duration::from_millis(10));
        };

        let connection = Connection::open(&database_path).expect("open persisted database");
        assert_eq!(
            terminal["data"]["status"],
            "succeeded",
            "import statuses: {}",
            import_statuses(&connection)
        );
        let daily_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM daily_usage", [], |row| row.get(0))
            .expect("count daily usage");
        assert_eq!(daily_count, 8);
        drop(connection);

        let summary = tauri::test::get_ipc_response(
            &webview,
            request_with_body(
                "usage_get_tray_summary",
                json!({
                    "request": {
                        "reportingTimezone": "UTC"
                    }
                }),
            ),
        )
        .expect("invoke usage tray summary")
        .deserialize::<Value>()
        .expect("deserialize usage tray summary");

        assert_eq!(summary["ok"], true);
        assert_eq!(summary["data"]["dataStatus"], "empty");
        assert!(summary["data"]["asOf"]
            .as_str()
            .expect("snapshot timestamp")
            .ends_with('Z'));
    }

    #[cfg(unix)]
    fn import_statuses(connection: &Connection) -> String {
        let mut statement = connection
            .prepare(
                "SELECT sources.source_key, import_runs.projection, import_runs.status,
                    import_runs.error_code, import_runs.error_detail
                FROM import_runs
                INNER JOIN sources ON sources.id = import_runs.source_id
                ORDER BY import_runs.id",
            )
            .expect("prepare import status query");
        let rows = statement
            .query_map([], |row| {
                Ok(format!(
                    "{}:{}:{}:{:?}:{:?}",
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            })
            .expect("query import statuses");

        rows.map(|row| row.expect("import status row"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}
