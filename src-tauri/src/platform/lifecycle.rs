//! Desktop window lifecycle policy and Tauri window actions.

use tauri::{Manager, Runtime, WebviewUrl};
use thiserror::Error;

use crate::application::ports::window_actions::{WindowActionError, WindowActions};

pub(crate) const TRAY_PANEL_WINDOW_LABEL: &str = "tray-panel";

const TRAY_PANEL_WIDTH: f64 = 440.0;
const TRAY_PANEL_HEIGHT: f64 = 540.0;
const TRAY_PANEL_TOP_OFFSET: f64 = 48.0;
const TRAY_PANEL_MARGIN: f64 = 12.0;
const TRAY_PANEL_ANCHOR_GAP: f64 = 8.0;

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

    let window = create_tray_panel_window(manager)?;

    position_tray_panel(&window);

    window
        .show()
        .map_err(|_| WindowActivationError::new(WindowActivationErrorKind::Show))?;
    window
        .set_focus()
        .map_err(|_| WindowActivationError::new(WindowActivationErrorKind::Focus))?;

    Ok(())
}

pub(crate) fn open_tray_panel_at_rect<R: Runtime, M: Manager<R>>(
    manager: &M,
    anchor: tauri::Rect,
) -> Result<(), WindowActivationError> {
    if let Some(window) = manager.get_webview_window(TRAY_PANEL_WINDOW_LABEL) {
        // Toggle: a visible panel hides on a repeat trigger.
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
            return Ok(());
        }
        position_tray_panel_at_rect(&window, anchor);
        return activate_webview_window(&window);
    }

    let window = create_tray_panel_window(manager)?;

    position_tray_panel_at_rect(&window, anchor);

    window
        .show()
        .map_err(|_| WindowActivationError::new(WindowActivationErrorKind::Show))?;
    window
        .set_focus()
        .map_err(|_| WindowActivationError::new(WindowActivationErrorKind::Focus))?;

    Ok(())
}

pub(crate) fn prepare_tray_panel<R: Runtime, M: Manager<R>>(
    manager: &M,
) -> Result<(), WindowActivationError> {
    if manager
        .get_webview_window(TRAY_PANEL_WINDOW_LABEL)
        .is_some()
    {
        return Ok(());
    }

    create_tray_panel_window(manager)?;

    Ok(())
}

fn create_tray_panel_window<R: Runtime, M: Manager<R>>(
    manager: &M,
) -> Result<tauri::WebviewWindow<R>, WindowActivationError> {
    tauri::WebviewWindowBuilder::new(
        manager,
        TRAY_PANEL_WINDOW_LABEL,
        WebviewUrl::App("index.html#/tray".into()),
    )
    .title("Burnly")
    .inner_size(TRAY_PANEL_WIDTH, TRAY_PANEL_HEIGHT)
    .resizable(false)
    .decorations(false)
    .transparent(true)
    .shadow(true)
    .skip_taskbar(true)
    .always_on_top(true)
    .focused(true)
    .visible(false)
    .build()
    .map_err(|_| WindowActivationError::new(WindowActivationErrorKind::Show))
}

/// Hides the tray panel when it loses focus, so clicking elsewhere dismisses it.
pub(crate) fn handle_tray_panel_blur<R: Runtime>(window: &tauri::Window<R>) {
    if window.label() == TRAY_PANEL_WINDOW_LABEL {
        if cursor_is_inside_window(window) {
            return;
        }
        let _ = window.hide();
    }
}

fn cursor_is_inside_window<R: Runtime>(window: &tauri::Window<R>) -> bool {
    let Ok(cursor) = window.cursor_position() else {
        return false;
    };
    let Ok(position) = window.outer_position() else {
        return false;
    };
    let Ok(size) = window.outer_size() else {
        return false;
    };

    let left = f64::from(position.x);
    let top = f64::from(position.y);
    let right = left + f64::from(size.width);
    let bottom = top + f64::from(size.height);

    cursor.x >= left && cursor.x <= right && cursor.y >= top && cursor.y <= bottom
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

    let x = position.x + (size.width - TRAY_PANEL_WIDTH - TRAY_PANEL_MARGIN).max(0.0);
    let y = position.y + TRAY_PANEL_TOP_OFFSET;

    let _ = window.set_position(tauri::LogicalPosition::new(x, y));
}

fn position_tray_panel_at_rect<R: Runtime>(window: &tauri::WebviewWindow<R>, anchor: tauri::Rect) {
    let lookup_position = anchor.position.to_physical::<f64>(1.0);
    let lookup_size = anchor.size.to_physical::<f64>(1.0);
    let center_x = lookup_position.x + (lookup_size.width / 2.0);
    let center_y = lookup_position.y + (lookup_size.height / 2.0);

    let Ok(Some(monitor)) = window.monitor_from_point(center_x, center_y) else {
        position_tray_panel(window);
        return;
    };

    let scale = monitor.scale_factor();
    let anchor_position = anchor.position.to_physical::<f64>(scale);
    let anchor_size = anchor.size.to_physical::<f64>(scale);
    let monitor_position = monitor.position();
    let monitor_size = monitor.size();
    let bounds = PanelBounds {
        x: f64::from(monitor_position.x),
        y: f64::from(monitor_position.y),
        width: f64::from(monitor_size.width),
        height: f64::from(monitor_size.height),
    };
    let anchor = PanelBounds {
        x: anchor_position.x,
        y: anchor_position.y,
        width: anchor_size.width,
        height: anchor_size.height,
    };
    let panel = PanelSize {
        width: TRAY_PANEL_WIDTH * scale,
        height: TRAY_PANEL_HEIGHT * scale,
    };

    let position = anchored_panel_position(bounds, anchor, panel, TRAY_PANEL_MARGIN * scale);

    let _ = window.set_position(tauri::PhysicalPosition::new(
        position.x.round() as i32,
        position.y.round() as i32,
    ));
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PanelBounds {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PanelSize {
    width: f64,
    height: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PanelPosition {
    x: f64,
    y: f64,
}

fn anchored_panel_position(
    monitor: PanelBounds,
    anchor: PanelBounds,
    panel: PanelSize,
    margin: f64,
) -> PanelPosition {
    let monitor_mid_x = monitor.x + (monitor.width / 2.0);
    let monitor_mid_y = monitor.y + (monitor.height / 2.0);
    let anchor_mid_x = anchor.x + (anchor.width / 2.0);
    let anchor_mid_y = anchor.y + (anchor.height / 2.0);
    let gap = TRAY_PANEL_ANCHOR_GAP * (margin / TRAY_PANEL_MARGIN);

    let preferred_x = if anchor_mid_x >= monitor_mid_x {
        anchor.x + anchor.width - panel.width
    } else {
        anchor.x
    };
    let preferred_y = if anchor_mid_y >= monitor_mid_y {
        anchor.y - panel.height - gap
    } else {
        anchor.y + anchor.height + gap
    };

    PanelPosition {
        x: preferred_x.clamp(
            monitor.x + margin,
            monitor.x + monitor.width - panel.width - margin,
        ),
        y: preferred_y.clamp(
            monitor.y + margin,
            monitor.y + monitor.height - panel.height - margin,
        ),
    }
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

    #[test]
    fn prepare_tray_panel_creates_panel_when_missing() {
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("build mock tauri app");

        prepare_tray_panel(app.handle()).expect("prepare tray panel");

        assert!(app.get_webview_window(TRAY_PANEL_WINDOW_LABEL).is_some());
    }

    #[test]
    fn prepare_tray_panel_is_idempotent() {
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("build mock tauri app");

        prepare_tray_panel(app.handle()).expect("first prepare");
        prepare_tray_panel(app.handle()).expect("second prepare");

        assert!(app.get_webview_window(TRAY_PANEL_WINDOW_LABEL).is_some());
    }

    #[test]
    fn anchored_position_places_bottom_right_tray_panel_above_anchor() {
        let position = anchored_panel_position(
            PanelBounds {
                x: 0.0,
                y: 0.0,
                width: 1920.0,
                height: 1080.0,
            },
            PanelBounds {
                x: 1840.0,
                y: 1040.0,
                width: 40.0,
                height: 40.0,
            },
            PanelSize {
                width: 440.0,
                height: 540.0,
            },
            TRAY_PANEL_MARGIN,
        );

        assert_eq!(
            position,
            PanelPosition {
                x: 1440.0,
                y: 492.0,
            }
        );
    }

    #[test]
    fn anchored_position_places_top_left_tray_panel_below_anchor() {
        let position = anchored_panel_position(
            PanelBounds {
                x: 0.0,
                y: 0.0,
                width: 1920.0,
                height: 1080.0,
            },
            PanelBounds {
                x: 40.0,
                y: 24.0,
                width: 40.0,
                height: 40.0,
            },
            PanelSize {
                width: 440.0,
                height: 540.0,
            },
            TRAY_PANEL_MARGIN,
        );

        assert_eq!(position, PanelPosition { x: 40.0, y: 72.0 });
    }
}
