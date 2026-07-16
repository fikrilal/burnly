//! Port for exchanging a desktop handoff code for first-party tokens.

#![allow(
    dead_code,
    reason = "Desktop token exchange is used by AccountService complete_login"
)]

use crate::application::cloud_session::{AccountSummary, CloudTokens};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DesktopTokenExchangeRequest {
    pub code: String,
    pub code_verifier: String,
    pub redirect_uri: String,
    pub device_id: Option<String>,
    pub device_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DesktopTokenExchangeResult {
    pub tokens: CloudTokens,
    pub account: AccountSummary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DesktopTokenExchangeError {
    pub code: Option<String>,
    pub message: String,
}

pub(crate) trait DesktopTokenExchanger: Send + Sync {
    fn exchange(
        &self,
        request: DesktopTokenExchangeRequest,
    ) -> Result<DesktopTokenExchangeResult, DesktopTokenExchangeError>;
}
