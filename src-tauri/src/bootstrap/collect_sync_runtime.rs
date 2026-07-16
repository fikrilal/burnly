//! Composes collect-sync orchestration at desktop startup.

use std::sync::Arc;

use tauri::{AppHandle, Manager, Runtime};

use crate::application::account::{AccountLifecycleListener, AccountService};
use crate::application::cloud_session::CloudSession;
use crate::application::collect_sync::{
    CollectSync, CollectSyncConfig, CollectSyncStatusSink, NoopCollectSyncStatusSink,
};
use crate::application::ports::collect_sync_remote::CollectSyncPlatform;
use crate::application::refresh::{CommittedDailyUploadSink, RefreshCoordinator};
use crate::infrastructure::cloud::client::CloudClient;
use crate::infrastructure::cloud::daily_usage_push::HttpCollectSyncRemote;
use crate::infrastructure::database::{
    Database, SqliteCollectSyncStore, SqliteDailyUsageExportStore,
};
use crate::platform::system_clock::SystemClock;

struct CollectSyncBridge {
    service: Arc<CollectSync>,
}

impl AccountLifecycleListener for CollectSyncBridge {
    fn on_signed_in(&self, user_id: &str) {
        self.service.on_signed_in(user_id);
    }

    fn on_signed_out(&self) {
        self.service.on_signed_out();
    }
}

impl CommittedDailyUploadSink for CollectSyncBridge {
    fn on_committed_daily_upload(
        &self,
        upload: crate::application::collect_sync::CommittedDailyUpload,
    ) {
        self.service.on_committed_daily_upload(upload);
    }
}

pub(crate) struct CollectSyncInstallArgs<'a, R: Runtime> {
    pub app: &'a AppHandle<R>,
    pub database_path: &'a std::path::Path,
    pub reporting_timezone: &'a str,
    pub app_version: &'a str,
    pub session: Option<Arc<CloudSession>>,
    pub authenticated_client: Option<Arc<CloudClient>>,
    pub account: &'a AccountService,
    pub refresh_coordinator: &'a RefreshCoordinator,
    pub device_id: Option<String>,
    pub device_name: String,
}

/// Best-effort install of collect-sync. Failures never block tray startup.
pub(crate) fn install_collect_sync<R: Runtime>(
    args: CollectSyncInstallArgs<'_, R>,
) -> Option<Arc<CollectSync>> {
    let CollectSyncInstallArgs {
        app,
        database_path,
        reporting_timezone,
        app_version,
        session,
        authenticated_client,
        account,
        refresh_coordinator,
        device_id,
        device_name,
    } = args;
    let session = session?;
    let client = authenticated_client?;
    let device_id = device_id.filter(|value| !value.trim().is_empty())?;

    let mut database = match Database::open(database_path) {
        Ok(database) => database,
        Err(error) => {
            eprintln!("Burnly collect-sync database open failed: {error}");
            return None;
        }
    };
    if let Err(error) = database.migrate_to_latest() {
        eprintln!("Burnly collect-sync migrate failed: {error}");
        return None;
    }

    let mut export_db = match Database::open(database_path) {
        Ok(database) => database,
        Err(error) => {
            eprintln!("Burnly collect-sync export database open failed: {error}");
            return None;
        }
    };
    let _ = export_db.migrate_to_latest();

    let export_store = Arc::new(SqliteDailyUsageExportStore::new(export_db));
    let collect_store = Arc::new(SqliteCollectSyncStore::new(database));
    let remote = Arc::new(HttpCollectSyncRemote::new(client));
    let status_sink: Arc<dyn CollectSyncStatusSink> = Arc::new(NoopCollectSyncStatusSink);

    let service = CollectSync::new(
        session,
        CollectSyncConfig {
            device_id,
            device_name,
            app_version: app_version.to_owned(),
            platform: current_platform(),
            reporting_timezone: reporting_timezone.to_owned(),
        },
        export_store,
        collect_store,
        remote,
        Arc::new(SystemClock),
        status_sink,
    );

    let bridge = Arc::new(CollectSyncBridge {
        service: service.clone(),
    });
    account.set_lifecycle_listener(Some(bridge.clone()));
    refresh_coordinator.set_committed_daily_upload_sink(bridge);

    service.on_startup();
    app.manage(service.clone());
    Some(service)
}

fn current_platform() -> CollectSyncPlatform {
    if cfg!(target_os = "macos") {
        CollectSyncPlatform::Macos
    } else if cfg!(target_os = "windows") {
        CollectSyncPlatform::Windows
    } else {
        CollectSyncPlatform::Linux
    }
}
