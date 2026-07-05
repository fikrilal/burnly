use std::path::Path;
use std::sync::{Arc, Mutex};

use rusqlite::Connection;
use serde_json::Value;
use tauri::webview::InvokeRequest;

use super::{RuntimeCapabilities, StartupError};
use crate::application::bootstrap::{
    BootstrapError, BootstrapService, BootstrapStorage, BootstrapStore, Capability,
    CapabilityStatus,
};
use crate::application::ports::collector::Collector;
use crate::application::ports::settings_store::{SettingsStore, SettingsStoreError};
use crate::application::settings::{RuntimeSettingError, SettingsRuntime};
use crate::application::update::{UnavailableUpdateRuntime, UpdateService};
use crate::domain::settings::{Settings, SettingsDocument};
use crate::infrastructure::collectors::antigravity::AntigravityCollector;
use crate::infrastructure::collectors::ccusage::CcusageCollector;
use crate::infrastructure::collectors::cline::ClineCollector;
use crate::infrastructure::collectors::routed::RoutedCollector;
use crate::infrastructure::collectors::zcode::ZCodeCollector;
use crate::infrastructure::database::Database;
use crate::ipc::CONTRACT_VERSION;

pub(super) struct FixedBootstrapStore;

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

pub(super) struct TestSettingsStore {
    pub(super) document: Mutex<SettingsDocument>,
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
        *document =
            SettingsDocument::new(settings.clone(), expected_revision + 1).expect("valid document");
        Ok(document.clone())
    }
}

pub(super) struct TestSettingsRuntime;

impl SettingsRuntime for TestSettingsRuntime {
    fn validate(
        &self,
        _current: &Settings,
        _proposed: &Settings,
    ) -> Result<(), RuntimeSettingError> {
        Ok(())
    }

    fn prepare_update(
        &self,
        _current: &Settings,
        _proposed: &Settings,
    ) -> Result<(), RuntimeSettingError> {
        Ok(())
    }

    fn rollback_update(&self, _current: &Settings) -> Result<(), RuntimeSettingError> {
        Ok(())
    }

    fn commit_update(&self, _settings: &Settings) {}
}

pub(super) fn capabilities_without_tray() -> RuntimeCapabilities {
    RuntimeCapabilities::new(
        Capability {
            supported: false,
            status: CapabilityStatus::NotImplemented,
        },
        RuntimeCapabilities::launch_at_login_not_implemented(),
        RuntimeCapabilities::update_not_implemented(),
    )
}

pub(super) fn unavailable_update_service() -> UpdateService {
    UpdateService::new(Arc::new(UnavailableUpdateRuntime::new()))
}

pub(super) fn write_packaged_sidecar_manifest(resource_directory: &Path) {
    let sidecar_directory = resource_directory.join("sidecars").join("ccusage");
    std::fs::create_dir_all(&sidecar_directory).expect("create sidecar directory");
    std::fs::write(sidecar_directory.join("manifest.json"), "{}").expect("write sidecar manifest");
}

pub(super) fn fake_ccusage_collector() -> Arc<dyn Collector> {
    Arc::new(CcusageCollector::development(fake_ccusage_path()).expect("development collector"))
}

#[cfg(unix)]
pub(super) fn composed_refresh_collector(data_root: &Path) -> Arc<dyn Collector> {
    Arc::new(RoutedCollector::new(
        fake_ccusage_collector(),
        Arc::new(ClineCollector::from_database_path(
            data_root.join("missing-cline-sessions.db"),
        )),
        Arc::new(ZCodeCollector::from_database_path(
            data_root.join("missing-zcode-usage.db"),
        )),
        Arc::new(AntigravityCollector::empty_from_data_root(
            data_root.join("empty-antigravity"),
        )),
    ))
}

pub(super) fn settings_count(connection: &Connection) -> i64 {
    connection
        .query_row("SELECT COUNT(*) FROM app_settings", [], |row| row.get(0))
        .expect("count settings")
}

pub(super) fn expect_startup_error(
    result: Result<Database, StartupError>,
    message: &str,
) -> StartupError {
    match result {
        Ok(_) => panic!("{message}"),
        Err(error) => error,
    }
}

pub(super) fn setting_text(connection: &Connection, column: &str) -> String {
    connection
        .query_row(
            &format!("SELECT {column} FROM app_settings WHERE id = 1"),
            [],
            |row| row.get(0),
        )
        .expect("query text setting")
}

pub(super) fn setting_i64(connection: &Connection, column: &str) -> i64 {
    connection
        .query_row(
            &format!("SELECT {column} FROM app_settings WHERE id = 1"),
            [],
            |row| row.get(0),
        )
        .expect("query integer setting")
}

pub(super) fn pragma_i64(connection: &Connection, name: &str) -> i64 {
    connection
        .pragma_query_value(None, name, |row| row.get(0))
        .expect("query pragma")
}

pub(super) fn invoke(command: &str) -> Value {
    invoke_from_window("main", command)
}

pub(super) fn invoke_from_window(label: &str, command: &str) -> Value {
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
    let webview = tauri::WebviewWindowBuilder::new(&app, label, Default::default())
        .build()
        .expect("build mock webview");

    tauri::test::get_ipc_response(&webview, request(command))
        .expect("invoke command")
        .deserialize::<Value>()
        .expect("deserialize command response")
}

pub(super) fn request(command: &str) -> InvokeRequest {
    request_with_body(command, Value::Object(Default::default()))
}

pub(super) fn request_with_body(command: &str, body: Value) -> InvokeRequest {
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

fn fake_ccusage_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .join("tests/fixtures/collectors/ccusage/process/fake-collector.sh")
}
