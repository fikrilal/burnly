//! Port for minting new access tokens from a refresh token.

#![allow(
    dead_code,
    reason = "Token refresher is implemented by infrastructure cloud adapters"
)]

use crate::application::cloud_session::{CloudSessionError, CloudTokens};

pub(crate) trait CloudTokenRefresher: Send + Sync {
    fn refresh(&self, refresh_token: &str) -> Result<CloudTokens, CloudSessionError>;
}
