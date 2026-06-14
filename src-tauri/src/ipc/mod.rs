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

pub(crate) fn invoke_handler<R: tauri::Runtime>() -> impl Fn(tauri::ipc::Invoke<R>) -> bool {
    tauri::generate_handler![commands::__burnly_contract_probe]
}
