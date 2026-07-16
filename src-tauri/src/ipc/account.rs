//! Account session IPC — secret-free cloud sign-in status, start/cancel, logout.

use std::thread;

use serde::Serialize;
use tauri::{Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

use crate::application::account::{
    user_visible_login_error, AccountService, AccountServiceError, AccountSessionStatus,
    AccountSessionView, PENDING_LOGIN_TIMEOUT,
};
use crate::application::auth_loopback::{LoopbackError, LoopbackServer};

use super::events::{names as event_names, AccountSessionChangeReason, AccountSessionChangedEvent};
use super::response::{ErrorCategory, IpcError, IpcResponse};

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct AccountSessionResponse {
    status: &'static str,
    email: Option<String>,
    user_id: Option<String>,
    last_error_code: Option<String>,
    last_error_message: Option<String>,
}

impl From<AccountSessionView> for AccountSessionResponse {
    fn from(value: AccountSessionView) -> Self {
        let (last_error_code, last_error_message) = match value.last_error {
            Some(error) => (Some(error.code), Some(error.message)),
            None => (None, None),
        };
        let status = match value.status {
            AccountSessionStatus::SignedOut => "signed_out",
            AccountSessionStatus::WaitingForBrowser => "waiting_for_browser",
            AccountSessionStatus::Exchanging => "exchanging",
            AccountSessionStatus::SignedIn => "signed_in",
        };
        Self {
            status,
            email: value.email,
            user_id: value.user_id,
            last_error_code,
            last_error_message,
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
        let reason = match outcome {
            Ok((code, state)) => match service.complete_login(&code, &state) {
                Ok(_) => Some(AccountSessionChangeReason::LoginCompleted),
                Err(AccountServiceError::NoPendingLogin) => {
                    // Late callback after cancel or success — no UI notification.
                    None
                }
                Err(_) => {
                    // complete_login already recorded a safe last_error when needed.
                    Some(AccountSessionChangeReason::LoginFailed)
                }
            },
            Err(LoopbackError::Cancelled) => None,
            Err(LoopbackError::Timeout) => {
                let _ = service.abandon_login_with_error(AccountServiceError::ExpiredPending);
                Some(AccountSessionChangeReason::LoginFailed)
            }
            Err(_) => {
                let _ = service.abandon_login_with_error(AccountServiceError::EmptyCode);
                Some(AccountSessionChangeReason::LoginFailed)
            }
        };
        if let Some(reason) = reason {
            emit_account_session_changed(&app_for_listener, reason);
        }
    });

    match app.opener().open_url(&started.login_url, None::<&str>) {
        Ok(()) => {
            emit_account_session_changed(&app, AccountSessionChangeReason::LoginStarted);
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
    emit_account_session_changed(&app, AccountSessionChangeReason::LoginCancelled);
    IpcResponse::success(view.into())
}

#[tauri::command]
pub(super) fn account_logout<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    service: State<'_, AccountService>,
) -> IpcResponse<AccountSessionResponse> {
    match service.logout() {
        Ok(view) => {
            emit_account_session_changed(&app, AccountSessionChangeReason::LoggedOut);
            IpcResponse::success(view.into())
        }
        Err(error) => IpcResponse::failure(account_error(error)),
    }
}

fn emit_account_session_changed<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    reason: AccountSessionChangeReason,
) {
    let _ = app.emit(
        event_names::ACCOUNT_SESSION_CHANGED,
        AccountSessionChangedEvent { reason },
    );
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
    if let Some(visible) = user_visible_login_error(&error) {
        let code: &'static str = match visible.code.as_str() {
            "AUTH_DESKTOP_HANDOFF_INVALID" => "AUTH_DESKTOP_HANDOFF_INVALID",
            "AUTH_USER_SUSPENDED" => "AUTH_USER_SUSPENDED",
            "RATE_LIMITED" => "RATE_LIMITED",
            "UNAUTHORIZED" => "UNAUTHORIZED",
            "account.state_mismatch" => "account.state_mismatch",
            "account.login_timeout" => "account.login_timeout",
            "account.invalid_callback" => "account.invalid_callback",
            "account.login_unavailable" => "account.login_unavailable",
            "account.storage_failed" => "account.storage_failed",
            "account.exchange_failed" => "account.exchange_failed",
            _ => "account.exchange_failed",
        };
        let message: &'static str = match code {
            "AUTH_DESKTOP_HANDOFF_INVALID" => "Sign-in expired or was invalid. Please try again.",
            "AUTH_USER_SUSPENDED" => "This account is suspended and cannot sign in.",
            "RATE_LIMITED" => "Too many sign-in attempts. Please wait and try again.",
            "UNAUTHORIZED" => "Your session is no longer valid. Please sign in again.",
            "account.state_mismatch" => "Sign-in could not be verified. Please try again.",
            "account.login_timeout" => "Sign-in timed out. Please try again.",
            "account.invalid_callback" => "Burnly received an invalid sign-in callback.",
            "account.login_unavailable" => "Sign-in is unavailable in this build or configuration.",
            "account.storage_failed" => {
                "Burnly could not save the signed-in session on this device."
            }
            _ => "Sign-in failed. Please try again.",
        };
        return IpcError::new(
            code,
            message,
            ErrorCategory::Unavailable,
            !matches!(code, "AUTH_USER_SUSPENDED"),
        );
    }

    match error {
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
        _ => IpcError::new(
            "account.exchange_failed",
            "Sign-in failed. Please try again.",
            ErrorCategory::Unavailable,
            true,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_exchanging_and_error_fields() {
        let response = AccountSessionResponse::from(AccountSessionView {
            status: AccountSessionStatus::Exchanging,
            email: None,
            user_id: None,
            last_error: None,
        });
        assert_eq!(response.status, "exchanging");

        let failed = AccountSessionResponse::from(AccountSessionView {
            status: AccountSessionStatus::SignedOut,
            email: None,
            user_id: None,
            last_error: Some(crate::application::account::AccountLoginError {
                code: "AUTH_DESKTOP_HANDOFF_INVALID".into(),
                message: "Sign-in expired or was invalid. Please try again.".into(),
            }),
        });
        assert_eq!(
            failed.last_error_code.as_deref(),
            Some("AUTH_DESKTOP_HANDOFF_INVALID")
        );
        assert!(!failed
            .last_error_message
            .as_deref()
            .unwrap_or("")
            .to_lowercase()
            .contains("token"));
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
    fn pending_timeout_is_ten_minutes() {
        assert_eq!(PENDING_LOGIN_TIMEOUT.as_secs(), 600);
    }
}
