mod application;
mod bootstrap;
mod domain;
mod infrastructure;
mod ipc;
mod platform;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    bootstrap::run();
}
