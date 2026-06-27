//! Process-wide single-instance behavior.

use tauri::{plugin::TauriPlugin, Runtime};

use super::lifecycle;

pub(crate) fn plugin<R: Runtime>() -> TauriPlugin<R> {
    tauri_plugin_single_instance::init(|app, _args, _cwd| {
        let _ = lifecycle::open_tray_panel(app);
    })
}
