#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NotificationMessage {
    pub title: String,
    pub body: String,
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NotificationDeliveryOutcome {
    Delivered,
    Failed,
}

pub(crate) trait NotificationPort: Send + Sync {
    fn capability(&self) -> NotificationCapability;

    fn request_permission(&self) -> NotificationPermission;

    fn deliver(&self, message: &NotificationMessage) -> NotificationDeliveryOutcome;
}
