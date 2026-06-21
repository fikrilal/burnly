//! Operating-system and Tauri lifecycle integrations.

pub mod database_path;
pub(crate) mod export;
pub(crate) mod lifecycle;
pub(crate) mod logs;
pub(crate) mod notifications;
pub(crate) mod single_instance;
pub mod system_clock;
pub mod system_timezone;
pub(crate) mod tray;
