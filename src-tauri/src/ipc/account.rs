//! Account session IPC — secret-free cloud sign-in status and logout.

use serde::Serialize;
use tauri::{Emitter, State};

use crate::application::account::{
    AccountService, AccountServiceError, AccountSessionStatus, AccountSessionView,
};

use super::response::{ErrorCategory, IpcError, IpcResponse};

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct AccountSessionResponse {
    status: &'static str,
    email: Option<String>,
    user_id: Option<String>,
}

impl From<AccountSessionView> for AccountSessionResponse {
    fn from(value: AccountSessionView) -> Self {
        match value.status {
            AccountSessionStatus::SignedOut => Self {
                status: "signed_out",
                email: None,
                user_id: None,
            },
            AccountSessionStatus::SignedIn => Self {
                status: "signed_in",
                email: value.email,
                user_id: value.user_id,
            },
        }
    }
}

#[tauri::command]
pub(super) fn account_get_session(
    service: State<'_, AccountService>,
) -> IpcResponse<AccountSessionResponse> {
    IpcResponse::success(service.session_view().into())
}

#[tauri::command]
pub(super) fn account_logout<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    service: State<'_, AccountService>,
) -> IpcResponse<AccountSessionResponse> {
    match service.logout() {
        Ok(view) => {
            let _ = app.emit("burnly://v1/account-session-changed", ());
            IpcResponse::success(view.into())
        }
        Err(AccountServiceError::LogoutFailed) => IpcResponse::failure(IpcError::new(
            "account.logout_failed",
            "Burnly could not sign out of this device.",
            ErrorCategory::Unavailable,
            true,
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_signed_in_view_without_token_fields() {
        let response = AccountSessionResponse::from(AccountSessionView {
            status: AccountSessionStatus::SignedIn,
            email: Some("dev@burnly.dev".into()),
            user_id: Some("user-1".into()),
        });
        assert_eq!(response.status, "signed_in");
        assert_eq!(response.email.as_deref(), Some("dev@burnly.dev"));
        assert_eq!(response.user_id.as_deref(), Some("user-1"));
        let json = serde_json::to_string(&response).expect("json");
        assert!(!json.contains("access"));
        assert!(!json.contains("refresh"));
        assert!(!json.to_lowercase().contains("token"));
    }

    #[test]
    fn maps_signed_out_view() {
        let response = AccountSessionResponse::from(AccountSessionView::signed_out());
        assert_eq!(response.status, "signed_out");
        assert!(response.email.is_none());
        assert!(response.user_id.is_none());
    }
}
