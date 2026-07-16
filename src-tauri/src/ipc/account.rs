//! Account session IPC — secret-free cloud sign-in status, start/cancel, logout.

use serde::Serialize;
use tauri::{Emitter, State};
use tauri_plugin_opener::OpenerExt;

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
            AccountSessionStatus::WaitingForBrowser => Self {
                status: "waiting_for_browser",
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
pub(super) fn account_start_login<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    service: State<'_, AccountService>,
) -> IpcResponse<AccountSessionResponse> {
    match service.start_login() {
        Ok(started) => match app.opener().open_url(&started.login_url, None::<&str>) {
            Ok(()) => {
                let _ = app.emit("burnly://v1/account-session-changed", ());
                IpcResponse::success(started.view.into())
            }
            Err(_) => {
                service.cancel_login();
                IpcResponse::failure(IpcError::new(
                    "account.open_browser_failed",
                    "Burnly could not open the system browser to sign in.",
                    ErrorCategory::Platform,
                    true,
                ))
            }
        },
        Err(error) => IpcResponse::failure(start_login_error(error)),
    }
}

#[tauri::command]
pub(super) fn account_cancel_login<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    service: State<'_, AccountService>,
) -> IpcResponse<AccountSessionResponse> {
    let view = service.cancel_login();
    let _ = app.emit("burnly://v1/account-session-changed", ());
    IpcResponse::success(view.into())
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
        Err(other) => IpcResponse::failure(start_login_error(other)),
    }
}

fn start_login_error(error: AccountServiceError) -> IpcError {
    match error {
        AccountServiceError::LoginUnavailable => IpcError::new(
            "account.login_unavailable",
            "Sign-in is unavailable in this build or configuration.",
            ErrorCategory::Unavailable,
            true,
        ),
        AccountServiceError::AlreadySignedIn => IpcError::new(
            "account.already_signed_in",
            "You are already signed in.",
            ErrorCategory::Conflict,
            false,
        ),
        AccountServiceError::LogoutFailed => IpcError::new(
            "account.logout_failed",
            "Burnly could not sign out of this device.",
            ErrorCategory::Unavailable,
            true,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_waiting_for_browser_without_secrets() {
        let response = AccountSessionResponse::from(AccountSessionView {
            status: AccountSessionStatus::WaitingForBrowser,
            email: None,
            user_id: None,
        });
        assert_eq!(response.status, "waiting_for_browser");
        let json = serde_json::to_string(&response).expect("json");
        assert!(!json.to_lowercase().contains("verifier"));
        assert!(!json.to_lowercase().contains("token"));
    }

    #[test]
    fn maps_signed_in_view_without_token_fields() {
        let response = AccountSessionResponse::from(AccountSessionView {
            status: AccountSessionStatus::SignedIn,
            email: Some("dev@burnly.dev".into()),
            user_id: Some("user-1".into()),
        });
        assert_eq!(response.status, "signed_in");
        assert_eq!(response.email.as_deref(), Some("dev@burnly.dev"));
    }

    #[test]
    fn maps_signed_out_view() {
        let response = AccountSessionResponse::from(AccountSessionView::signed_out());
        assert_eq!(response.status, "signed_out");
    }
}
