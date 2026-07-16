//! Durable storage for cloud session tokens and display cache.
//!
//! Secrets stay behind this port. UI never reads the store directly.

#![allow(
    dead_code,
    reason = "Cloud token store is wired by Phase 1 cloud core before product auth UI"
)]

use crate::application::cloud_session::{AccountSummary, CloudTokens};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StoredCloudSession {
    pub tokens: CloudTokens,
    pub account: AccountSummary,
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub(crate) enum CloudTokenStoreError {
    #[error("cloud token storage failed")]
    Backend,
}

pub(crate) trait CloudTokenStore: Send + Sync {
    fn load(&self) -> Result<Option<StoredCloudSession>, CloudTokenStoreError>;

    fn save(&self, session: &StoredCloudSession) -> Result<(), CloudTokenStoreError>;

    fn clear(&self) -> Result<(), CloudTokenStoreError>;
}
