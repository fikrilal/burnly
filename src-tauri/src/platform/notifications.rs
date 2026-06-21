use tauri::{plugin::PermissionState, AppHandle, Runtime};
use tauri_plugin_notification::NotificationExt;

use crate::application::ports::notification::{
    NotificationCapability, NotificationDeliveryOutcome, NotificationMessage,
    NotificationPermission, NotificationPort,
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

    fn deliver(&self, message: &NotificationMessage) -> NotificationDeliveryOutcome {
        match self
            .app
            .notification()
            .builder()
            .title(&message.title)
            .body(&message.body)
            .show()
        {
            Ok(()) => NotificationDeliveryOutcome::Delivered,
            Err(_) => NotificationDeliveryOutcome::Failed,
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "shows a real desktop notification"]
    fn smoke_sends_a_native_notification() {
        let app = tauri::test::mock_builder()
            .plugin(tauri_plugin_notification::init())
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("build notification smoke app");
        let adapter = NativeNotificationAdapter::new(app.handle().clone());

        assert_eq!(
            adapter.capability().permission,
            NotificationPermission::Granted
        );
        assert_eq!(
            adapter.deliver(&NotificationMessage {
                title: "Burnly notification test".to_owned(),
                body: "Native budget notifications are available.".to_owned(),
            }),
            NotificationDeliveryOutcome::Delivered
        );
    }
}
