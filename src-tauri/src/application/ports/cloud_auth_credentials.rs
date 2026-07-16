//! Credentials surface that the cloud HTTP client uses for Bearer attach and refresh.

#![allow(
    dead_code,
    reason = "Cloud auth credentials are consumed by infrastructure CloudClient"
)]

use crate::application::cloud_session::CloudSessionError;

/// Read/refresh access for authenticated cloud HTTP calls.
///
/// Implemented by `CloudSession`. Infrastructure depends on this port only.
pub(crate) trait CloudAuthCredentials: Send + Sync {
    fn access_token(&self) -> Option<String>;

    fn is_access_expiring_soon(&self, now_epoch_ms: i64, leeway_ms: i64) -> bool;

    fn refresh_single_flight(&self) -> Result<(), CloudSessionError>;
}
