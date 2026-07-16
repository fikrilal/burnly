//! Best-effort remote logout against burnly-api.

#![allow(
    dead_code,
    reason = "Remote logout is implemented by infrastructure cloud adapters"
)]

use crate::application::cloud_session::CloudSessionError;

pub(crate) trait CloudRemoteLogout: Send + Sync {
    /// Revoke the refresh token server-side when possible.
    ///
    /// Local session clear proceeds even when this fails; callers treat remote
    /// logout as best-effort.
    fn logout_remote(&self, refresh_token: &str) -> Result<(), CloudSessionError>;
}
