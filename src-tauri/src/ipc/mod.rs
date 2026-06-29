//! Tauri command, event, and transport mapping boundary.
//!
//! IPC handlers invoke application use cases and do not own product rules or
//! infrastructure behavior.

mod commands;
mod contract;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "response primitives are consumed by registered commands in Phase 2B"
    )
)]
mod response;
mod settings;
mod update;
mod usage;

pub(crate) use commands::refresh_event_sink;
pub(crate) use response::CONTRACT_VERSION;

pub(crate) fn invoke_handler<R: tauri::Runtime>() -> impl Fn(tauri::ipc::Invoke<R>) -> bool {
    tauri::generate_handler![
        commands::__burnly_contract_probe,
        commands::app_get_bootstrap,
        commands::app_get_capabilities,
        commands::app_hide_tray_panel,
        settings::settings_get,
        settings::settings_update,
        commands::refresh_get_state,
        commands::refresh_request,
        commands::refresh_cancel,
        update::update_get_state,
        update::update_check,
        update::update_download,
        update::update_restart,
        usage::usage_get_tray_summary,
    ]
}
