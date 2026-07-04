use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(any(target_os = "windows", target_os = "macos"))]
use tauri::tray::{MouseButton, MouseButtonState, TrayIconEvent};
use tauri::{Manager, RunEvent, Runtime};

use crate::application::reconciliation::RefreshTrigger;
use crate::application::refresh::RefreshCoordinator;
use crate::platform::tray;

use super::tray_runtime;

#[derive(Default)]
pub(super) struct ExitGuard {
    explicit_exit_requested: AtomicBool,
}

impl ExitGuard {
    fn request_explicit_exit(&self) {
        self.explicit_exit_requested.store(true, Ordering::SeqCst);
    }

    fn allows_exit(&self) -> bool {
        self.explicit_exit_requested.load(Ordering::SeqCst)
    }
}

pub(super) fn handle_run_event<R: Runtime>(app: &tauri::AppHandle<R>, event: RunEvent) {
    match event {
        RunEvent::Resumed => {
            if let Some(coordinator) = app.try_state::<RefreshCoordinator>() {
                coordinator.request_refresh(RefreshTrigger::Resume);
            }
        }
        RunEvent::MenuEvent(event) => {
            handle_menu_event(app, &event);
        }
        #[cfg(any(target_os = "windows", target_os = "macos"))]
        RunEvent::TrayIconEvent(event) => {
            handle_tray_icon_event(app, event);
        }
        RunEvent::ExitRequested { api, .. } => {
            let explicit_exit_requested = app
                .try_state::<ExitGuard>()
                .is_some_and(|guard| guard.allows_exit());
            if !explicit_exit_requested {
                api.prevent_exit();
            }
        }
        #[cfg(target_os = "macos")]
        RunEvent::Reopen { .. } => {
            // Burnly has no main window; re-opening from the Dock reveals the
            // tray panel, matching the menu-bar interaction model.
            tray_runtime::open_tray_panel(app, None);
        }
        _ => {}
    }
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn handle_tray_icon_event<R: Runtime>(app: &tauri::AppHandle<R>, event: TrayIconEvent) {
    if let TrayIconEvent::Click {
        button: MouseButton::Left,
        button_state: MouseButtonState::Up,
        rect,
        ..
    } = event
    {
        tray_runtime::open_tray_panel(app, Some(rect));
    }
}

fn handle_menu_event<R: Runtime>(app: &tauri::AppHandle<R>, event: &tauri::menu::MenuEvent) {
    match tray::TrayAction::from_menu_event(event) {
        Some(tray::TrayAction::OpenPanel) => {
            tray_runtime::open_tray_panel(app, None);
        }

        Some(tray::TrayAction::Refresh) => {
            if let Some(coordinator) = app.try_state::<RefreshCoordinator>() {
                coordinator.request_refresh(RefreshTrigger::Manual);
            }
        }
        Some(tray::TrayAction::Quit) => {
            if let Some(exit_guard) = app.try_state::<ExitGuard>() {
                exit_guard.request_explicit_exit();
            }
            app.exit(0);
        }
        None => {}
    }
}
