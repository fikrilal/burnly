//! Application composition root.
//!
//! This module selects concrete infrastructure and platform integrations. Other
//! modules receive constructed dependencies instead of constructing their own.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use iana_time_zone::GetTimezoneError;
use tauri::{Listener, Manager, RunEvent, Runtime, WindowEvent};
use thiserror::Error;

use crate::application::bootstrap::{BootstrapService, RuntimeCapabilities, RuntimeSettings};

use crate::application::collection::CollectorFailure;
use crate::application::ports::window_actions::WindowActions;
use crate::application::reconciliation::RefreshTrigger;
use crate::application::refresh::{
    RefreshCoordinator, RefreshEventSink, RefreshPolicy, RefreshScheduler, RefreshSchedulerError,
    RefreshSnapshot, RefreshStatus,
};
use crate::application::settings::{RuntimeSettingError, SettingsRuntime, SettingsService};
use crate::application::usage::TraySummaryQuery;
use crate::domain::settings::{CloseBehavior, Settings};
use crate::infrastructure::bootstrap_store::SqliteBootstrapStore;
use crate::infrastructure::collectors::ccusage::CcusageCollector;
use crate::infrastructure::database::{
    Database, PersistenceError, PersistenceErrorKind, SqliteReconciliationStore,
    SqliteTraySummaryStore,
};
use crate::infrastructure::settings_store::SqliteSettingsStore;
use crate::ipc::refresh_event_sink;
use crate::ipc::CONTRACT_VERSION;
use crate::platform::lifecycle;
#[cfg(not(debug_assertions))]
use crate::platform::single_instance;
use crate::platform::system_clock::SystemClock;
use crate::platform::{database_path, system_clock, system_timezone, tray};

const TRAY_OPEN_STALE_AFTER_MS: i64 = 5 * 60 * 1_000;
const TRAY_OPEN_REFRESH_THROTTLE_MS: i64 = 60 * 1_000;

#[derive(Default)]
struct ExitGuard {
    explicit_exit_requested: AtomicBool,
}

impl ExitGuard {
    fn request_explicit_exit(&self) {
        self.explicit_exit_requested.store(true, Ordering::SeqCst);
    }

    fn allows_exit(&self) -> bool {
        self.explicit_exit_requested.load(Ordering::SeqCst)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StartupErrorKind {
    DatabasePath,
    Timezone,
    Clock,
    ResourceDir,
    Collector,
    RefreshScheduler,
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

    app.run(handle_run_event);
}

fn handle_run_event<R: Runtime>(app: &tauri::AppHandle<R>, event: RunEvent) {
    match event {
        RunEvent::Resumed => {
            if let Some(coordinator) = app.try_state::<RefreshCoordinator>() {
                coordinator.request_refresh(RefreshTrigger::Resume);
            }
        }
        RunEvent::MenuEvent(event) => {
            handle_menu_event(app, &event);
        }
        RunEvent::ExitRequested { api, .. } => {
            let explicit_exit_requested = app
                .try_state::<ExitGuard>()
                .is_some_and(|guard| guard.allows_exit());
            if !explicit_exit_requested {
                api.prevent_exit();
            }
        }
        #[cfg(target_os = "macos")]
        RunEvent::Reopen { .. } => {
            let _ = lifecycle::activate_main_window(app);
        }
        _ => {}
    }
}

fn setup_runtime<R: Runtime>(app: &mut tauri::App<R>) -> Result<(), StartupError> {
    app.manage(ExitGuard::default());
    let database_path = database_path::resolve(app.handle()).map_err(StartupError::DatabasePath)?;
    let reporting_timezone = system_timezone::resolve().map_err(StartupError::Timezone)?;
    let created_at_ms = system_clock::now_epoch_ms().map_err(StartupError::Clock)?;
    let database = initialize(&database_path, &reporting_timezone, created_at_ms)?;
    let (_, close_behavior) = database
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
    let refresh_event_sink = runtime_refresh_event_sink(
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
    let runtime_settings =
        RuntimeSettings::new(CloseBehavior::parse(&close_behavior).map_err(|_| {
            StartupError::Persistence(PersistenceError::invalid_stored_value(
                "app_settings.close_behavior",
            ))
        })?);
    let tray_controller = tray::TrayController::install(
        app.handle(),
        &tray_snapshot(
            &refresh_coordinator.snapshot(),
            &tray_summary_query,
            &reporting_timezone,
        ),
    )
    .map_err(StartupError::Tray)?;
    *tray_state.lock().expect("tray state lock is poisoned") = Some(tray_controller.clone());
    tray_controller.update(&tray_snapshot(
        &refresh_coordinator.snapshot(),
        &tray_summary_query,
        &reporting_timezone,
    ));
    app.manage(tray_controller);
    lifecycle::prepare_tray_panel(app.handle()).map_err(StartupError::TrayPanel)?;
    install_tray_invalidation_listener(app.handle().clone(), tray_summary_query.clone());
    let runtime_capabilities = RuntimeCapabilities::new(RuntimeCapabilities::tray_available());

    app.manage(
        Arc::new(lifecycle::DesktopWindowActions::new(app.handle().clone()))
            as Arc<dyn WindowActions>,
    );
    app.manage(refresh_coordinator.clone());
    app.manage(refresh_scheduler);
    app.manage(runtime_settings.clone());
    let tray_open_refresh = TrayOpenRefreshController::new(
        reporting_timezone.clone(),
        tray_summary_query.clone(),
        refresh_coordinator.clone(),
        Arc::new(SystemClock),
    );
    tray_open_refresh.request_startup_refresh_if_stale();
    app.manage(tray_summary_query.clone());
    app.manage(tray_open_refresh);

    app.manage(BootstrapService::new(
        env!("CARGO_PKG_VERSION"),
        CONTRACT_VERSION,
        SqliteBootstrapStore::new(database),
        runtime_capabilities,
    ));
    let runtime = Arc::new(DesktopSettingsRuntime {
        app: app.handle().clone(),
        runtime_settings,
    });
    app.manage(SettingsService::new(
        settings_store.clone(),
        runtime,
        Arc::new(SystemClock),
    ));

    Ok(())
}

struct DesktopSettingsRuntime<R: Runtime> {
    app: tauri::AppHandle<R>,
    runtime_settings: RuntimeSettings,
}

impl<R: Runtime> SettingsRuntime for DesktopSettingsRuntime<R> {
    fn validate(
        &self,
        _current: &Settings,
        _proposed: &Settings,
    ) -> Result<(), RuntimeSettingError> {
        Ok(())
    }

    fn apply(&self, settings: &Settings) {
        self.runtime_settings.update(settings);

        use tauri_plugin_autostart::ManagerExt;
        let autostart = self.app.autolaunch();
        if settings.launch_at_login() {
            let _ = autostart.enable();
        } else {
            let _ = autostart.disable();
        }
    }
}

fn handle_menu_event<R: Runtime>(app: &tauri::AppHandle<R>, event: &tauri::menu::MenuEvent) {
    match tray::TrayAction::from_menu_event(event) {
        Some(tray::TrayAction::OpenPanel) => {
            if let Some(controller) = app.try_state::<TrayOpenRefreshController>() {
                controller.request_tray_open_refresh_if_stale();
            }
            let _ = lifecycle::open_tray_panel(app);
        }

        Some(tray::TrayAction::Refresh) => {
            if let Some(coordinator) = app.try_state::<RefreshCoordinator>() {
                coordinator.request_refresh(RefreshTrigger::Manual);
            }
        }
        Some(tray::TrayAction::Quit) => {
            if let Some(exit_guard) = app.try_state::<ExitGuard>() {
                exit_guard.request_explicit_exit();
            }
            app.exit(0);
        }
        None => {}
    }
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
    let collector = Arc::new(
        match std::env::var_os("BURNLY_CCUSAGE_DEV_BINARY") {
            Some(binary) => CcusageCollector::development(binary),
            None => CcusageCollector::packaged(resource_directory),
        }
        .map_err(StartupError::Collector)?,
    );

    compose_refresh_coordinator(
        database_path,
        collector,
        refresh_event_sink,
        reporting_timezone,
    )
}

fn compose_refresh_coordinator(
    database_path: &Path,
    collector: Arc<CcusageCollector>,
    refresh_event_sink: Arc<dyn RefreshEventSink>,
    reporting_timezone: String,
) -> Result<RefreshCoordinator, StartupError> {
    let write_database = Database::open(database_path).map_err(StartupError::Persistence)?;
    let store = Arc::new(SqliteReconciliationStore::new(write_database));
    let clock = Arc::new(SystemClock);

    Ok(RefreshCoordinator::with_event_sink(
        collector,
        store.clone(),
        store,
        clock,
        refresh_event_sink,
        env!("CARGO_PKG_VERSION"),
        reporting_timezone,
    ))
}

struct RuntimeRefreshEventSink<R: Runtime> {
    frontend: Arc<dyn RefreshEventSink>,
    tray: Arc<Mutex<Option<tray::TrayController<R>>>>,
    tray_summary: TraySummaryQuery,
}

impl<R: Runtime> RefreshEventSink for RuntimeRefreshEventSink<R> {
    fn publish(&self, snapshot: RefreshSnapshot, usage_changed: bool) {
        self.frontend.publish(snapshot.clone(), usage_changed);
        if let Some(tray) = self
            .tray
            .lock()
            .expect("tray state lock is poisoned")
            .as_ref()
        {
            let timezone = system_timezone::resolve().unwrap_or_else(|_| "UTC".to_owned());
            tray.update(&tray_snapshot(&snapshot, &self.tray_summary, &timezone));
        }
    }
}

fn runtime_refresh_event_sink<R: Runtime>(
    app: tauri::AppHandle<R>,
    tray: Arc<Mutex<Option<tray::TrayController<R>>>>,
    tray_summary: TraySummaryQuery,
) -> Arc<dyn RefreshEventSink> {
    Arc::new(RuntimeRefreshEventSink {
        frontend: refresh_event_sink(app),
        tray,
        tray_summary,
    })
}

fn install_tray_invalidation_listener<R: Runtime>(
    app: tauri::AppHandle<R>,
    tray_summary: TraySummaryQuery,
) {
    let listener_app = app.clone();
    app.listen("burnly://v1/data-invalidated", move |_| {
        if let (Some(controller), Some(coordinator)) = (
            listener_app.try_state::<tray::TrayController<R>>(),
            listener_app.try_state::<RefreshCoordinator>(),
        ) {
            let timezone = system_timezone::resolve().unwrap_or_else(|_| "UTC".to_owned());
            controller.update(&tray_snapshot(
                &coordinator.snapshot(),
                &tray_summary,
                &timezone,
            ));
        }
    });
}

pub(crate) fn tray_snapshot(
    snapshot: &RefreshSnapshot,
    tray_summary: &TraySummaryQuery,
    reporting_timezone: &str,
) -> tray::TraySnapshot {
    let summary = tray_summary.get(reporting_timezone).ok();
    let today_tokens = summary.as_ref().map(|s| s.today.total_tokens);
    let week_tokens = summary.as_ref().map(|s| s.week.total_tokens);
    let month_tokens = summary.as_ref().map(|s| s.month.total_tokens);

    tray::TraySnapshot {
        status: tray_refresh_status(snapshot.status),
        last_successful_refresh_at_ms: snapshot.last_successful_refresh_at_ms,
        budget_summary: None,
        today_tokens,
        week_tokens,
        month_tokens,
    }
}

const fn tray_refresh_status(status: RefreshStatus) -> tray::TrayRefreshStatus {
    match status {
        RefreshStatus::Idle => tray::TrayRefreshStatus::Idle,
        RefreshStatus::Queued => tray::TrayRefreshStatus::Queued,
        RefreshStatus::Running => tray::TrayRefreshStatus::Running,
        RefreshStatus::Cancelling => tray::TrayRefreshStatus::Cancelling,
        RefreshStatus::Succeeded => tray::TrayRefreshStatus::Succeeded,
        RefreshStatus::Partial => tray::TrayRefreshStatus::Partial,
        RefreshStatus::Failed => tray::TrayRefreshStatus::Failed,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrayOpenRefreshDecision {
    Request,
    SkipActive,
    SkipFresh,
    SkipThrottled,
    SkipClock,
    SkipReadFailure,
}

trait TrayOpenClock: Send + Sync {
    fn now_epoch_ms(&self) -> Option<i64>;
}

impl TrayOpenClock for SystemClock {
    fn now_epoch_ms(&self) -> Option<i64> {
        system_clock::now_epoch_ms().ok()
    }
}

struct TrayOpenRefreshController {
    reporting_timezone: String,
    summary_query: TraySummaryQuery,
    coordinator: RefreshCoordinator,
    clock: Arc<dyn TrayOpenClock>,
    last_request_at_ms: Mutex<Option<i64>>,
}

impl TrayOpenRefreshController {
    fn new(
        reporting_timezone: String,
        summary_query: TraySummaryQuery,
        coordinator: RefreshCoordinator,
        clock: Arc<dyn TrayOpenClock>,
    ) -> Self {
        Self {
            reporting_timezone,
            summary_query,
            coordinator,
            clock,
            last_request_at_ms: Mutex::new(None),
        }
    }

    fn request_startup_refresh_if_stale(&self) -> TrayOpenRefreshDecision {
        self.request_if_stale(RefreshTrigger::Launch)
    }

    fn request_tray_open_refresh_if_stale(&self) -> TrayOpenRefreshDecision {
        self.request_if_stale(RefreshTrigger::Manual)
    }

    fn request_if_stale(&self, trigger: RefreshTrigger) -> TrayOpenRefreshDecision {
        let now_ms = match self.clock.now_epoch_ms() {
            Some(value) => value,
            None => return TrayOpenRefreshDecision::SkipClock,
        };
        let last_successful_refresh_at_ms = match self.summary_query.get(&self.reporting_timezone) {
            Ok(summary) => summary.last_successful_refresh_at_ms,
            Err(_) => return TrayOpenRefreshDecision::SkipReadFailure,
        };
        let snapshot = self.coordinator.snapshot();
        let mut last_request = self
            .last_request_at_ms
            .lock()
            .expect("tray open refresh lock is poisoned");
        let decision = tray_open_refresh_decision(
            now_ms,
            last_successful_refresh_at_ms,
            *last_request,
            snapshot.status.is_active(),
        );
        if decision == TrayOpenRefreshDecision::Request {
            *last_request = Some(now_ms);
            if matches!(trigger, RefreshTrigger::Manual) {
                self.coordinator.request_freshness_refresh(trigger);
            } else {
                self.coordinator.request_refresh(trigger);
            }
        }
        decision
    }
}

fn tray_open_refresh_decision(
    now_ms: i64,
    last_successful_refresh_at_ms: Option<i64>,
    last_request_at_ms: Option<i64>,
    refresh_active: bool,
) -> TrayOpenRefreshDecision {
    if refresh_active {
        return TrayOpenRefreshDecision::SkipActive;
    }
    if let Some(last_request_at_ms) = last_request_at_ms {
        if now_ms.saturating_sub(last_request_at_ms) < TRAY_OPEN_REFRESH_THROTTLE_MS {
            return TrayOpenRefreshDecision::SkipThrottled;
        }
    }
    if let Some(last_successful_refresh_at_ms) = last_successful_refresh_at_ms {
        if now_ms.saturating_sub(last_successful_refresh_at_ms) < TRAY_OPEN_STALE_AFTER_MS {
            return TrayOpenRefreshDecision::SkipFresh;
        }
    }
    TrayOpenRefreshDecision::Request
}

fn initialize(
    database_path: &Path,
    reporting_timezone: &str,
    created_at_ms: i64,
) -> Result<Database, StartupError> {
    let mut database = Database::open(database_path).map_err(StartupError::Persistence)?;
    if database
        .needs_migration()
        .map_err(StartupError::Persistence)?
    {
        database
            .create_verified_migration_backup(database_path)
            .map_err(StartupError::Persistence)?;
    }
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
    use crate::application::bootstrap::{
        BootstrapError, BootstrapStorage, BootstrapStore, Capability, CapabilityStatus,
    };
    use crate::application::ports::settings_store::{SettingsStore, SettingsStoreError};
    use crate::domain::settings::{Settings, SettingsDocument};

    use rusqlite::Connection;
    use serde_json::{json, Value};
    use tauri::webview::InvokeRequest;

    use super::*;

    struct FixedBootstrapStore;

    impl BootstrapStore for FixedBootstrapStore {
        fn read_bootstrap_storage(&self) -> Result<BootstrapStorage, BootstrapError> {
            Ok(BootstrapStorage {
                launch_at_login: false,
                close_behavior: "quit".to_owned(),
                settings_revision: 1,
                schema_version: 2,
            })
        }
    }

    struct TestSettingsStore {
        document: Mutex<SettingsDocument>,
    }

    impl SettingsStore for TestSettingsStore {
        fn get(&self) -> Result<SettingsDocument, SettingsStoreError> {
            Ok(self.document.lock().expect("settings lock").clone())
        }

        fn replace(
            &self,
            expected_revision: i64,
            settings: &Settings,
            _updated_at_ms: i64,
        ) -> Result<SettingsDocument, SettingsStoreError> {
            let mut document = self.document.lock().expect("settings lock");
            if document.revision() != expected_revision {
                return Err(SettingsStoreError::Conflict);
            }
            *document = SettingsDocument::new(settings.clone(), expected_revision + 1)
                .expect("valid document");
            Ok(document.clone())
        }
    }

    struct TestSettingsRuntime;

    impl SettingsRuntime for TestSettingsRuntime {
        fn validate(
            &self,
            _current: &Settings,
            _proposed: &Settings,
        ) -> Result<(), RuntimeSettingError> {
            Ok(())
        }

        fn apply(&self, _settings: &Settings) {}
    }

    fn capabilities_without_tray() -> RuntimeCapabilities {
        RuntimeCapabilities::new(Capability {
            supported: false,
            status: CapabilityStatus::NotImplemented,
        })
    }

    #[test]
    fn tray_open_refresh_requests_only_when_stale_and_not_throttled() {
        assert_eq!(
            tray_open_refresh_decision(600_000, None, None, false),
            TrayOpenRefreshDecision::Request
        );
        assert_eq!(
            tray_open_refresh_decision(600_000, Some(550_001), None, false),
            TrayOpenRefreshDecision::SkipFresh
        );
        assert_eq!(
            tray_open_refresh_decision(600_000, Some(200_000), Some(590_001), false),
            TrayOpenRefreshDecision::SkipThrottled
        );
        assert_eq!(
            tray_open_refresh_decision(600_000, Some(200_000), Some(500_000), false),
            TrayOpenRefreshDecision::Request
        );
        assert_eq!(
            tray_open_refresh_decision(600_000, Some(200_000), None, true),
            TrayOpenRefreshDecision::SkipActive
        );
    }

    #[test]
    fn fresh_startup_creates_migrates_and_seeds_database() {
        let directory = tempfile::TempDir::new().expect("create app data directory");
        let database_path = directory.path().join("nested").join("burnly.sqlite3");

        drop(initialize(&database_path, "Asia/Jakarta", 100).expect("initialize application"));

        let connection = Connection::open(database_path).expect("reopen database");
        assert_eq!(pragma_i64(&connection, "user_version"), 3);
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
            .pragma_update(None, "user_version", 4)
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
        assert_eq!(pragma_i64(&connection, "user_version"), 4);
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
        assert_eq!(response["data"]["diagnostics"]["desktopEvidence"], true);
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
        let collector = Arc::new(
            CcusageCollector::development(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .parent()
                    .expect("workspace root")
                    .join("tests/fixtures/collectors/ccusage/process/fake-collector.sh"),
            )
            .expect("collector"),
        );
        let coordinator = RefreshCoordinator::new(
            collector,
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
        use std::{thread, time::Duration};

        let directory = tempfile::TempDir::new().expect("create app data directory");
        let database_path = directory.path().join("burnly.sqlite3");
        drop(initialize(&database_path, "UTC", 100).expect("initialize application"));

        let app = tauri::test::mock_builder()
            .invoke_handler(crate::ipc::invoke_handler())
            .manage(BootstrapService::new(
                env!("CARGO_PKG_VERSION"),
                CONTRACT_VERSION,
                FixedBootstrapStore,
                capabilities_without_tray(),
            ))
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("build mock tauri app");
        let collector = Arc::new(
            CcusageCollector::development(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .parent()
                    .expect("workspace root")
                    .join("tests/fixtures/collectors/ccusage/process/fake-collector.sh"),
            )
            .expect("development collector"),
        );
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

        let terminal = (0..1_000)
            .find_map(|_| {
                let response =
                    tauri::test::get_ipc_response(&webview, request("refresh_get_state"))
                        .expect("invoke refresh state")
                        .deserialize::<Value>()
                        .expect("deserialize refresh state");
                if response["data"]["status"] == "running" {
                    thread::sleep(Duration::from_millis(1));
                    None
                } else {
                    Some(response)
                }
            })
            .expect("refresh reaches terminal state");

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
        assert_eq!(daily_count, 6);
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

    fn settings_count(connection: &Connection) -> i64 {
        connection
            .query_row("SELECT COUNT(*) FROM app_settings", [], |row| row.get(0))
            .expect("count settings")
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
        invoke_from_window("main", command)
    }

    fn invoke_from_window(label: &str, command: &str) -> Value {
        let app = tauri::test::mock_builder()
            .invoke_handler(crate::ipc::invoke_handler())
            .manage(BootstrapService::new(
                env!("CARGO_PKG_VERSION"),
                CONTRACT_VERSION,
                FixedBootstrapStore,
                capabilities_without_tray(),
            ))
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("build mock tauri app");
        let webview = tauri::WebviewWindowBuilder::new(&app, label, Default::default())
            .build()
            .expect("build mock webview");

        tauri::test::get_ipc_response(&webview, request(command))
            .expect("invoke command")
            .deserialize::<Value>()
            .expect("deserialize command response")
    }

    fn request(command: &str) -> InvokeRequest {
        request_with_body(command, Value::Object(Default::default()))
    }

    fn request_with_body(command: &str, body: Value) -> InvokeRequest {
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
            body: tauri::ipc::InvokeBody::Json(body),
            headers: Default::default(),
            invoke_key: tauri::test::INVOKE_KEY.to_owned(),
        }
    }
}
