//! Typed IPC event names and payloads.
//!
//! Events are lossy notifications. Payloads are always JSON objects (never unit/`null`).
//! The frontend re-queries authoritative command data after invalidation events.

use serde::Serialize;

/// Versioned event names (must stay aligned with `contract.rs` / generated TS).
pub(crate) mod names {
    pub(crate) const REFRESH_PROGRESS: &str = "burnly://v1/refresh-progress";
    pub(crate) const DATA_INVALIDATED: &str = "burnly://v1/data-invalidated";
    pub(crate) const SETTINGS_CHANGED: &str = "burnly://v1/settings-changed";
    pub(crate) const ACCOUNT_SESSION_CHANGED: &str = "burnly://v1/account-session-changed";
    #[expect(dead_code, reason = "reserved until platform-state emitters ship")]
    pub(crate) const PLATFORM_STATE_CHANGED: &str = "burnly://v1/platform-state-changed";
    #[expect(dead_code, reason = "reserved until update-progress emitters ship")]
    pub(crate) const UPDATE_PROGRESS: &str = "burnly://v1/update-progress";
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RefreshProgressEvent {
    pub(crate) status: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DataInvalidatedEvent {
    pub(crate) scope: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SettingsChangedEvent {
    pub(crate) revision: i64,
}

/// Why the account session view may have changed. Frontend always re-queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AccountSessionChangeReason {
    LoginStarted,
    LoginCompleted,
    LoginCancelled,
    LoginFailed,
    LoggedOut,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AccountSessionChangedEvent {
    pub(crate) reason: AccountSessionChangeReason,
}

/// Reserved for future platform capability / lifecycle notifications.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[expect(dead_code, reason = "wire type reserved until platform events ship")]
pub(crate) struct PlatformStateChangedEvent {
    pub(crate) kind: &'static str,
}

/// Reserved for update-progress UI (mirrors refresh-progress shape today).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[expect(dead_code, reason = "wire type reserved until update progress ships")]
pub(crate) struct UpdateProgressEvent {
    pub(crate) status: &'static str,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_session_payload_is_object_not_null() {
        let json = serde_json::to_value(AccountSessionChangedEvent {
            reason: AccountSessionChangeReason::LoginCompleted,
        })
        .expect("serialize");
        assert!(json.is_object());
        assert_eq!(json["reason"], "login_completed");
    }

    #[test]
    fn settings_changed_payload_includes_revision() {
        let json = serde_json::to_value(SettingsChangedEvent { revision: 7 }).expect("serialize");
        assert_eq!(json["revision"], 7);
    }
}
