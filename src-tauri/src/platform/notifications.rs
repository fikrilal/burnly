use tauri::{plugin::PermissionState, AppHandle, Runtime};
use tauri_plugin_notification::NotificationExt;

use crate::application::ports::notification::{
    NotificationCapability, NotificationPermission, NotificationPort,
};

pub(crate) struct NativeNotificationAdapter<R: Runtime> {
    app: AppHandle<R>,
}

impl<R: Runtime> NativeNotificationAdapter<R> {
    pub(crate) fn new(app: AppHandle<R>) -> Self {
        Self { app }
    }
}

impl<R: Runtime> NotificationPort for NativeNotificationAdapter<R> {
    fn capability(&self) -> NotificationCapability {
        NotificationCapability {
            supported: true,
            permission: self
                .app
                .notification()
                .permission_state()
                .map(permission)
                .unwrap_or(NotificationPermission::Unknown),
        }
    }

    fn request_permission(&self) -> NotificationPermission {
        self.app
            .notification()
            .request_permission()
            .map(permission)
            .unwrap_or(NotificationPermission::Unknown)
    }
}

fn permission(state: PermissionState) -> NotificationPermission {
    match state {
        PermissionState::Granted => NotificationPermission::Granted,
        PermissionState::Denied => NotificationPermission::Denied,
        PermissionState::Prompt | PermissionState::PromptWithRationale => {
            NotificationPermission::Prompt
        }
    }
}
