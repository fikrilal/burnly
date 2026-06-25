//! Application composition root.
//!
//! This module selects concrete infrastructure and platform integrations. Other
//! modules receive constructed dependencies instead of constructing their own.

use std::path::Path;
use std::sync::{Arc, Mutex};

use iana_time_zone::GetTimezoneError;
use tauri::{Listener, Manager, RunEvent, Runtime, WindowEvent};
use thiserror::Error;

use crate::application::bootstrap::{
    BootstrapService, CapabilityStatus, NativeNotificationCapability, RuntimeCapabilities,
    RuntimeSettings, StartupRecoveryState,
};
use crate::application::budget_evaluation::BudgetEvaluationService;
use crate::application::budget_notifications::BudgetNotificationService;
use crate::application::budget_progress::BudgetProgressQuery;
use crate::application::budgets::BudgetService;
use crate::application::collection::CollectorFailure;
use crate::application::database_maintenance::DatabaseMaintenanceService;
use crate::application::diagnostics::{DiagnosticsService, RuntimeDiagnosticRecord};
use crate::application::export::ExportService;
use crate::application::history::HistoryService;
use crate::application::history_deletion::HistoryDeletionService;
use crate::application::ports::database_maintenance::{MaintenanceActivity, MaintenanceGuard};
use crate::application::ports::notification::{NotificationPermission, NotificationPort};
use crate::application::ports::window_actions::WindowActions;
use crate::application::reconciliation::RefreshTrigger;
use crate::application::refresh::{
    RefreshCoordinator, RefreshCoordinatorHooks, RefreshEventSink, RefreshPolicy, RefreshScheduler,
    RefreshSchedulerError, RefreshSnapshot, RefreshStatus,
};
use crate::application::settings::{RuntimeSettingError, SettingsRuntime, SettingsService};
use crate::application::usage::{
    CalendarQuery, DayDetailQuery, OverviewQuery, SessionQuery, TraySummaryQuery,
};
use crate::domain::settings::{CloseBehavior, Settings};
use crate::infrastructure::bootstrap_store::SqliteBootstrapStore;
use crate::infrastructure::collectors::ccusage::CcusageCollector;
use crate::infrastructure::database::{
    Database, PersistenceError, PersistenceErrorKind, SqliteBudgetNotificationStore,
    SqliteBudgetStore, SqliteBudgetUsageStore, SqliteCalendarStore, SqliteDatabaseMaintenanceStore,
    SqliteDiagnosticsStore, SqliteExportStore, SqliteHistoryDeletionStore, SqliteHistoryStore,
    SqliteOverviewStore, SqliteReconciliationStore, SqliteSessionStore, SqliteTraySummaryStore,
};
use crate::infrastructure::settings_store::SqliteSettingsStore;
use crate::ipc::refresh_event_sink;
use crate::ipc::CONTRACT_VERSION;
use crate::platform::export::DesktopExportWriter;
use crate::platform::lifecycle;
use crate::platform::logs::DesktopLogReveal;
use crate::platform::system_clock::SystemClock;
use crate::platform::{
    database_path, notifications::NativeNotificationAdapter, single_instance, system_clock,
    system_timezone, tray,
};

const TRAY_OPEN_STALE_AFTER_MS: i64 = 5 * 60 * 1_000;
const TRAY_OPEN_REFRESH_THROTTLE_MS: i64 = 60 * 1_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StartupErrorKind {
    DatabasePath,
    Timezone,
    Clock,
    ResourceDir,
    Collector,
    RefreshScheduler,
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
            Self::PrivacyPolicy => StartupErrorKind::PrivacyPolicy,
            Self::Persistence(error) => StartupErrorKind::Persistence(error.kind()),
        }
    }
}

pub(crate) fn run() {
    let app = tauri::Builder::default()
        .plugin(single_instance::plugin())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(crate::ipc::invoke_handler())
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if let Some(settings) = window.app_handle().try_state::<RuntimeSettings>() {
                    lifecycle::handle_close_request(window, api, settings.close_behavior());
                }
            }
        })
        .setup(|app| {
            setup_runtime(app).map_err(|error| {
                eprintln!("Burnly startup failed ({:?})", error.kind());
                Box::new(error) as Box<dyn std::error::Error>
            })
        })
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
        #[cfg(target_os = "macos")]
        RunEvent::Reopen { .. } => {
            let _ = lifecycle::activate_main_window(app);
        }
        _ => {}
    }
}

fn setup_runtime<R: Runtime>(app: &mut tauri::App<R>) -> Result<(), StartupError> {
    let database_path = database_path::resolve(app.handle()).map_err(StartupError::DatabasePath)?;
    let reporting_timezone = system_timezone::resolve().map_err(StartupError::Timezone)?;
    let created_at_ms = system_clock::now_epoch_ms().map_err(StartupError::Clock)?;
    let database = match initialize(&database_path, &reporting_timezone, created_at_ms) {
        Ok(database) => database,
        Err(StartupError::Persistence(error)) => {
            eprintln!(
                "Burnly persistence startup entered recovery mode ({:?})",
                error.kind()
            );
            app.manage(StartupRecoveryState);
            app.manage(DatabaseMaintenanceService::new(
                Arc::new(SqliteDatabaseMaintenanceStore::new(database_path)),
                Arc::new(RecoveryMaintenanceGuard),
            ));
            return Ok(());
        }
        Err(error) => return Err(error),
    };
    let (_, background_refresh_enabled, refresh_interval_minutes, _, close_behavior, ..) = database
        .read_settings()
        .map_err(StartupError::Persistence)?;
    let refresh_policy = refresh_policy(background_refresh_enabled, refresh_interval_minutes);
    let tray_state = Arc::new(Mutex::new(None));
    let budget_progress_query = build_budget_progress_query(&database_path)?;
    let notification_port: Arc<dyn NotificationPort> =
        Arc::new(NativeNotificationAdapter::new(app.handle().clone()));
    let refresh_event_sink = runtime_refresh_event_sink(
        app.handle().clone(),
        tray_state.clone(),
        budget_progress_query.clone(),
    );
    let refresh_coordinator = build_refresh_coordinator(
        app,
        &database_path,
        refresh_event_sink,
        notification_port.clone(),
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
        &tray_snapshot(&refresh_coordinator.snapshot(), &budget_progress_query),
    )
    .ok();
    if let Some(controller) = tray_controller.clone() {
        *tray_state.lock().expect("tray state lock is poisoned") = Some(controller.clone());
        controller.update(&tray_snapshot(
            &refresh_coordinator.snapshot(),
            &budget_progress_query,
        ));
        app.manage(controller);
    }
    install_tray_invalidation_listener(app.handle().clone(), budget_progress_query.clone());
    let tray_capability = match tray_controller {
        Some(_) => RuntimeCapabilities::tray_available(),
        None => RuntimeCapabilities::tray_unavailable(),
    };
    let notification_capability = notification_port.capability();
    let runtime_capabilities = RuntimeCapabilities::with_native_notifications(
        tray_capability,
        NativeNotificationCapability {
            supported: notification_capability.supported,
            status: if notification_capability.supported {
                CapabilityStatus::Available
            } else {
                CapabilityStatus::Unavailable
            },
            permission: notification_capability.permission,
        },
    );

    app.manage(DatabaseMaintenanceService::new(
        Arc::new(SqliteDatabaseMaintenanceStore::new(database_path.clone())),
        Arc::new(RuntimeMaintenanceGuard {
            coordinator: refresh_coordinator.clone(),
        }),
    ));
    app.manage(
        Arc::new(lifecycle::DesktopWindowActions::new(app.handle().clone()))
            as Arc<dyn WindowActions>,
    );
    app.manage(refresh_coordinator.clone());
    app.manage(refresh_scheduler);
    app.manage(runtime_settings.clone());
    app.manage(build_overview_query(&database_path)?);
    let tray_summary_query = build_tray_summary_query(&database_path)?;
    let tray_open_refresh = TrayOpenRefreshController::new(
        reporting_timezone.clone(),
        tray_summary_query.clone(),
        refresh_coordinator.clone(),
        Arc::new(SystemClock),
    );
    tray_open_refresh.request_startup_refresh_if_stale();
    app.manage(tray_summary_query);
    app.manage(tray_open_refresh);
    app.manage(build_calendar_query(&database_path)?);
    app.manage(build_day_detail_query(&database_path)?);
    app.manage(build_session_query(&database_path)?);
    app.manage(budget_progress_query);
    let budget_database = Database::open(&database_path).map_err(StartupError::Persistence)?;
    app.manage(BudgetService::new(
        Arc::new(SqliteBudgetStore::new(budget_database)),
        Arc::new(SystemClock),
    ));
    app.manage(BootstrapService::new(
        env!("CARGO_PKG_VERSION"),
        CONTRACT_VERSION,
        SqliteBootstrapStore::new(database),
        runtime_capabilities,
    ));
    let settings_database = Database::open(&database_path).map_err(StartupError::Persistence)?;
    let settings_store = Arc::new(SqliteSettingsStore::new(settings_database));
    settings_store
        .enforce_current_project_path_policy()
        .map_err(|_| StartupError::PrivacyPolicy)?;
    let runtime = Arc::new(DesktopSettingsRuntime {
        app: app.handle().clone(),
        runtime_settings,
        launch_at_login_available: false,
        notifications: notification_port,
    });
    app.manage(SettingsService::new(
        settings_store.clone(),
        runtime,
        Arc::new(SystemClock),
    ));
    app.manage(DiagnosticsService::new(
        Arc::new(SqliteDiagnosticsStore::new(
            Database::open(&database_path).map_err(StartupError::Persistence)?,
        )),
        settings_store,
        Arc::new(DesktopLogReveal::new(app.handle().clone())),
        RuntimeDiagnosticRecord {
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
            contract_version: CONTRACT_VERSION,
            collector_initialized: true,
        },
    ));
    app.manage(HistoryService::new(
        Arc::new(SqliteHistoryStore::new(
            Database::open(&database_path).map_err(StartupError::Persistence)?,
        )),
        Arc::new(SystemClock),
    ));
    app.manage(ExportService::new(
        Arc::new(SqliteExportStore::new(
            Database::open(&database_path).map_err(StartupError::Persistence)?,
        )),
        Arc::new(DesktopExportWriter::new(app.handle().clone())),
    ));
    app.manage(HistoryDeletionService::new(Arc::new(
        SqliteHistoryDeletionStore::new(
            Database::open(&database_path).map_err(StartupError::Persistence)?,
        ),
    )));
    Ok(())
}

struct RecoveryMaintenanceGuard;

impl MaintenanceGuard for RecoveryMaintenanceGuard {
    fn activity(&self) -> MaintenanceActivity {
        MaintenanceActivity::Idle
    }
}

struct RuntimeMaintenanceGuard {
    coordinator: RefreshCoordinator,
}

impl MaintenanceGuard for RuntimeMaintenanceGuard {
    fn activity(&self) -> MaintenanceActivity {
        match self.coordinator.snapshot().status {
            RefreshStatus::Queued | RefreshStatus::Running | RefreshStatus::Cancelling => {
                MaintenanceActivity::Busy
            }
            RefreshStatus::Idle
            | RefreshStatus::Succeeded
            | RefreshStatus::Partial
            | RefreshStatus::Failed => MaintenanceActivity::Idle,
        }
    }
}

struct DesktopSettingsRuntime<R: Runtime> {
    app: tauri::AppHandle<R>,
    runtime_settings: RuntimeSettings,
    launch_at_login_available: bool,
    notifications: Arc<dyn NotificationPort>,
}

impl<R: Runtime> SettingsRuntime for DesktopSettingsRuntime<R> {
    fn validate(&self, current: &Settings, proposed: &Settings) -> Result<(), RuntimeSettingError> {
        if proposed.launch_at_login()
            && !current.launch_at_login()
            && !self.launch_at_login_available
        {
            return Err(RuntimeSettingError::LaunchAtLoginUnavailable);
        }
        if proposed.notifications_enabled() && !current.notifications_enabled() {
            ensure_notification_permission(self.notifications.as_ref())?;
        }
        if proposed.store_project_paths() != current.store_project_paths() {
            return Err(RuntimeSettingError::ProjectPathRetentionRequiresPrivacyFlow);
        }
        Ok(())
    }

    fn apply(&self, settings: &Settings) {
        self.runtime_settings.update(settings);
        if let Some(scheduler) = self.app.try_state::<RefreshScheduler>() {
            scheduler.apply_policy(refresh_policy(
                settings.background_refresh_enabled(),
                settings.refresh_interval_minutes(),
            ));
        }
        if let Some(coordinator) = self.app.try_state::<RefreshCoordinator>() {
            coordinator.set_aggregation_timezone(settings.reporting_timezone());
        }
    }
}

fn ensure_notification_permission(
    notifications: &dyn NotificationPort,
) -> Result<(), RuntimeSettingError> {
    let capability = notifications.capability();
    if !capability.supported {
        return Err(RuntimeSettingError::NotificationsUnavailable);
    }
    let permission = match capability.permission {
        NotificationPermission::Prompt => notifications.request_permission(),
        permission => permission,
    };
    if permission == NotificationPermission::Granted {
        Ok(())
    } else {
        Err(RuntimeSettingError::NotificationsUnavailable)
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
        Some(tray::TrayAction::OpenDetails) => {
            let _ = lifecycle::open_details_window(app);
        }
        Some(tray::TrayAction::Refresh) => {
            if let Some(coordinator) = app.try_state::<RefreshCoordinator>() {
                coordinator.request_refresh(RefreshTrigger::Manual);
            }
        }
        Some(tray::TrayAction::Quit) => app.exit(0),
        None => {}
    }
}

fn refresh_policy(
    background_refresh_enabled: bool,
    refresh_interval_minutes: i64,
) -> RefreshPolicy {
    if background_refresh_enabled {
        RefreshPolicy::enabled_minutes(refresh_interval_minutes)
    } else {
        RefreshPolicy::disabled()
    }
}

fn build_overview_query(database_path: &Path) -> Result<OverviewQuery, StartupError> {
    let database = Database::open(database_path).map_err(StartupError::Persistence)?;
    Ok(OverviewQuery::new(
        Arc::new(SqliteOverviewStore::new(database)),
        Arc::new(SystemClock),
    ))
}

fn build_tray_summary_query(database_path: &Path) -> Result<TraySummaryQuery, StartupError> {
    let database = Database::open(database_path).map_err(StartupError::Persistence)?;
    Ok(TraySummaryQuery::new(
        Arc::new(SqliteTraySummaryStore::new(database)),
        Arc::new(SystemClock),
    ))
}

fn build_calendar_query(database_path: &Path) -> Result<CalendarQuery, StartupError> {
    let database = Database::open(database_path).map_err(StartupError::Persistence)?;
    Ok(CalendarQuery::new(Arc::new(SqliteCalendarStore::new(
        database,
    ))))
}

fn build_day_detail_query(database_path: &Path) -> Result<DayDetailQuery, StartupError> {
    let database = Database::open(database_path).map_err(StartupError::Persistence)?;
    Ok(DayDetailQuery::new(
        Arc::new(SqliteCalendarStore::new(database)),
        Arc::new(SystemClock),
    ))
}

fn build_session_query(database_path: &Path) -> Result<SessionQuery, StartupError> {
    let database = Database::open(database_path).map_err(StartupError::Persistence)?;
    Ok(SessionQuery::new(Arc::new(SqliteSessionStore::new(
        Arc::new(Mutex::new(database)),
    ))))
}

fn build_budget_progress_query(
    database_path: &Path,
) -> Result<Arc<BudgetProgressQuery>, StartupError> {
    Ok(Arc::new(BudgetProgressQuery::new(
        Arc::new(SqliteBudgetStore::new(
            Database::open(database_path).map_err(StartupError::Persistence)?,
        )),
        Arc::new(SqliteBudgetUsageStore::new(
            Database::open(database_path).map_err(StartupError::Persistence)?,
        )),
        Arc::new(SqliteSettingsStore::new(
            Database::open(database_path).map_err(StartupError::Persistence)?,
        )),
        Arc::new(SystemClock),
    )))
}

fn build_refresh_coordinator<R: Runtime>(
    app: &tauri::App<R>,
    database_path: &Path,
    refresh_event_sink: Arc<dyn RefreshEventSink>,
    notifications: Arc<dyn NotificationPort>,
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

    compose_refresh_coordinator(database_path, collector, refresh_event_sink, notifications)
}

fn compose_refresh_coordinator(
    database_path: &Path,
    collector: Arc<CcusageCollector>,
    refresh_event_sink: Arc<dyn RefreshEventSink>,
    notifications: Arc<dyn NotificationPort>,
) -> Result<RefreshCoordinator, StartupError> {
    let write_database = Database::open(database_path).map_err(StartupError::Persistence)?;
    let (aggregation_timezone, ..) = write_database
        .read_settings()
        .map_err(StartupError::Persistence)?;
    let store = Arc::new(SqliteReconciliationStore::new(write_database));
    let budget_store = Arc::new(SqliteBudgetStore::new(
        Database::open(database_path).map_err(StartupError::Persistence)?,
    ));
    let budget_usage_store = Arc::new(SqliteBudgetUsageStore::new(
        Database::open(database_path).map_err(StartupError::Persistence)?,
    ));
    let budget_evaluator = BudgetEvaluationService::new(budget_store, budget_usage_store);
    let budget_notifications = Arc::new(BudgetNotificationService::new(
        budget_evaluator,
        Arc::new(SqliteSettingsStore::new(
            Database::open(database_path).map_err(StartupError::Persistence)?,
        )),
        Arc::new(SqliteBudgetNotificationStore::new(
            Database::open(database_path).map_err(StartupError::Persistence)?,
        )),
        notifications,
    ));
    let clock = Arc::new(SystemClock);

    Ok(RefreshCoordinator::with_event_sink_and_budget_evaluator(
        collector,
        store.clone(),
        store,
        clock,
        RefreshCoordinatorHooks::new(refresh_event_sink, budget_notifications),
        env!("CARGO_PKG_VERSION"),
        aggregation_timezone,
    ))
}

struct RuntimeRefreshEventSink<R: Runtime> {
    frontend: Arc<dyn RefreshEventSink>,
    tray: Arc<Mutex<Option<tray::TrayController<R>>>>,
    budget_progress: Arc<BudgetProgressQuery>,
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
            tray.update(&tray_snapshot(&snapshot, &self.budget_progress));
        }
    }
}

fn runtime_refresh_event_sink<R: Runtime>(
    app: tauri::AppHandle<R>,
    tray: Arc<Mutex<Option<tray::TrayController<R>>>>,
    budget_progress: Arc<BudgetProgressQuery>,
) -> Arc<dyn RefreshEventSink> {
    Arc::new(RuntimeRefreshEventSink {
        frontend: refresh_event_sink(app),
        tray,
        budget_progress,
    })
}

fn install_tray_invalidation_listener<R: Runtime>(
    app: tauri::AppHandle<R>,
    budget_progress: Arc<BudgetProgressQuery>,
) {
    let listener_app = app.clone();
    app.listen("burnly://v1/data-invalidated", move |_| {
        if let (Some(controller), Some(coordinator)) = (
            listener_app.try_state::<tray::TrayController<R>>(),
            listener_app.try_state::<RefreshCoordinator>(),
        ) {
            controller.update(&tray_snapshot(&coordinator.snapshot(), &budget_progress));
        }
    });
}

pub(crate) fn tray_snapshot(
    snapshot: &RefreshSnapshot,
    budget_progress: &BudgetProgressQuery,
) -> tray::TraySnapshot {
    tray::TraySnapshot {
        status: tray_refresh_status(snapshot.status),
        last_successful_refresh_at_ms: snapshot.last_successful_refresh_at_ms,
        budget_summary: budget_progress
            .current()
            .ok()
            .and_then(|progress| progress.tray_summary),
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
            self.coordinator.request_refresh(trigger);
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
    use crate::application::ports::notification::{
        NotificationCapability, NotificationDeliveryOutcome, NotificationMessage,
    };
    use crate::application::ports::settings_store::{SettingsStore, SettingsStoreError};
    use crate::domain::settings::{Settings, SettingsDocument};

    use rusqlite::Connection;
    use serde_json::{json, Value};
    use tauri::webview::InvokeRequest;

    use super::*;

    struct FixedBootstrapStore;

    #[cfg(unix)]
    struct TestNotificationPort;

    #[cfg(unix)]
    impl NotificationPort for TestNotificationPort {
        fn capability(&self) -> NotificationCapability {
            NotificationCapability {
                supported: false,
                permission: NotificationPermission::Unknown,
            }
        }

        fn request_permission(&self) -> NotificationPermission {
            NotificationPermission::Unknown
        }

        fn deliver(&self, _message: &NotificationMessage) -> NotificationDeliveryOutcome {
            NotificationDeliveryOutcome::Failed
        }
    }

    struct PermissionNotificationPort {
        initial: NotificationPermission,
        requested: NotificationPermission,
        requests: Mutex<u32>,
    }

    impl NotificationPort for PermissionNotificationPort {
        fn capability(&self) -> NotificationCapability {
            NotificationCapability {
                supported: true,
                permission: self.initial,
            }
        }

        fn request_permission(&self) -> NotificationPermission {
            *self.requests.lock().expect("requests lock") += 1;
            self.requested
        }

        fn deliver(&self, _message: &NotificationMessage) -> NotificationDeliveryOutcome {
            NotificationDeliveryOutcome::Failed
        }
    }

    impl BootstrapStore for FixedBootstrapStore {
        fn read_bootstrap_storage(&self) -> Result<BootstrapStorage, BootstrapError> {
            Ok(BootstrapStorage {
                reporting_timezone: "Asia/Jakarta".to_owned(),
                background_refresh_enabled: false,
                refresh_interval_minutes: 15,
                launch_at_login: false,
                close_behavior: "quit".to_owned(),
                notifications_enabled: false,
                store_project_paths: false,
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

        fn replace_project_path_retention(
            &self,
            expected_revision: i64,
            retain_paths: bool,
            _updated_at_ms: i64,
        ) -> Result<
            crate::application::ports::settings_store::ProjectPathRetentionResult,
            SettingsStoreError,
        > {
            let mut document = self.document.lock().expect("settings lock");
            if document.revision() != expected_revision {
                return Err(SettingsStoreError::Conflict);
            }
            let current = document.settings();
            let settings = Settings::new(
                current.reporting_timezone().to_owned(),
                current.background_refresh_enabled(),
                current.refresh_interval_minutes(),
                current.launch_at_login(),
                current.close_behavior().as_str(),
                current.notifications_enabled(),
                retain_paths,
            )
            .expect("valid settings");
            *document =
                SettingsDocument::new(settings, expected_revision + 1).expect("valid document");
            Ok(
                crate::application::ports::settings_store::ProjectPathRetentionResult {
                    settings: document.clone(),
                    cleared_paths: 0,
                },
            )
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
    fn notification_enablement_requests_prompted_permission_and_rejects_denial() {
        let granted = PermissionNotificationPort {
            initial: NotificationPermission::Prompt,
            requested: NotificationPermission::Granted,
            requests: Mutex::new(0),
        };
        assert_eq!(ensure_notification_permission(&granted), Ok(()));
        assert_eq!(*granted.requests.lock().expect("requests"), 1);

        let denied = PermissionNotificationPort {
            initial: NotificationPermission::Denied,
            requested: NotificationPermission::Granted,
            requests: Mutex::new(0),
        };
        assert_eq!(
            ensure_notification_permission(&denied),
            Err(RuntimeSettingError::NotificationsUnavailable)
        );
        assert_eq!(*denied.requests.lock().expect("requests"), 0);
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

    #[test]
    fn tauri_bridge_updates_settings_when_scheduler_state_is_available() {
        let initial = Settings::new(
            "Asia/Jakarta".to_owned(),
            false,
            15,
            false,
            "quit",
            false,
            false,
        )
        .expect("valid settings");
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
                        "reportingTimezone": "UTC",
                        "backgroundRefreshEnabled": true,
                        "refreshIntervalMinutes": 30,
                        "launchAtLogin": false,
                        "closeBehavior": "hide",
                        "notificationsEnabled": false,
                        "storeProjectPaths": false
                    }
                }),
            ),
        )
        .expect("invoke settings update")
        .deserialize::<Value>()
        .expect("deserialize settings response");

        assert_eq!(response["ok"], true);
        assert_eq!(response["data"]["reportingTimezone"], "UTC");
        assert_eq!(response["data"]["backgroundRefreshEnabled"], true);
        assert_eq!(response["data"]["refreshIntervalMinutes"], 30);
        assert_eq!(response["data"]["closeBehavior"], "hide");
        assert_eq!(response["data"]["revision"], 2);

        let privacy_response = tauri::test::get_ipc_response(
            &webview,
            request_with_body(
                "settings_update_project_path_retention",
                json!({
                    "request": {
                        "expectedRevision": 2,
                        "retainPaths": true
                    }
                }),
            ),
        )
        .expect("invoke privacy update")
        .deserialize::<Value>()
        .expect("deserialize privacy response");

        assert_eq!(privacy_response["ok"], true);
        assert_eq!(
            privacy_response["data"]["settings"]["storeProjectPaths"],
            true
        );
        assert_eq!(privacy_response["data"]["settings"]["revision"], 3);
        assert_eq!(privacy_response["data"]["clearedPaths"], 0);
    }

    #[test]
    fn tauri_bridge_runs_budget_crud_with_exact_string_contracts() {
        let directory = tempfile::TempDir::new().expect("temporary directory");
        let database_path = directory.path().join("burnly.sqlite3");
        let mut database = Database::open(database_path).expect("open database");
        database.migrate_to_latest().expect("migrate database");
        let app = tauri::test::mock_builder()
            .invoke_handler(crate::ipc::invoke_handler())
            .manage(BudgetService::new(
                Arc::new(SqliteBudgetStore::new(database)),
                Arc::new(SystemClock),
            ))
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("build mock tauri app");
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("build mock webview");

        let created = invoke_webview(
            &webview,
            "budgets_create",
            json!({
                "request": {
                    "budget": {
                        "name": "Monthly tokens",
                        "limit": { "kind": "tokens", "value": "100000" },
                        "period": "monthly",
                        "scope": { "kind": "global" },
                        "enabled": true,
                        "thresholds": [
                            { "basisPoints": 10000, "enabled": true },
                            { "basisPoints": 8000, "enabled": true }
                        ]
                    }
                }
            }),
        );
        assert_eq!(created["ok"], true);
        assert_eq!(created["data"]["id"], "1");
        assert_eq!(created["data"]["revision"], "1");
        assert_eq!(created["data"]["thresholds"][0]["basisPoints"], 8000);

        let listed = invoke_webview(&webview, "budgets_list", json!({}));
        assert_eq!(listed["data"]["items"][0]["id"], "1");

        let fetched = invoke_webview(
            &webview,
            "budgets_get",
            json!({ "request": { "budgetId": "1" } }),
        );
        assert_eq!(fetched["data"]["name"], "Monthly tokens");

        let updated = invoke_webview(
            &webview,
            "budgets_update",
            json!({
                "request": {
                    "budgetId": "1",
                    "expectedRevision": "1",
                    "budget": {
                        "name": "Daily tokens",
                        "limit": { "kind": "tokens", "value": "5000" },
                        "period": "daily",
                        "scope": { "kind": "global" },
                        "enabled": true,
                        "thresholds": [
                            { "basisPoints": 9000, "enabled": true }
                        ]
                    }
                }
            }),
        );
        assert_eq!(updated["data"]["revision"], "2");
        assert_eq!(updated["data"]["period"], "daily");

        let disabled = invoke_webview(
            &webview,
            "budgets_disable",
            json!({
                "request": {
                    "budgetId": "1",
                    "expectedRevision": "2"
                }
            }),
        );
        assert_eq!(disabled["data"]["enabled"], false);
        assert_eq!(disabled["data"]["revision"], "3");

        let enabled = invoke_webview(
            &webview,
            "budgets_enable",
            json!({
                "request": {
                    "budgetId": "1",
                    "expectedRevision": "3"
                }
            }),
        );
        assert_eq!(enabled["data"]["enabled"], true);
        assert_eq!(enabled["data"]["revision"], "4");

        let conflict = invoke_webview(
            &webview,
            "budgets_delete",
            json!({
                "request": {
                    "budgetId": "1",
                    "expectedRevision": "1"
                }
            }),
        );
        assert_eq!(conflict["ok"], false);
        assert_eq!(conflict["error"]["code"], "budgets.revision_conflict");

        let deleted = invoke_webview(
            &webview,
            "budgets_delete",
            json!({
                "request": {
                    "budgetId": "1",
                    "expectedRevision": "4"
                }
            }),
        );
        assert_eq!(deleted["ok"], true);
        assert_eq!(deleted["data"]["budgetId"], "1");
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
            Arc::new(TestNotificationPort),
        )
        .expect("coordinator");
        assert!(app.manage(coordinator));
        assert!(app.manage(build_overview_query(&database_path).expect("overview query")));
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

        let overview = tauri::test::get_ipc_response(
            &webview,
            request_with_body(
                "usage_get_overview",
                json!({
                    "request": {
                        "startDate": "2026-06-13",
                        "endDate": "2026-06-14",
                        "reportingTimezone": "UTC"
                    }
                }),
            ),
        )
        .expect("invoke usage overview")
        .deserialize::<Value>()
        .expect("deserialize usage overview");

        assert_eq!(overview["ok"], true);
        assert_eq!(overview["data"]["totalTokens"], "7500");
        assert_eq!(overview["data"]["activeDays"], 2);
        assert_eq!(overview["data"]["cost"]["amountMicros"], "1890000");
        assert_eq!(overview["data"]["cost"]["valuation"], "estimated");
        assert_eq!(overview["data"]["sources"][0]["source"], "claude-code");
        assert_eq!(overview["data"]["sources"][1]["source"], "codex");
        assert_eq!(overview["data"]["sources"][2]["source"], "opencode");
        assert_eq!(overview["data"]["dataStatus"], "current");
        assert!(overview["data"]["asOf"]
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
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
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

    fn invoke_webview(
        webview: &tauri::WebviewWindow<tauri::test::MockRuntime>,
        command: &str,
        body: Value,
    ) -> Value {
        tauri::test::get_ipc_response(webview, request_with_body(command, body))
            .expect("invoke command")
            .deserialize::<Value>()
            .expect("deserialize command response")
    }
}
