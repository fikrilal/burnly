//! Account session IPC — secret-free cloud sign-in status, start/cancel, logout.

use std::thread;

use serde::Serialize;
use tauri::{Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

use crate::application::account::{
    AccountService, AccountServiceError, AccountSessionStatus, AccountSessionView,
    PENDING_LOGIN_TIMEOUT,
};
use crate::application::auth_loopback::{LoopbackError, LoopbackServer};

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
    let started = match service.start_login() {
        Ok(started) => started,
        Err(error) => return IpcResponse::failure(account_error(error)),
    };

    let cancel = service.arm_loopback_cancel();
    let server = match LoopbackServer::bind(&started.redirect_uri, cancel) {
        Ok(server) => server,
        Err(error) => {
            service.cancel_login();
            return IpcResponse::failure(loopback_error(error));
        }
    };

    let app_for_listener = app.clone();
    thread::spawn(move || {
        let outcome = server.accept_once(PENDING_LOGIN_TIMEOUT);
        let Some(service) = app_for_listener.try_state::<AccountService>() else {
            return;
        };
        match outcome {
            Ok((code, state)) => {
                let _ = service.complete_login(&code, &state);
            }
            Err(LoopbackError::Cancelled) => {}
            Err(_) => {
                let _ = service.cancel_login();
            }
        }
        let _ = app_for_listener.emit("burnly://v1/account-session-changed", ());
    });

    match app.opener().open_url(&started.login_url, None::<&str>) {
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
        Err(error) => IpcResponse::failure(account_error(error)),
    }
}

fn loopback_error(error: LoopbackError) -> IpcError {
    match error {
        LoopbackError::InvalidRedirectUri => IpcError::new(
            "account.invalid_redirect_uri",
            "Sign-in callback is not configured for loopback on this build.",
            ErrorCategory::Validation,
            false,
        ),
        LoopbackError::BindFailed => IpcError::new(
            "account.callback_bind_failed",
            "Burnly could not listen for the sign-in callback. Is the port already in use?",
            ErrorCategory::Platform,
            true,
        ),
        LoopbackError::Timeout => IpcError::new(
            "account.login_timeout",
            "Sign-in timed out. Please try again.",
            ErrorCategory::Unavailable,
            true,
        ),
        LoopbackError::Cancelled => IpcError::new(
            "account.login_cancelled",
            "Sign-in was cancelled.",
            ErrorCategory::Validation,
            false,
        ),
        LoopbackError::InvalidRequest => IpcError::new(
            "account.invalid_callback",
            "Burnly received an invalid sign-in callback.",
            ErrorCategory::Validation,
            true,
        ),
    }
}

fn account_error(error: AccountServiceError) -> IpcError {
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
        AccountServiceError::NoPendingLogin => IpcError::new(
            "account.no_sign_in_in_progress",
            "No sign-in is in progress.",
            ErrorCategory::Validation,
            false,
        ),
        AccountServiceError::StateMismatch => IpcError::new(
            "account.state_mismatch",
            "Sign-in could not be verified. Please try again.",
            ErrorCategory::Validation,
            true,
        ),
        AccountServiceError::ExpiredPending => IpcError::new(
            "account.login_timeout",
            "Sign-in timed out. Please try again.",
            ErrorCategory::Unavailable,
            true,
        ),
        AccountServiceError::EmptyCode => IpcError::new(
            "account.invalid_callback",
            "Burnly received an invalid sign-in callback.",
            ErrorCategory::Validation,
            true,
        ),
        AccountServiceError::ExchangeFailed { code, message: _ } => {
            let (ipc_code, user_message, retryable) = match code.as_deref() {
                Some("AUTH_DESKTOP_HANDOFF_INVALID") => (
                    "AUTH_DESKTOP_HANDOFF_INVALID",
                    "Sign-in expired or was invalid. Please try again.",
                    true,
                ),
                Some("AUTH_USER_SUSPENDED") => (
                    "AUTH_USER_SUSPENDED",
                    "This account is suspended and cannot sign in.",
                    false,
                ),
                Some("RATE_LIMITED") => (
                    "RATE_LIMITED",
                    "Too many sign-in attempts. Please wait and try again.",
                    true,
                ),
                _ => (
                    "account.exchange_failed",
                    "Sign-in failed. Please try again.",
                    true,
                ),
            };
            IpcError::new(ipc_code, user_message, ErrorCategory::Unavailable, retryable)
        }
        AccountServiceError::StorageFailed => IpcError::new(
            "account.storage_failed",
            "Burnly could not save the signed-in session on this device.",
            ErrorCategory::Persistence,
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
    fn maps_exchange_failed_handoff_code_to_safe_response() {
        let error = account_error(AccountServiceError::ExchangeFailed {
            code: Some("AUTH_DESKTOP_HANDOFF_INVALID".into()),
            message: "secret detail".into(),
        });
        let json = serde_json::to_string(&error).expect("json");
        assert!(json.contains("AUTH_DESKTOP_HANDOFF_INVALID"));
        assert!(!json.contains("secret detail"));
    }

    #[test]
    fn maps_signed_in_view_without_token_fields() {
        let response = AccountSessionResponse::from(AccountSessionView {
            status: AccountSessionStatus::SignedIn,
            email: Some("dev@burnly.dev".into()),
            user_id: Some("user-1".into()),
        });
        assert_eq!(response.status, "signed_in");
    }

    #[test]
    fn pending_timeout_is_ten_minutes() {
        assert_eq!(PENDING_LOGIN_TIMEOUT.as_secs(), 600);
    }
}
