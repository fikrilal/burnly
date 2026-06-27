//! Desktop window lifecycle policy and Tauri window actions.

use tauri::{Manager, Runtime, WebviewUrl};
use thiserror::Error;

use crate::application::ports::window_actions::{WindowActionError, WindowActions};

pub(crate) const TRAY_PANEL_WINDOW_LABEL: &str = "tray-panel";

const TRAY_PANEL_WIDTH: f64 = 440.0;
const TRAY_PANEL_HEIGHT: f64 = 540.0;
const TRAY_PANEL_TOP_OFFSET: f64 = 48.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WindowActivationErrorKind {
    Show,
    Unminimize,
    Focus,
}

#[derive(Debug, Error)]
#[error("failed to activate the window")]
pub(crate) struct WindowActivationError {
    kind: WindowActivationErrorKind,
}

impl WindowActivationError {
    fn new(kind: WindowActivationErrorKind) -> Self {
        Self { kind }
    }
}

fn hide_tray_panel<R: Runtime, M: Manager<R>>(manager: &M) -> tauri::Result<()> {
    if let Some(window) = manager.get_webview_window(TRAY_PANEL_WINDOW_LABEL) {
        window.hide()?;
    }

    Ok(())
}

pub(crate) fn open_tray_panel<R: Runtime, M: Manager<R>>(
    manager: &M,
) -> Result<(), WindowActivationError> {
    if let Some(window) = manager.get_webview_window(TRAY_PANEL_WINDOW_LABEL) {
        // Toggle: a visible panel hides on a repeat trigger.
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
            return Ok(());
        }
        position_tray_panel(&window);
        return activate_webview_window(&window);
    }

    let window = tauri::WebviewWindowBuilder::new(
        manager,
        TRAY_PANEL_WINDOW_LABEL,
        WebviewUrl::App("index.html#/tray".into()),
    )
    .title("Burnly")
    .inner_size(TRAY_PANEL_WIDTH, TRAY_PANEL_HEIGHT)
    .resizable(false)
    .decorations(false)
    .transparent(true)
    .skip_taskbar(true)
    .always_on_top(true)
    .focused(true)
    .visible(false)
    .build()
    .map_err(|_| WindowActivationError::new(WindowActivationErrorKind::Show))?;

    position_tray_panel(&window);

    window
        .show()
        .map_err(|_| WindowActivationError::new(WindowActivationErrorKind::Show))?;
    window
        .set_focus()
        .map_err(|_| WindowActivationError::new(WindowActivationErrorKind::Focus))?;

    Ok(())
}

/// Hides the tray panel when it loses focus, so clicking elsewhere dismisses it.
pub(crate) fn handle_tray_panel_blur<R: Runtime>(window: &tauri::Window<R>) {
    if window.label() == TRAY_PANEL_WINDOW_LABEL {
        let _ = window.hide();
    }
}

/// Positions the panel near the top-right of its monitor. Tray-icon anchoring is
/// unreliable across Linux desktops, so a predictable corner is used instead.
fn position_tray_panel<R: Runtime>(window: &tauri::WebviewWindow<R>) {
    let Ok(Some(monitor)) = window.current_monitor() else {
        return;
    };

    let scale = monitor.scale_factor();
    let size = monitor.size().to_logical::<f64>(scale);
    let position = monitor.position().to_logical::<f64>(scale);

    let margin = 12.0;
    let x = position.x + (size.width - TRAY_PANEL_WIDTH - margin).max(0.0);
    let y = position.y + TRAY_PANEL_TOP_OFFSET;

    let _ = window.set_position(tauri::LogicalPosition::new(x, y));
}

fn activate_webview_window<R: Runtime>(
    window: &tauri::WebviewWindow<R>,
) -> Result<(), WindowActivationError> {
    window
        .show()
        .map_err(|_| WindowActivationError::new(WindowActivationErrorKind::Show))?;
    window
        .unminimize()
        .map_err(|_| WindowActivationError::new(WindowActivationErrorKind::Unminimize))?;
    window
        .set_focus()
        .map_err(|_| WindowActivationError::new(WindowActivationErrorKind::Focus))?;

    Ok(())
}

pub(crate) struct DesktopWindowActions<R: Runtime> {
    app: tauri::AppHandle<R>,
}

impl<R: Runtime> DesktopWindowActions<R> {
    pub(crate) fn new(app: tauri::AppHandle<R>) -> Self {
        Self { app }
    }
}

impl<R: Runtime> WindowActions for DesktopWindowActions<R> {
    fn hide_tray_panel(&self) -> Result<(), WindowActionError> {
        hide_tray_panel(&self.app).map_err(|_| WindowActionError::HideTrayPanel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_labels_are_stable_contracts() {
        assert_eq!(TRAY_PANEL_WINDOW_LABEL, "tray-panel");
    }

    #[test]
    fn open_tray_panel_creates_panel_when_missing() {
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("build mock tauri app");

        open_tray_panel(app.handle()).expect("open tray panel");

        assert!(app.get_webview_window(TRAY_PANEL_WINDOW_LABEL).is_some());
    }
}
