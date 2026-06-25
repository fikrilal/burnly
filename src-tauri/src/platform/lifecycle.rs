//! Desktop window lifecycle policy and Tauri window actions.

use serde::Serialize;
use tauri::{Emitter, Manager, Runtime, WebviewUrl};
use thiserror::Error;

use crate::domain::settings::CloseBehavior;

pub(crate) const MAIN_WINDOW_LABEL: &str = "main";
pub(crate) const TRAY_PANEL_WINDOW_LABEL: &str = "tray-panel";
const OPEN_DETAILS_EVENT: &str = "burnly://v1/open-details";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CloseDecision {
    HideWindow,
    QuitApplication,
}

pub(crate) const fn close_decision(close_behavior: CloseBehavior) -> CloseDecision {
    match close_behavior {
        CloseBehavior::Hide => CloseDecision::HideWindow,
        CloseBehavior::Quit => CloseDecision::QuitApplication,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WindowActivationErrorKind {
    MissingMainWindow,
    Show,
    Unminimize,
    Focus,
}

#[derive(Debug, Error)]
#[error("failed to activate the main window")]
pub(crate) struct WindowActivationError {
    kind: WindowActivationErrorKind,
}

impl WindowActivationError {
    fn new(kind: WindowActivationErrorKind) -> Self {
        Self { kind }
    }
}

pub(crate) fn activate_main_window<R: Runtime, M: Manager<R>>(
    manager: &M,
) -> Result<(), WindowActivationError> {
    let window = manager
        .get_webview_window(MAIN_WINDOW_LABEL)
        .ok_or_else(|| WindowActivationError::new(WindowActivationErrorKind::MissingMainWindow))?;

    activate_webview_window(&window)
}

pub(crate) fn open_details_window<R: Runtime, M: Manager<R>>(
    manager: &M,
) -> Result<(), WindowActivationError> {
    activate_main_window(manager)?;
    if let Some(window) = manager.get_webview_window(MAIN_WINDOW_LABEL) {
        let _ = window.emit(OPEN_DETAILS_EVENT, OpenDetailsEvent { view: "overview" });
    }
    Ok(())
}

pub(crate) fn open_tray_panel<R: Runtime, M: Manager<R>>(
    manager: &M,
) -> Result<(), WindowActivationError> {
    if let Some(window) = manager.get_webview_window(TRAY_PANEL_WINDOW_LABEL) {
        return activate_webview_window(&window);
    }

    let window = tauri::WebviewWindowBuilder::new(
        manager,
        TRAY_PANEL_WINDOW_LABEL,
        WebviewUrl::App("index.html#/tray".into()),
    )
    .title("Burnly")
    .inner_size(360.0, 520.0)
    .resizable(false)
    .decorations(false)
    .skip_taskbar(true)
    .always_on_top(true)
    .focused(true)
    .center()
    .build()
    .map_err(|_| WindowActivationError::new(WindowActivationErrorKind::Show))?;

    window
        .set_focus()
        .map_err(|_| WindowActivationError::new(WindowActivationErrorKind::Focus))?;

    Ok(())
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

pub(crate) fn handle_close_request<R: Runtime>(
    window: &tauri::Window<R>,
    api: &tauri::CloseRequestApi,
    close_behavior: CloseBehavior,
) {
    if window.label() != MAIN_WINDOW_LABEL {
        return;
    }

    if close_decision(close_behavior) == CloseDecision::HideWindow {
        api.prevent_close();
        let _ = window.hide();
    }
}

#[derive(Clone, Debug, Serialize)]
struct OpenDetailsEvent {
    view: &'static str,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close_decision_follows_current_policy() {
        assert_eq!(
            close_decision(CloseBehavior::Quit),
            CloseDecision::QuitApplication
        );
        assert_eq!(
            close_decision(CloseBehavior::Hide),
            CloseDecision::HideWindow
        );
    }

    #[test]
    fn window_labels_are_stable_contracts() {
        assert_eq!(MAIN_WINDOW_LABEL, "main");
        assert_eq!(TRAY_PANEL_WINDOW_LABEL, "tray-panel");
        assert_eq!(OPEN_DETAILS_EVENT, "burnly://v1/open-details");
    }
}
