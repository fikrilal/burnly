//! Thin cloud session orchestration: restore, apply tokens, clear, refresh.
//!
//! Secrets stay in the token store. This type is the application owner of
//! signed-in state for burnly-api calls.

#![allow(
    dead_code,
    reason = "Cloud session is constructed by cloud bootstrap/auth features after Phase 1"
)]

use std::sync::{Arc, Mutex};

use thiserror::Error;

use crate::application::ports::clock::Clock;
use crate::application::ports::cloud_auth_credentials::CloudAuthCredentials;
use crate::application::ports::cloud_remote_logout::CloudRemoteLogout;
use crate::application::ports::cloud_token_refresher::CloudTokenRefresher;
use crate::application::ports::cloud_token_store::{
    CloudTokenStore, CloudTokenStoreError, StoredCloudSession,
};

/// Default preflight window before access token expiry (1 minute).
pub(crate) const ACCESS_TOKEN_EXPIRY_LEEWAY_MS: i64 = 60_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CloudTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub access_expires_at_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AccountSummary {
    pub user_id: String,
    pub email: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SessionSnapshot {
    SignedOut,
    SignedIn { account: AccountSummary },
    SessionExpired { account: AccountSummary },
}

pub(crate) trait CloudSessionObserver: Send + Sync {
    fn on_session_expired(&self, account: &AccountSummary);
}

pub(crate) fn is_terminal_auth_code(code: &str) -> bool {
    matches!(
        code,
        "AUTH_REFRESH_TOKEN_EXPIRED"
            | "AUTH_REFRESH_TOKEN_INVALID"
            | "AUTH_REFRESH_TOKEN_REUSED"
            | "AUTH_SESSION_REVOKED"
    )
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub(crate) enum CloudSessionError {
    #[error("cloud session storage failed")]
    Storage,
    #[error("not signed in")]
    NotSignedIn,
    #[error("token refresh failed")]
    RefreshFailed { code: Option<String> },
    #[error("remote logout failed")]
    LogoutFailed { code: Option<String> },
    #[error("refresh already in progress and failed")]
    RefreshInFlightFailed,
    #[error("signed-in account changed during token refresh")]
    AccountChanged,
}

impl From<CloudTokenStoreError> for CloudSessionError {
    fn from(_value: CloudTokenStoreError) -> Self {
        Self::Storage
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SessionState {
    Active {
        tokens: CloudTokens,
        account: AccountSummary,
    },
    Expired {
        account: AccountSummary,
    },
}

pub(crate) struct CloudSession {
    store: Arc<dyn CloudTokenStore>,
    refresher: Arc<dyn CloudTokenRefresher>,
    remote_logout: Arc<dyn CloudRemoteLogout>,
    clock: Arc<dyn Clock>,
    state: Mutex<Option<SessionState>>,
    refresh_lock: Mutex<()>,
    observer: Mutex<Option<Arc<dyn CloudSessionObserver>>>,
}

impl CloudSession {
    pub(crate) fn new(
        store: Arc<dyn CloudTokenStore>,
        refresher: Arc<dyn CloudTokenRefresher>,
        remote_logout: Arc<dyn CloudRemoteLogout>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            store,
            refresher,
            remote_logout,
            clock,
            state: Mutex::new(None),
            refresh_lock: Mutex::new(()),
            observer: Mutex::new(None),
        }
    }

    pub(crate) fn set_observer(&self, observer: Option<Arc<dyn CloudSessionObserver>>) {
        if let Ok(mut guard) = self.observer.lock() {
            *guard = observer;
        }
    }

    pub(crate) fn restore(&self) -> Result<SessionSnapshot, CloudSessionError> {
        let loaded = self.store.load()?;
        let mut guard = self.state.lock().map_err(|_| CloudSessionError::Storage)?;
        match loaded {
            Some(session) => {
                *guard = Some(SessionState::Active {
                    tokens: session.tokens,
                    account: session.account.clone(),
                });
                Ok(SessionSnapshot::SignedIn {
                    account: session.account,
                })
            }
            None => {
                *guard = None;
                Ok(SessionSnapshot::SignedOut)
            }
        }
    }

    pub(crate) fn snapshot(&self) -> Result<SessionSnapshot, CloudSessionError> {
        let guard = self.state.lock().map_err(|_| CloudSessionError::Storage)?;
        Ok(match guard.as_ref() {
            Some(SessionState::Active { account, .. }) => SessionSnapshot::SignedIn {
                account: account.clone(),
            },
            Some(SessionState::Expired { account }) => SessionSnapshot::SessionExpired {
                account: account.clone(),
            },
            None => SessionSnapshot::SignedOut,
        })
    }

    pub(crate) fn is_signed_in(&self) -> bool {
        self.state
            .lock()
            .map(|guard| matches!(guard.as_ref(), Some(SessionState::Active { .. })))
            .unwrap_or(false)
    }

    pub(crate) fn account(&self) -> Option<AccountSummary> {
        self.state
            .lock()
            .ok()
            .and_then(|guard| match guard.as_ref() {
                Some(SessionState::Active { account, .. })
                | Some(SessionState::Expired { account }) => Some(account.clone()),
                None => None,
            })
    }

    pub(crate) fn apply_tokens(
        &self,
        tokens: CloudTokens,
        account: AccountSummary,
    ) -> Result<(), CloudSessionError> {
        if tokens.access_token.trim().is_empty() || tokens.refresh_token.trim().is_empty() {
            return Err(CloudSessionError::Storage);
        }
        if account.user_id.trim().is_empty() || account.email.trim().is_empty() {
            return Err(CloudSessionError::Storage);
        }

        let mut guard = self.state.lock().map_err(|_| CloudSessionError::Storage)?;
        self.store.save(&StoredCloudSession {
            tokens: tokens.clone(),
            account: account.clone(),
        })?;
        *guard = Some(SessionState::Active { tokens, account });
        Ok(())
    }

    pub(crate) fn clear_local(&self) -> Result<(), CloudSessionError> {
        let mut guard = self.state.lock().map_err(|_| CloudSessionError::Storage)?;
        self.store.clear()?;
        *guard = None;
        Ok(())
    }

    /// Clears local session and best-effort revokes the refresh token remotely.
    pub(crate) fn logout(&self) -> Result<(), CloudSessionError> {
        let refresh_token = {
            let guard = self.state.lock().map_err(|_| CloudSessionError::Storage)?;
            match guard.as_ref() {
                Some(SessionState::Active { tokens, .. }) => Some(tokens.refresh_token.clone()),
                Some(SessionState::Expired { .. }) => None,
                None => return Ok(()),
            }
        };

        // Always clear local first so UI cannot keep a broken signed-in state.
        self.clear_local()?;

        if let Some(refresh_token) = refresh_token {
            let _ = self.remote_logout.logout_remote(&refresh_token);
        }
        Ok(())
    }

    pub(crate) fn access_token(&self) -> Option<String> {
        self.state
            .lock()
            .ok()
            .and_then(|guard| match guard.as_ref() {
                Some(SessionState::Active { tokens, .. }) => Some(tokens.access_token.clone()),
                _ => None,
            })
    }

    pub(crate) fn refresh_single_flight(&self) -> Result<(), CloudSessionError> {
        self.refresh_single_flight_for_expected_user(None)
    }

    fn refresh_single_flight_for_user(
        &self,
        expected_user_id: &str,
    ) -> Result<(), CloudSessionError> {
        self.refresh_single_flight_for_expected_user(Some(expected_user_id))
    }

    fn refresh_single_flight_for_expected_user(
        &self,
        expected_user_id: Option<&str>,
    ) -> Result<(), CloudSessionError> {
        let _refresh_guard = self
            .refresh_lock
            .lock()
            .map_err(|_| CloudSessionError::RefreshInFlightFailed)?;

        let (refresh_token, account) = {
            let guard = self.state.lock().map_err(|_| CloudSessionError::Storage)?;
            match guard.as_ref() {
                Some(SessionState::Active { tokens, account })
                    if expected_user_id.is_none_or(|expected| account.user_id == expected) =>
                {
                    (tokens.refresh_token.clone(), account.clone())
                }
                Some(SessionState::Expired { .. }) => return Err(CloudSessionError::NotSignedIn),
                Some(_) => return Err(CloudSessionError::AccountChanged),
                None => return Err(CloudSessionError::NotSignedIn),
            }
        };

        let new_tokens = match self.refresher.refresh(&refresh_token) {
            Ok(tokens) => tokens,
            Err(err) => {
                if let CloudSessionError::RefreshFailed {
                    code: Some(ref code),
                } = &err
                {
                    if is_terminal_auth_code(code) {
                        let mut guard =
                            self.state.lock().map_err(|_| CloudSessionError::Storage)?;
                        let session_unchanged =
                            guard.as_ref().is_some_and(|session| match session {
                                SessionState::Active {
                                    tokens,
                                    account: cur_account,
                                } => {
                                    cur_account.user_id == account.user_id
                                        && tokens.refresh_token == refresh_token
                                }
                                SessionState::Expired { .. } => false,
                            });
                        if session_unchanged {
                            self.store.clear()?;
                            *guard = Some(SessionState::Expired {
                                account: account.clone(),
                            });
                            drop(guard);
                            if let Ok(guard) = self.observer.lock() {
                                if let Some(observer) = guard.as_ref() {
                                    observer.on_session_expired(&account);
                                }
                            }
                        }
                    }
                }
                return Err(err);
            }
        };

        let mut guard = self.state.lock().map_err(|_| CloudSessionError::Storage)?;
        let session_unchanged = guard.as_ref().is_some_and(|session| match session {
            SessionState::Active {
                tokens,
                account: cur_account,
            } => cur_account.user_id == account.user_id && tokens.refresh_token == refresh_token,
            SessionState::Expired { .. } => false,
        });
        if !session_unchanged {
            return Err(CloudSessionError::AccountChanged);
        };
        self.store.save(&StoredCloudSession {
            tokens: new_tokens.clone(),
            account: account.clone(),
        })?;
        *guard = Some(SessionState::Active {
            tokens: new_tokens,
            account,
        });
        Ok(())
    }
}

impl CloudAuthCredentials for CloudSession {
    fn access_token(&self) -> Option<String> {
        CloudSession::access_token(self)
    }

    fn access_token_for_user(&self, expected_user_id: &str) -> Option<String> {
        self.state
            .lock()
            .ok()
            .and_then(|guard| match guard.as_ref()? {
                SessionState::Active { tokens, account } if account.user_id == expected_user_id => {
                    Some(tokens.access_token.clone())
                }
                _ => None,
            })
    }

    fn is_access_expiring_soon(&self, now_epoch_ms: i64, leeway_ms: i64) -> bool {
        let guard = match self.state.lock() {
            Ok(guard) => guard,
            Err(_) => return false,
        };
        let Some(SessionState::Active { tokens, .. }) = guard.as_ref() else {
            return false;
        };
        let Some(expires_at) = tokens.access_expires_at_ms else {
            return false;
        };
        now_epoch_ms >= expires_at.saturating_sub(leeway_ms)
    }

    fn refresh_single_flight(&self) -> Result<(), CloudSessionError> {
        CloudSession::refresh_single_flight(self)
    }

    fn refresh_single_flight_for_user(
        &self,
        expected_user_id: &str,
    ) -> Result<(), CloudSessionError> {
        CloudSession::refresh_single_flight_for_user(self, expected_user_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::ports::cloud_token_store::CloudTokenStoreError;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Barrier;

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

    struct FixedClock {
        now_ms: i64,
    }

    impl Clock for FixedClock {
        fn now_epoch_ms(&self) -> i64 {
            self.now_ms
        }
    }

    struct CountingRefresher {
        calls: AtomicUsize,
        tokens: CloudTokens,
    }

    impl CloudTokenRefresher for CountingRefresher {
        fn refresh(&self, refresh_token: &str) -> Result<CloudTokens, CloudSessionError> {
            assert_eq!(refresh_token, "refresh-1");
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.tokens.clone())
        }
    }

    struct BlockingRefresher {
        entered: Arc<Barrier>,
        release: Arc<Barrier>,
    }

    impl CloudTokenRefresher for BlockingRefresher {
        fn refresh(&self, refresh_token: &str) -> Result<CloudTokens, CloudSessionError> {
            assert_eq!(refresh_token, "refresh-1");
            self.entered.wait();
            self.release.wait();
            Ok(CloudTokens {
                access_token: "refreshed-a".into(),
                refresh_token: "refresh-a2".into(),
                access_expires_at_ms: Some(9_000_000),
            })
        }
    }

    struct BlockingFailingRefresher {
        entered: Arc<Barrier>,
        release: Arc<Barrier>,
        error_code: &'static str,
    }

    impl CloudTokenRefresher for BlockingFailingRefresher {
        fn refresh(&self, _refresh_token: &str) -> Result<CloudTokens, CloudSessionError> {
            self.entered.wait();
            self.release.wait();
            Err(CloudSessionError::RefreshFailed {
                code: Some(self.error_code.into()),
            })
        }
    }

    struct FailingClearStore {
        inner: MemoryStore,
    }

    impl CloudTokenStore for FailingClearStore {
        fn load(&self) -> Result<Option<StoredCloudSession>, CloudTokenStoreError> {
            self.inner.load()
        }

        fn save(&self, session: &StoredCloudSession) -> Result<(), CloudTokenStoreError> {
            self.inner.save(session)
        }

        fn clear(&self) -> Result<(), CloudTokenStoreError> {
            Err(CloudTokenStoreError::Backend)
        }
    }

    struct NoopLogout;

    impl CloudRemoteLogout for NoopLogout {
        fn logout_remote(&self, _refresh_token: &str) -> Result<(), CloudSessionError> {
            Ok(())
        }
    }

    fn sample_tokens(expires_at: Option<i64>) -> CloudTokens {
        CloudTokens {
            access_token: "access-1".into(),
            refresh_token: "refresh-1".into(),
            access_expires_at_ms: expires_at,
        }
    }

    fn sample_account() -> AccountSummary {
        AccountSummary {
            user_id: "user-1".into(),
            email: "dev@burnly.dev".into(),
        }
    }

    fn session_with_store(
        store: Arc<dyn CloudTokenStore>,
        refresher: Arc<dyn CloudTokenRefresher>,
    ) -> CloudSession {
        CloudSession::new(
            store,
            refresher,
            Arc::new(NoopLogout),
            Arc::new(FixedClock { now_ms: 1_000_000 }),
        )
    }

    fn session_with(
        store: Arc<MemoryStore>,
        refresher: Arc<dyn CloudTokenRefresher>,
    ) -> CloudSession {
        session_with_store(store, refresher)
    }

    #[test]
    fn apply_restore_and_clear_round_trip() {
        let store = Arc::new(MemoryStore::new());
        let refresher = Arc::new(CountingRefresher {
            calls: AtomicUsize::new(0),
            tokens: sample_tokens(Some(2_000_000)),
        });
        let session = session_with(store.clone(), refresher);

        session
            .apply_tokens(sample_tokens(Some(2_000_000)), sample_account())
            .expect("apply");
        assert!(session.is_signed_in());
        assert_eq!(session.account().expect("account").email, "dev@burnly.dev");

        let restored = session_with(
            store,
            Arc::new(CountingRefresher {
                calls: AtomicUsize::new(0),
                tokens: sample_tokens(Some(2_000_000)),
            }),
        );
        assert_eq!(
            restored.restore().expect("restore"),
            SessionSnapshot::SignedIn {
                account: sample_account()
            }
        );
        assert_eq!(restored.access_token().as_deref(), Some("access-1"));

        restored.clear_local().expect("clear");
        assert!(!restored.is_signed_in());
        assert_eq!(
            restored.restore().expect("restore empty"),
            SessionSnapshot::SignedOut
        );
    }

    #[test]
    fn refresh_single_flight_updates_tokens_once() {
        let store = Arc::new(MemoryStore::new());
        let refresher = Arc::new(CountingRefresher {
            calls: AtomicUsize::new(0),
            tokens: CloudTokens {
                access_token: "access-2".into(),
                refresh_token: "refresh-2".into(),
                access_expires_at_ms: Some(9_000_000),
            },
        });
        let session = session_with(store, refresher.clone());
        session
            .apply_tokens(sample_tokens(Some(1_000)), sample_account())
            .expect("apply");

        session.refresh_single_flight().expect("refresh");
        assert_eq!(session.access_token().as_deref(), Some("access-2"));
        assert_eq!(refresher.calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn refresh_cannot_replace_a_newly_signed_in_account() {
        let store = Arc::new(MemoryStore::new());
        let entered = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));
        let session = Arc::new(session_with(
            store.clone(),
            Arc::new(BlockingRefresher {
                entered: entered.clone(),
                release: release.clone(),
            }),
        ));
        session
            .apply_tokens(sample_tokens(Some(1_000)), sample_account())
            .expect("apply a");

        let worker = {
            let session = session.clone();
            std::thread::spawn(move || session.refresh_single_flight_for_user("user-1"))
        };
        entered.wait();
        let account_b = AccountSummary {
            user_id: "user-2".into(),
            email: "other@burnly.dev".into(),
        };
        let tokens_b = CloudTokens {
            access_token: "access-b".into(),
            refresh_token: "refresh-b".into(),
            access_expires_at_ms: Some(9_000_000),
        };
        session
            .apply_tokens(tokens_b.clone(), account_b.clone())
            .expect("apply b");
        release.wait();

        assert_eq!(
            worker.join().expect("worker"),
            Err(CloudSessionError::AccountChanged)
        );
        assert_eq!(
            session.snapshot().expect("snapshot"),
            SessionSnapshot::SignedIn {
                account: account_b.clone()
            }
        );
        assert_eq!(
            store.load().expect("stored"),
            Some(StoredCloudSession {
                tokens: tokens_b,
                account: account_b,
            })
        );
    }

    #[test]
    fn logout_clears_local_session() {
        let store = Arc::new(MemoryStore::new());
        let session = session_with(
            store.clone(),
            Arc::new(CountingRefresher {
                calls: AtomicUsize::new(0),
                tokens: sample_tokens(None),
            }),
        );
        session
            .apply_tokens(sample_tokens(None), sample_account())
            .expect("apply");
        session.logout().expect("logout");
        assert!(!session.is_signed_in());
        assert!(store.load().expect("load").is_none());
    }

    #[test]
    fn access_expiring_soon_respects_leeway() {
        let store = Arc::new(MemoryStore::new());
        let session = session_with(
            store,
            Arc::new(CountingRefresher {
                calls: AtomicUsize::new(0),
                tokens: sample_tokens(None),
            }),
        );
        session
            .apply_tokens(sample_tokens(Some(1_030_000)), sample_account())
            .expect("apply");

        let credentials: &dyn CloudAuthCredentials = &session;
        assert!(credentials.is_access_expiring_soon(1_000_000, ACCESS_TOKEN_EXPIRY_LEEWAY_MS));
        assert!(!credentials.is_access_expiring_soon(900_000, ACCESS_TOKEN_EXPIRY_LEEWAY_MS));
    }

    struct ErrorRefresher {
        code: Option<String>,
    }

    impl CloudTokenRefresher for ErrorRefresher {
        fn refresh(&self, _refresh_token: &str) -> Result<CloudTokens, CloudSessionError> {
            Err(CloudSessionError::RefreshFailed {
                code: self.code.clone(),
            })
        }
    }

    struct RecordingObserver {
        expired: Mutex<Option<AccountSummary>>,
    }

    impl CloudSessionObserver for RecordingObserver {
        fn on_session_expired(&self, account: &AccountSummary) {
            *self.expired.lock().expect("lock") = Some(account.clone());
        }
    }

    #[test]
    fn terminal_refresh_error_clears_store_and_expires_session() {
        let store = Arc::new(MemoryStore::new());
        let session = session_with(
            store.clone(),
            Arc::new(ErrorRefresher {
                code: Some("AUTH_REFRESH_TOKEN_EXPIRED".into()),
            }),
        );
        let observer = Arc::new(RecordingObserver {
            expired: Mutex::new(None),
        });
        session.set_observer(Some(observer.clone()));

        session
            .apply_tokens(sample_tokens(None), sample_account())
            .expect("apply");
        assert!(session.is_signed_in());
        assert!(store.load().expect("load").is_some());

        let res = session.refresh_single_flight();
        assert!(res.is_err());
        assert!(!session.is_signed_in());
        assert!(session.access_token().is_none());
        assert!(store.load().expect("store should be cleared").is_none());

        match session.snapshot().expect("snapshot") {
            SessionSnapshot::SessionExpired { account } => {
                assert_eq!(account.email, "dev@burnly.dev");
            }
            other => panic!("expected SessionExpired, got {other:?}"),
        }

        let observed = observer.expired.lock().expect("lock").clone();
        assert_eq!(
            observed.as_ref().map(|a| a.email.as_str()),
            Some("dev@burnly.dev")
        );

        session.logout().expect("logout");
        assert_eq!(
            session.snapshot().expect("snapshot"),
            SessionSnapshot::SignedOut
        );
    }

    #[test]
    fn non_terminal_refresh_error_preserves_store_and_signed_in_state() {
        let store = Arc::new(MemoryStore::new());
        let session = session_with(
            store.clone(),
            Arc::new(ErrorRefresher {
                code: Some("NETWORK_TIMEOUT".into()),
            }),
        );
        let observer = Arc::new(RecordingObserver {
            expired: Mutex::new(None),
        });
        session.set_observer(Some(observer.clone()));

        session
            .apply_tokens(sample_tokens(None), sample_account())
            .expect("apply");

        let res = session.refresh_single_flight();
        assert!(res.is_err());
        assert!(session.is_signed_in());
        assert!(store.load().expect("load").is_some());
        assert_eq!(
            session.snapshot().expect("snapshot"),
            SessionSnapshot::SignedIn {
                account: sample_account()
            }
        );
        assert!(observer.expired.lock().expect("lock").is_none());
    }

    #[test]
    fn terminal_refresh_failure_cannot_overwrite_a_newly_signed_in_account() {
        let store = Arc::new(MemoryStore::new());
        let entered = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));
        let session = Arc::new(session_with(
            store.clone(),
            Arc::new(BlockingFailingRefresher {
                entered: entered.clone(),
                release: release.clone(),
                error_code: "AUTH_REFRESH_TOKEN_EXPIRED",
            }),
        ));
        let observer = Arc::new(RecordingObserver {
            expired: Mutex::new(None),
        });
        session.set_observer(Some(observer.clone()));

        session
            .apply_tokens(sample_tokens(Some(1_000)), sample_account())
            .expect("apply a");

        let worker = {
            let session = session.clone();
            std::thread::spawn(move || session.refresh_single_flight_for_user("user-1"))
        };
        entered.wait();

        let account_b = AccountSummary {
            user_id: "user-2".into(),
            email: "other@burnly.dev".into(),
        };
        let tokens_b = CloudTokens {
            access_token: "access-b".into(),
            refresh_token: "refresh-b".into(),
            access_expires_at_ms: Some(9_000_000),
        };
        session
            .apply_tokens(tokens_b.clone(), account_b.clone())
            .expect("apply b");
        release.wait();

        assert!(worker.join().expect("worker").is_err());
        assert_eq!(
            session.snapshot().expect("snapshot"),
            SessionSnapshot::SignedIn {
                account: account_b.clone()
            }
        );
        assert_eq!(
            store.load().expect("stored"),
            Some(StoredCloudSession {
                tokens: tokens_b,
                account: account_b,
            })
        );
        assert!(observer.expired.lock().expect("lock").is_none());
    }

    #[test]
    fn terminal_refresh_failure_cannot_resurrect_a_logged_out_account() {
        let store = Arc::new(MemoryStore::new());
        let entered = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));
        let session = Arc::new(session_with(
            store.clone(),
            Arc::new(BlockingFailingRefresher {
                entered: entered.clone(),
                release: release.clone(),
                error_code: "AUTH_SESSION_REVOKED",
            }),
        ));
        let observer = Arc::new(RecordingObserver {
            expired: Mutex::new(None),
        });
        session.set_observer(Some(observer.clone()));

        session
            .apply_tokens(sample_tokens(Some(1_000)), sample_account())
            .expect("apply");

        let worker = {
            let session = session.clone();
            std::thread::spawn(move || session.refresh_single_flight_for_user("user-1"))
        };
        entered.wait();

        session.logout().expect("logout");
        release.wait();

        assert!(worker.join().expect("worker").is_err());
        assert_eq!(
            session.snapshot().expect("snapshot"),
            SessionSnapshot::SignedOut
        );
        assert!(store.load().expect("stored").is_none());
        assert!(observer.expired.lock().expect("lock").is_none());
    }

    #[test]
    fn terminal_refresh_failure_with_failing_store_clear_does_not_expire_in_memory() {
        let store = Arc::new(FailingClearStore {
            inner: MemoryStore::new(),
        });
        let session = session_with_store(
            store.clone(),
            Arc::new(ErrorRefresher {
                code: Some("AUTH_REFRESH_TOKEN_EXPIRED".into()),
            }),
        );
        let observer = Arc::new(RecordingObserver {
            expired: Mutex::new(None),
        });
        session.set_observer(Some(observer.clone()));

        session
            .apply_tokens(sample_tokens(None), sample_account())
            .expect("apply");

        let res = session.refresh_single_flight();
        assert!(matches!(res, Err(CloudSessionError::Storage)));
        assert!(session.is_signed_in());
        assert_eq!(
            session.snapshot().expect("snapshot"),
            SessionSnapshot::SignedIn {
                account: sample_account()
            }
        );
        assert!(observer.expired.lock().expect("lock").is_none());
    }
}
