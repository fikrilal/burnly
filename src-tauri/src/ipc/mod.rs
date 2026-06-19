//! Tauri command, event, and transport mapping boundary.
//!
//! IPC handlers invoke application use cases and do not own product rules or
//! infrastructure behavior.

mod budgets;
mod commands;
mod contract;
mod diagnostics;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "response primitives are consumed by registered commands in Phase 2B"
    )
)]
mod response;
mod settings;
mod usage;

pub(crate) use commands::refresh_event_sink;
pub(crate) use response::CONTRACT_VERSION;

pub(crate) fn invoke_handler<R: tauri::Runtime>() -> impl Fn(tauri::ipc::Invoke<R>) -> bool {
    tauri::generate_handler![
        commands::__burnly_contract_probe,
        commands::app_get_bootstrap,
        commands::app_get_capabilities,
        diagnostics::diagnostics_get_status,
        diagnostics::diagnostics_reveal_logs,
        settings::settings_get,
        settings::settings_update,
        settings::settings_update_project_path_retention,
        budgets::budgets_list,
        budgets::budgets_get,
        budgets::budgets_create,
        budgets::budgets_update,
        budgets::budgets_enable,
        budgets::budgets_disable,
        budgets::budgets_delete,
        budgets::budgets_get_progress,
        commands::refresh_get_state,
        commands::refresh_request,
        commands::refresh_cancel,
        usage::usage_get_overview,
        usage::usage_get_calendar,
        usage::usage_get_day_detail,
        usage::usage_get_sessions,
        usage::usage_get_session_detail,
    ]
}
