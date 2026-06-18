//! Desktop window lifecycle policy and Tauri window actions.

use tauri::{Manager, Runtime};
use thiserror::Error;

pub(crate) const MAIN_WINDOW_LABEL: &str = "main";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CloseBehavior {
    Hide,
    Quit,
}

impl CloseBehavior {
    pub(crate) fn from_setting(value: &str) -> Self {
        match value {
            "hide" => Self::Hide,
            "quit" => Self::Quit,
            _ => Self::Quit,
        }
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close_behavior_parses_persisted_settings_defensively() {
        assert_eq!(CloseBehavior::from_setting("hide"), CloseBehavior::Hide);
        assert_eq!(CloseBehavior::from_setting("quit"), CloseBehavior::Quit);
        assert_eq!(
            CloseBehavior::from_setting("unexpected"),
            CloseBehavior::Quit
        );
    }

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
}
