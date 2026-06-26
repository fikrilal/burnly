#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NotificationCapability {
    pub supported: bool,
    pub permission: NotificationPermission,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NotificationPermission {
    Granted,
    Denied,
    Prompt,
    Unknown,
}

pub(crate) trait NotificationPort: Send + Sync {
    fn capability(&self) -> NotificationCapability;

    fn request_permission(&self) -> NotificationPermission;
}
