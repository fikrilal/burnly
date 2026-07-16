//! Secret-free account session surface for IPC and settings UI.
//!
//! Owns pending desktop-login state for the web handoff. Tokens stay in
//! `CloudSession` / the token store — never on this view or IPC DTOs.

#![allow(
    dead_code,
    reason = "device id and pending accessors used across auth chunks"
)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use thiserror::Error;

use crate::application::cloud_session::{CloudSession, SessionSnapshot};
use crate::application::pkce::{
    build_desktop_login_url, generate_code_verifier, generate_state, s256_challenge,
};
use crate::application::ports::desktop_token_exchanger::{
    DesktopTokenExchangeRequest, DesktopTokenExchanger,
};

/// Default pending-login / loopback wait window.
pub(crate) const PENDING_LOGIN_TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// Web login endpoints needed to start desktop auth (from cloud config).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DesktopLoginConfig {
    pub web_origin: String,
    pub redirect_uri: String,
}

#[derive(Debug, Clone)]
pub(crate) struct PendingLogin {
    pub state: String,
    pub code_verifier: String,
    pub redirect_uri: String,
    pub started_at: Instant,
}

/// Application-owned account handle. Secrets stay inside `CloudSession`.
pub(crate) struct AccountService {
    session: Option<Arc<CloudSession>>,
    device_id: Option<String>,
    device_name: String,
    login_config: Option<DesktopLoginConfig>,
    token_exchanger: Option<Arc<dyn DesktopTokenExchanger>>,
    pending: Mutex<Option<PendingLogin>>,
    loopback_cancel: Mutex<Option<Arc<AtomicBool>>>,
    exchanging: AtomicBool,
    last_error: Mutex<Option<AccountLoginError>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AccountSessionStatus {
    SignedOut,
    WaitingForBrowser,
    Exchanging,
    SignedIn,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AccountLoginError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AccountSessionView {
    pub status: AccountSessionStatus,
    pub email: Option<String>,
    pub user_id: Option<String>,
    pub last_error: Option<AccountLoginError>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StartLoginResult {
    pub view: AccountSessionView,
    pub login_url: String,
    pub redirect_uri: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub(crate) enum AccountServiceError {
    #[error("account logout failed")]
    LogoutFailed,
    #[error("cloud account login is unavailable")]
    LoginUnavailable,
    #[error("already signed in")]
    AlreadySignedIn,
    #[error("no sign-in in progress")]
    NoPendingLogin,
    #[error("sign-in state mismatch")]
    StateMismatch,
    #[error("sign-in timed out")]
    ExpiredPending,
    #[error("missing authorization code")]
    EmptyCode,
    #[error("token exchange failed")]
    ExchangeFailed { code: Option<String>, message: String },
    #[error("failed to store session")]
    StorageFailed,
}

impl AccountService {
    pub(crate) fn unavailable(
        device_id: Option<String>,
        device_name: impl Into<String>,
        login_config: Option<DesktopLoginConfig>,
    ) -> Self {
        Self {
            session: None,
            device_id,
            device_name: device_name.into(),
            login_config,
            token_exchanger: None,
            pending: Mutex::new(None),
            loopback_cancel: Mutex::new(None),
            exchanging: AtomicBool::new(false),
            last_error: Mutex::new(None),
        }
    }

    pub(crate) fn from_session(
        session: Arc<CloudSession>,
        device_id: Option<String>,
        device_name: impl Into<String>,
        login_config: DesktopLoginConfig,
        token_exchanger: Arc<dyn DesktopTokenExchanger>,
    ) -> Self {
        Self {
            session: Some(session),
            device_id,
            device_name: device_name.into(),
            login_config: Some(login_config),
            token_exchanger: Some(token_exchanger),
            pending: Mutex::new(None),
            loopback_cancel: Mutex::new(None),
            exchanging: AtomicBool::new(false),
            last_error: Mutex::new(None),
        }
    }

    pub(crate) fn device_id(&self) -> Option<&str> {
        self.device_id.as_deref()
    }

    pub(crate) fn device_name(&self) -> &str {
        &self.device_name
    }

    pub(crate) fn redirect_uri(&self) -> Option<&str> {
        self.login_config
            .as_ref()
            .map(|config| config.redirect_uri.as_str())
    }

    pub(crate) fn session_view(&self) -> AccountSessionView {
        let last_error = self.last_error.lock().ok().and_then(|guard| guard.clone());

        if let Some(session) = &self.session {
            if let Ok(SessionSnapshot::SignedIn { account }) = session.snapshot() {
                return AccountSessionView {
                    status: AccountSessionStatus::SignedIn,
                    email: Some(account.email),
                    user_id: Some(account.user_id),
                    last_error: None,
                };
            }
        }

        if self.exchanging.load(Ordering::SeqCst) {
            return AccountSessionView {
                status: AccountSessionStatus::Exchanging,
                email: None,
                user_id: None,
                last_error: None,
            };
        }

        if self.has_pending_login() {
            return AccountSessionView {
                status: AccountSessionStatus::WaitingForBrowser,
                email: None,
                user_id: None,
                last_error: None,
            };
        }

        AccountSessionView {
            status: AccountSessionStatus::SignedOut,
            email: None,
            user_id: None,
            last_error,
        }
    }

    pub(crate) fn logout(&self) -> Result<AccountSessionView, AccountServiceError> {
        self.cancel_loopback();
        self.clear_pending();
        self.exchanging.store(false, Ordering::SeqCst);
        self.clear_last_error();
        let Some(session) = &self.session else {
            return Ok(self.session_view());
        };
        session
            .logout()
            .map_err(|_| AccountServiceError::LogoutFailed)?;
        Ok(self.session_view())
    }

    /// Starts PKCE login. A second start **replaces** any existing pending login.
    pub(crate) fn start_login(&self) -> Result<StartLoginResult, AccountServiceError> {
        if matches!(self.session_view().status, AccountSessionStatus::SignedIn) {
            return Err(AccountServiceError::AlreadySignedIn);
        }
        if self.session.is_none() || self.token_exchanger.is_none() {
            return Err(AccountServiceError::LoginUnavailable);
        }
        let config = self
            .login_config
            .as_ref()
            .ok_or(AccountServiceError::LoginUnavailable)?;

        self.clear_last_error();
        self.exchanging.store(false, Ordering::SeqCst);

        let state = generate_state();
        let code_verifier = generate_code_verifier();
        let code_challenge = s256_challenge(&code_verifier);
        let login_url = build_desktop_login_url(
            &config.web_origin,
            &config.redirect_uri,
            &state,
            &code_challenge,
        );

        let pending = PendingLogin {
            state,
            code_verifier,
            redirect_uri: config.redirect_uri.clone(),
            started_at: Instant::now(),
        };
        *self
            .pending
            .lock()
            .map_err(|_| AccountServiceError::LoginUnavailable)? = Some(pending);

        Ok(StartLoginResult {
            view: self.session_view(),
            login_url,
            redirect_uri: config.redirect_uri.clone(),
        })
    }

    pub(crate) fn cancel_login(&self) -> AccountSessionView {
        self.cancel_loopback();
        self.clear_pending();
        self.exchanging.store(false, Ordering::SeqCst);
        self.clear_last_error();
        self.session_view()
    }

    /// Ends a pending login with a user-visible error (timeout / invalid callback).
    pub(crate) fn abandon_login_with_error(
        &self,
        error: AccountServiceError,
    ) -> AccountSessionView {
        self.cancel_loopback();
        self.clear_pending();
        self.exchanging.store(false, Ordering::SeqCst);
        self.record_login_error(&error);
        self.session_view()
    }

    /// Records a user-visible login failure (never secrets).
    pub(crate) fn record_login_error(&self, error: &AccountServiceError) {
        if matches!(error, AccountServiceError::NoPendingLogin) {
            // Late callback after cancel/success — ignore quietly.
            return;
        }
        if let Some(view_error) = user_visible_login_error(error) {
            if let Ok(mut guard) = self.last_error.lock() {
                *guard = Some(view_error);
            }
        }
        self.exchanging.store(false, Ordering::SeqCst);
    }

    /// Arms a new cancel flag for the loopback listener; cancels any previous one.
    pub(crate) fn arm_loopback_cancel(&self) -> Arc<AtomicBool> {
        let flag = Arc::new(AtomicBool::new(false));
        if let Ok(mut guard) = self.loopback_cancel.lock() {
            if let Some(previous) = guard.take() {
                previous.store(true, Ordering::SeqCst);
            }
            *guard = Some(flag.clone());
        }
        flag
    }

    pub(crate) fn complete_login(
        &self,
        code: &str,
        state: &str,
    ) -> Result<AccountSessionView, AccountServiceError> {
        let pending = match self.take_pending_login() {
            Some(pending) => pending,
            None => return Err(AccountServiceError::NoPendingLogin),
        };

        if pending.started_at.elapsed() > PENDING_LOGIN_TIMEOUT {
            let error = AccountServiceError::ExpiredPending;
            self.record_login_error(&error);
            return Err(error);
        }
        if pending.state != state {
            let error = AccountServiceError::StateMismatch;
            self.record_login_error(&error);
            return Err(error);
        }
        if code.trim().is_empty() {
            let error = AccountServiceError::EmptyCode;
            self.record_login_error(&error);
            return Err(error);
        }

        let session = match self.session.as_ref() {
            Some(session) => session,
            None => {
                let error = AccountServiceError::LoginUnavailable;
                self.record_login_error(&error);
                return Err(error);
            }
        };
        let exchanger = match self.token_exchanger.as_ref() {
            Some(exchanger) => exchanger,
            None => {
                let error = AccountServiceError::LoginUnavailable;
                self.record_login_error(&error);
                return Err(error);
            }
        };

        self.clear_last_error();
        self.exchanging.store(true, Ordering::SeqCst);

        let exchanged = match exchanger.exchange(DesktopTokenExchangeRequest {
            code: code.to_owned(),
            code_verifier: pending.code_verifier,
            redirect_uri: pending.redirect_uri,
            device_id: self.device_id.clone(),
            device_name: self.device_name.clone(),
        }) {
            Ok(result) => result,
            Err(error) => {
                let error = AccountServiceError::ExchangeFailed {
                    code: error.code,
                    message: error.message,
                };
                self.record_login_error(&error);
                return Err(error);
            }
        };

        if session
            .apply_tokens(exchanged.tokens, exchanged.account)
            .is_err()
        {
            let error = AccountServiceError::StorageFailed;
            self.record_login_error(&error);
            return Err(error);
        }

        self.exchanging.store(false, Ordering::SeqCst);
        self.cancel_loopback();
        self.clear_last_error();
        Ok(self.session_view())
    }

    pub(crate) fn take_pending_login(&self) -> Option<PendingLogin> {
        self.pending.lock().ok().and_then(|mut guard| guard.take())
    }

    pub(crate) fn peek_pending_login(&self) -> Option<PendingLogin> {
        self.pending
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
    }

    fn has_pending_login(&self) -> bool {
        self.pending
            .lock()
            .map(|guard| guard.is_some())
            .unwrap_or(false)
    }

    fn clear_pending(&self) {
        if let Ok(mut guard) = self.pending.lock() {
            *guard = None;
        }
    }

    fn cancel_loopback(&self) {
        if let Ok(mut guard) = self.loopback_cancel.lock() {
            if let Some(flag) = guard.take() {
                flag.store(true, Ordering::SeqCst);
            }
        }
    }

    fn clear_last_error(&self) {
        if let Ok(mut guard) = self.last_error.lock() {
            *guard = None;
        }
    }
}

impl AccountSessionView {
    pub(crate) fn signed_out() -> Self {
        Self {
            status: AccountSessionStatus::SignedOut,
            email: None,
            user_id: None,
            last_error: None,
        }
    }
}

/// User-visible login error copy. Never includes raw server secrets.
pub(crate) fn user_visible_login_error(error: &AccountServiceError) -> Option<AccountLoginError> {
    match error {
        AccountServiceError::NoPendingLogin => None,
        AccountServiceError::StateMismatch => Some(AccountLoginError {
            code: "account.state_mismatch".into(),
            message: "Sign-in could not be verified. Please try again.".into(),
        }),
        AccountServiceError::ExpiredPending => Some(AccountLoginError {
            code: "account.login_timeout".into(),
            message: "Sign-in timed out. Please try again.".into(),
        }),
        AccountServiceError::EmptyCode => Some(AccountLoginError {
            code: "account.invalid_callback".into(),
            message: "Burnly received an invalid sign-in callback.".into(),
        }),
        AccountServiceError::LoginUnavailable => Some(AccountLoginError {
            code: "account.login_unavailable".into(),
            message: "Sign-in is unavailable in this build or configuration.".into(),
        }),
        AccountServiceError::StorageFailed => Some(AccountLoginError {
            code: "account.storage_failed".into(),
            message: "Burnly could not save the signed-in session on this device.".into(),
        }),
        AccountServiceError::ExchangeFailed { code, .. } => {
            let (code, message) = match code.as_deref() {
                Some("AUTH_DESKTOP_HANDOFF_INVALID") => (
                    "AUTH_DESKTOP_HANDOFF_INVALID",
                    "Sign-in expired or was invalid. Please try again.",
                ),
                Some("AUTH_USER_SUSPENDED") => (
                    "AUTH_USER_SUSPENDED",
                    "This account is suspended and cannot sign in.",
                ),
                Some("RATE_LIMITED") => (
                    "RATE_LIMITED",
                    "Too many sign-in attempts. Please wait and try again.",
                ),
                Some("AUTH_REFRESH_TOKEN_INVALID")
                | Some("AUTH_REFRESH_TOKEN_EXPIRED")
                | Some("AUTH_REFRESH_TOKEN_REUSED")
                | Some("AUTH_SESSION_REVOKED")
                | Some("UNAUTHORIZED") => (
                    "UNAUTHORIZED",
                    "Your session is no longer valid. Please sign in again.",
                ),
                _ => (
                    "account.exchange_failed",
                    "Sign-in failed. Please try again.",
                ),
            };
            Some(AccountLoginError {
                code: code.into(),
                message: message.into(),
            })
        }
        AccountServiceError::LogoutFailed
        | AccountServiceError::AlreadySignedIn => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::cloud_session::{AccountSummary, CloudTokens};
    use crate::application::ports::clock::Clock;
    use crate::application::ports::cloud_remote_logout::CloudRemoteLogout;
    use crate::application::ports::cloud_token_refresher::CloudTokenRefresher;
    use crate::application::ports::cloud_token_store::{
        CloudTokenStore, CloudTokenStoreError, StoredCloudSession,
    };
    use crate::application::ports::desktop_token_exchanger::{
        DesktopTokenExchangeError, DesktopTokenExchangeResult,
    };
    use std::sync::atomic::AtomicUsize;

    struct MemoryStore {
        inner: Mutex<Option<StoredCloudSession>>,
    }

    impl MemoryStore {
        fn new() -> Self {
            Self {
                inner: Mutex::new(None),
            }
        }
    }

    impl CloudTokenStore for MemoryStore {
        fn load(&self) -> Result<Option<StoredCloudSession>, CloudTokenStoreError> {
            Ok(self.inner.lock().expect("lock").clone())
        }

        fn save(&self, session: &StoredCloudSession) -> Result<(), CloudTokenStoreError> {
            *self.inner.lock().expect("lock") = Some(session.clone());
            Ok(())
        }

        fn clear(&self) -> Result<(), CloudTokenStoreError> {
            *self.inner.lock().expect("lock") = None;
            Ok(())
        }
    }

    struct FixedClock;
    impl Clock for FixedClock {
        fn now_epoch_ms(&self) -> i64 {
            1
        }
    }

    struct NoopRefresher;
    impl CloudTokenRefresher for NoopRefresher {
        fn refresh(
            &self,
            _: &str,
        ) -> Result<CloudTokens, crate::application::cloud_session::CloudSessionError> {
            Err(crate::application::cloud_session::CloudSessionError::NotSignedIn)
        }
    }

    struct NoopLogout;
    impl CloudRemoteLogout for NoopLogout {
        fn logout_remote(
            &self,
            _: &str,
        ) -> Result<(), crate::application::cloud_session::CloudSessionError> {
            Ok(())
        }
    }

    struct FakeExchanger {
        calls: AtomicUsize,
        result: Mutex<Option<Result<DesktopTokenExchangeResult, DesktopTokenExchangeError>>>,
    }

    impl DesktopTokenExchanger for FakeExchanger {
        fn exchange(
            &self,
            request: DesktopTokenExchangeRequest,
        ) -> Result<DesktopTokenExchangeResult, DesktopTokenExchangeError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            assert!(!request.code.is_empty());
            assert!(!request.code_verifier.is_empty());
            self.result
                .lock()
                .expect("lock")
                .take()
                .unwrap_or(Err(DesktopTokenExchangeError {
                    code: Some("INTERNAL".into()),
                    message: "missing scripted result".into(),
                }))
        }
    }

    fn login_config() -> DesktopLoginConfig {
        DesktopLoginConfig {
            web_origin: "http://127.0.0.1:3000".into(),
            redirect_uri: "http://127.0.0.1:39201/callback".into(),
        }
    }

    fn session(store: Arc<MemoryStore>) -> Arc<CloudSession> {
        Arc::new(CloudSession::new(
            store,
            Arc::new(NoopRefresher),
            Arc::new(NoopLogout),
            Arc::new(FixedClock),
        ))
    }

    fn service_with(
        exchanger: Arc<FakeExchanger>,
    ) -> (AccountService, Arc<MemoryStore>) {
        let store = Arc::new(MemoryStore::new());
        let service = AccountService::from_session(
            session(store.clone()),
            Some("dev_1".into()),
            "host",
            login_config(),
            exchanger,
        );
        (service, store)
    }

    #[test]
    fn start_login_sets_waiting_and_builds_url() {
        let exchanger = Arc::new(FakeExchanger {
            calls: AtomicUsize::new(0),
            result: Mutex::new(None),
        });
        let (service, _) = service_with(exchanger);
        let started = service.start_login().expect("start");
        assert_eq!(
            started.view.status,
            AccountSessionStatus::WaitingForBrowser
        );
        assert!(started.login_url.contains("client=desktop"));
        assert_eq!(
            started.redirect_uri,
            "http://127.0.0.1:39201/callback"
        );
    }

    #[test]
    fn complete_login_success_applies_session() {
        let exchanger = Arc::new(FakeExchanger {
            calls: AtomicUsize::new(0),
            result: Mutex::new(Some(Ok(DesktopTokenExchangeResult {
                tokens: CloudTokens {
                    access_token: "access".into(),
                    refresh_token: "refresh".into(),
                    access_expires_at_ms: Some(9_000),
                },
                account: AccountSummary {
                    user_id: "user-1".into(),
                    email: "dev@burnly.dev".into(),
                },
            }))),
        });
        let (service, store) = service_with(exchanger.clone());
        let started = service.start_login().expect("start");
        let pending = service.peek_pending_login().expect("pending");
        let view = service
            .complete_login("auth-code", &pending.state)
            .expect("complete");
        assert_eq!(view.status, AccountSessionStatus::SignedIn);
        assert_eq!(view.email.as_deref(), Some("dev@burnly.dev"));
        assert_eq!(exchanger.calls.load(Ordering::SeqCst), 1);
        assert!(store.load().expect("load").is_some());
        assert!(started.login_url.contains("/login?"));
    }

    #[test]
    fn state_mismatch_never_calls_exchange() {
        let exchanger = Arc::new(FakeExchanger {
            calls: AtomicUsize::new(0),
            result: Mutex::new(None),
        });
        let (service, _) = service_with(exchanger.clone());
        service.start_login().expect("start");
        let err = service
            .complete_login("auth-code", "wrong-state")
            .expect_err("mismatch");
        assert_eq!(err, AccountServiceError::StateMismatch);
        assert_eq!(exchanger.calls.load(Ordering::SeqCst), 0);
        assert!(service.peek_pending_login().is_none());
    }

    #[test]
    fn exchange_failure_maps_api_code() {
        let exchanger = Arc::new(FakeExchanger {
            calls: AtomicUsize::new(0),
            result: Mutex::new(Some(Err(DesktopTokenExchangeError {
                code: Some("AUTH_DESKTOP_HANDOFF_INVALID".into()),
                message: "invalid".into(),
            }))),
        });
        let (service, _) = service_with(exchanger);
        service.start_login().expect("start");
        let state = service.peek_pending_login().expect("pending").state;
        let err = service
            .complete_login("auth-code", &state)
            .expect_err("exchange");
        assert!(matches!(
            err,
            AccountServiceError::ExchangeFailed {
                code: Some(ref code),
                ..
            } if code == "AUTH_DESKTOP_HANDOFF_INVALID"
        ));
    }

    #[test]
    fn cancel_login_clears_pending() {
        let exchanger = Arc::new(FakeExchanger {
            calls: AtomicUsize::new(0),
            result: Mutex::new(None),
        });
        let (service, _) = service_with(exchanger);
        service.start_login().expect("start");
        let view = service.cancel_login();
        assert_eq!(view.status, AccountSessionStatus::SignedOut);
        assert!(service.peek_pending_login().is_none());
    }

    #[test]
    fn second_start_replaces_pending_login() {
        let exchanger = Arc::new(FakeExchanger {
            calls: AtomicUsize::new(0),
            result: Mutex::new(None),
        });
        let (service, _) = service_with(exchanger);
        service.start_login().expect("first");
        let first = service.peek_pending_login().expect("pending").state;
        service.start_login().expect("second");
        let second = service.peek_pending_login().expect("pending").state;
        assert_ne!(first, second);
    }
}
