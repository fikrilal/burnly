//! OS credential-store adapter for cloud session tokens.

use keyring::Entry;
use serde::{Deserialize, Serialize};

use crate::application::cloud_session::{AccountSummary, CloudTokens};
use crate::application::ports::cloud_token_store::{
    CloudTokenStore, CloudTokenStoreError, StoredCloudSession,
};

const SERVICE: &str = "dev.burnly.desktop";
const ACCOUNT: &str = "cloud_session";

#[derive(Debug, Serialize, Deserialize)]
struct StoredPayload {
    access_token: String,
    refresh_token: String,
    access_expires_at_ms: Option<i64>,
    user_id: String,
    email: String,
}

pub(crate) struct KeyringCloudTokenStore {
    entry: Entry,
}

impl KeyringCloudTokenStore {
    pub(crate) fn new() -> Result<Self, CloudTokenStoreError> {
        let entry = Entry::new(SERVICE, ACCOUNT).map_err(|_| CloudTokenStoreError::Backend)?;
        Ok(Self { entry })
    }
}

impl CloudTokenStore for KeyringCloudTokenStore {
    fn load(&self) -> Result<Option<StoredCloudSession>, CloudTokenStoreError> {
        match self.entry.get_password() {
            Ok(secret) => {
                let payload: StoredPayload =
                    serde_json::from_str(&secret).map_err(|_| CloudTokenStoreError::Backend)?;
                Ok(Some(StoredCloudSession {
                    tokens: CloudTokens {
                        access_token: payload.access_token,
                        refresh_token: payload.refresh_token,
                        access_expires_at_ms: payload.access_expires_at_ms,
                    },
                    account: AccountSummary {
                        user_id: payload.user_id,
                        email: payload.email,
                    },
                }))
            }
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(CloudTokenStoreError::Backend),
        }
    }

    fn save(&self, session: &StoredCloudSession) -> Result<(), CloudTokenStoreError> {
        let payload = StoredPayload {
            access_token: session.tokens.access_token.clone(),
            refresh_token: session.tokens.refresh_token.clone(),
            access_expires_at_ms: session.tokens.access_expires_at_ms,
            user_id: session.account.user_id.clone(),
            email: session.account.email.clone(),
        };
        let secret = serde_json::to_string(&payload).map_err(|_| CloudTokenStoreError::Backend)?;
        self.entry
            .set_password(&secret)
            .map_err(|_| CloudTokenStoreError::Backend)
    }

    fn clear(&self) -> Result<(), CloudTokenStoreError> {
        match self.entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(CloudTokenStoreError::Backend),
        }
    }
}
