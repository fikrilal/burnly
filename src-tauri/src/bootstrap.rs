//! Application composition root.
//!
//! This module selects concrete infrastructure and platform integrations. Other
//! modules receive constructed dependencies instead of constructing their own.

pub(crate) fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
