//! Secret-free account session surface for IPC and settings UI.
//!
//! Owns pending desktop-login state for the web handoff. Tokens stay in
//! `CloudSession` / the token store — never on this view or IPC DTOs.

#![allow(
    dead_code,
    reason = "device id and pending accessors used by later auth chunks"
)]

use std::sync::{Arc, Mutex};
use std::time::Instant;

use thiserror::Error;

use crate::application::cloud_session::{CloudSession, SessionSnapshot};
use crate::application::pkce::{
    build_desktop_login_url, generate_code_verifier, generate_state, s256_challenge,
};

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
    pending: Mutex<Option<PendingLogin>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AccountSessionStatus {
    SignedOut,
    WaitingForBrowser,
    SignedIn,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AccountSessionView {
    pub status: AccountSessionStatus,
    pub email: Option<String>,
    pub user_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StartLoginResult {
    pub view: AccountSessionView,
    pub login_url: String,
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AccountServiceError {
    #[error("account logout failed")]
    LogoutFailed,
    #[error("cloud account login is unavailable")]
    LoginUnavailable,
    #[error("already signed in")]
    AlreadySignedIn,
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
            pending: Mutex::new(None),
        }
    }

    pub(crate) fn from_session(
        session: Arc<CloudSession>,
        device_id: Option<String>,
        device_name: impl Into<String>,
        login_config: DesktopLoginConfig,
    ) -> Self {
        Self {
            session: Some(session),
            device_id,
            device_name: device_name.into(),
            login_config: Some(login_config),
            pending: Mutex::new(None),
        }
    }

    pub(crate) fn device_id(&self) -> Option<&str> {
        self.device_id.as_deref()
    }

    pub(crate) fn device_name(&self) -> &str {
        &self.device_name
    }

    pub(crate) fn session_view(&self) -> AccountSessionView {
        if let Some(session) = &self.session {
            if let Ok(SessionSnapshot::SignedIn { account }) = session.snapshot() {
                return AccountSessionView {
                    status: AccountSessionStatus::SignedIn,
                    email: Some(account.email),
                    user_id: Some(account.user_id),
                };
            }
        }

        if self.has_pending_login() {
            return AccountSessionView {
                status: AccountSessionStatus::WaitingForBrowser,
                email: None,
                user_id: None,
            };
        }

        AccountSessionView::signed_out()
    }

    pub(crate) fn logout(&self) -> Result<AccountSessionView, AccountServiceError> {
        self.clear_pending();
        let Some(session) = &self.session else {
            return Ok(AccountSessionView::signed_out());
        };
        session
            .logout()
            .map_err(|_| AccountServiceError::LogoutFailed)?;
        Ok(AccountSessionView::signed_out())
    }

    /// Starts PKCE login. A second start **replaces** any existing pending login.
    pub(crate) fn start_login(&self) -> Result<StartLoginResult, AccountServiceError> {
        if matches!(
            self.session_view().status,
            AccountSessionStatus::SignedIn
        ) {
            return Err(AccountServiceError::AlreadySignedIn);
        }
        if self.session.is_none() {
            return Err(AccountServiceError::LoginUnavailable);
        }
        let config = self
            .login_config
            .as_ref()
            .ok_or(AccountServiceError::LoginUnavailable)?;

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
            view: AccountSessionView {
                status: AccountSessionStatus::WaitingForBrowser,
                email: None,
                user_id: None,
            },
            login_url,
        })
    }

    pub(crate) fn cancel_login(&self) -> AccountSessionView {
        self.clear_pending();
        self.session_view()
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
}

impl AccountSessionView {
    pub(crate) fn signed_out() -> Self {
        Self {
            status: AccountSessionStatus::SignedOut,
            email: None,
            user_id: None,
        }
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

    fn service_with_session() -> AccountService {
        AccountService::from_session(
            session(Arc::new(MemoryStore::new())),
            Some("dev_1".into()),
            "host",
            login_config(),
        )
    }

    #[test]
    fn unavailable_is_signed_out_and_cannot_start_login() {
        let service = AccountService::unavailable(None, "host", Some(login_config()));
        assert_eq!(service.session_view(), AccountSessionView::signed_out());
        assert_eq!(
            service.start_login().expect_err("unavailable"),
            AccountServiceError::LoginUnavailable
        );
    }

    #[test]
    fn start_login_sets_waiting_and_builds_url() {
        let service = service_with_session();
        let started = service.start_login().expect("start");
        assert_eq!(
            started.view.status,
            AccountSessionStatus::WaitingForBrowser
        );
        assert!(started.login_url.contains("/login?"));
        assert!(started.login_url.contains("client=desktop"));
        assert!(started.login_url.contains("code_challenge_method=S256"));
        assert_eq!(
            service.session_view().status,
            AccountSessionStatus::WaitingForBrowser
        );
        assert!(service.peek_pending_login().is_some());
    }

    #[test]
    fn second_start_replaces_pending_login() {
        let service = service_with_session();
        let first = service.start_login().expect("first");
        let first_state = service.peek_pending_login().expect("pending").state;
        let second = service.start_login().expect("second");
        let second_state = service.peek_pending_login().expect("pending").state;
        assert_ne!(first_state, second_state);
        assert_ne!(first.login_url, second.login_url);
    }

    #[test]
    fn cancel_login_clears_pending() {
        let service = service_with_session();
        service.start_login().expect("start");
        let view = service.cancel_login();
        assert_eq!(view.status, AccountSessionStatus::SignedOut);
        assert!(service.peek_pending_login().is_none());
    }

    #[test]
    fn signed_in_blocks_start_login() {
        let store = Arc::new(MemoryStore::new());
        let cloud = session(store);
        cloud
            .apply_tokens(
                CloudTokens {
                    access_token: "a".into(),
                    refresh_token: "r".into(),
                    access_expires_at_ms: None,
                },
                AccountSummary {
                    user_id: "user-1".into(),
                    email: "dev@burnly.dev".into(),
                },
            )
            .expect("apply");
        let service =
            AccountService::from_session(cloud, Some("dev_1".into()), "host", login_config());
        assert_eq!(
            service.start_login().expect_err("signed in"),
            AccountServiceError::AlreadySignedIn
        );
    }

    #[test]
    fn logout_clears_pending_and_session() {
        let store = Arc::new(MemoryStore::new());
        let cloud = session(store.clone());
        cloud
            .apply_tokens(
                CloudTokens {
                    access_token: "a".into(),
                    refresh_token: "r".into(),
                    access_expires_at_ms: None,
                },
                AccountSummary {
                    user_id: "user-1".into(),
                    email: "dev@burnly.dev".into(),
                },
            )
            .expect("apply");
        let service =
            AccountService::from_session(cloud, Some("dev_1".into()), "host", login_config());
        // Cannot start while signed in; cancel path via logout
        assert_eq!(
            service.logout().expect("logout"),
            AccountSessionView::signed_out()
        );
        service.start_login().expect("start after logout");
        service.logout().expect("logout again");
        assert!(service.peek_pending_login().is_none());
        assert!(store.load().expect("load").is_none());
    }
}
