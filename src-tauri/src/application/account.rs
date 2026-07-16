//! Secret-free account session surface for IPC and settings UI.

#![allow(
    dead_code,
    reason = "device id accessors used by later auth chunks"
)]

use std::sync::Arc;

use thiserror::Error;

use crate::application::cloud_session::{CloudSession, SessionSnapshot};

/// Application-owned account handle. Secrets stay inside `CloudSession`.
pub(crate) struct AccountService {
    session: Option<Arc<CloudSession>>,
    device_id: Option<String>,
    device_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AccountSessionStatus {
    SignedOut,
    SignedIn,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AccountSessionView {
    pub status: AccountSessionStatus,
    pub email: Option<String>,
    pub user_id: Option<String>,
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AccountServiceError {
    #[error("account logout failed")]
    LogoutFailed,
}

impl AccountService {
    pub(crate) fn unavailable(
        device_id: Option<String>,
        device_name: impl Into<String>,
    ) -> Self {
        Self {
            session: None,
            device_id,
            device_name: device_name.into(),
        }
    }

    pub(crate) fn from_session(
        session: Arc<CloudSession>,
        device_id: Option<String>,
        device_name: impl Into<String>,
    ) -> Self {
        Self {
            session: Some(session),
            device_id,
            device_name: device_name.into(),
        }
    }

    pub(crate) fn device_id(&self) -> Option<&str> {
        self.device_id.as_deref()
    }

    pub(crate) fn device_name(&self) -> &str {
        &self.device_name
    }

    pub(crate) fn session_view(&self) -> AccountSessionView {
        let Some(session) = &self.session else {
            return AccountSessionView::signed_out();
        };
        match session.snapshot().unwrap_or(SessionSnapshot::SignedOut) {
            SessionSnapshot::SignedOut => AccountSessionView::signed_out(),
            SessionSnapshot::SignedIn { account } => AccountSessionView {
                status: AccountSessionStatus::SignedIn,
                email: Some(account.email),
                user_id: Some(account.user_id),
            },
        }
    }

    pub(crate) fn logout(&self) -> Result<AccountSessionView, AccountServiceError> {
        let Some(session) = &self.session else {
            return Ok(AccountSessionView::signed_out());
        };
        session
            .logout()
            .map_err(|_| AccountServiceError::LogoutFailed)?;
        Ok(AccountSessionView::signed_out())
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
    use std::sync::Mutex;

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

    fn session(store: Arc<MemoryStore>) -> Arc<CloudSession> {
        Arc::new(CloudSession::new(
            store,
            Arc::new(NoopRefresher),
            Arc::new(NoopLogout),
            Arc::new(FixedClock),
        ))
    }

    #[test]
    fn unavailable_is_signed_out_and_logout_is_noop() {
        let service = AccountService::unavailable(None, "host");
        assert_eq!(service.session_view(), AccountSessionView::signed_out());
        assert_eq!(
            service.logout().expect("logout"),
            AccountSessionView::signed_out()
        );
    }

    #[test]
    fn signed_in_view_and_logout_clear_store() {
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

        let service = AccountService::from_session(cloud, Some("dev_1".into()), "host");
        assert_eq!(
            service.session_view(),
            AccountSessionView {
                status: AccountSessionStatus::SignedIn,
                email: Some("dev@burnly.dev".into()),
                user_id: Some("user-1".into()),
            }
        );
        assert_eq!(service.device_id(), Some("dev_1"));

        assert_eq!(
            service.logout().expect("logout"),
            AccountSessionView::signed_out()
        );
        assert_eq!(service.session_view(), AccountSessionView::signed_out());
        assert!(store.load().expect("load").is_none());
    }
}
