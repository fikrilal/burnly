//! Operating-system and Tauri lifecycle integrations.

pub mod database_path;
pub(crate) mod lifecycle;
#[cfg(not(debug_assertions))]
pub(crate) mod single_instance;
pub mod system_clock;
pub mod system_timezone;
pub(crate) mod tray;
pub(crate) mod updater;
