//! `POST /v1/auth/logout` best-effort remote session revoke.

use std::sync::Arc;

use serde::Serialize;

use crate::application::cloud_session::CloudSessionError;
use crate::application::ports::cloud_remote_logout::CloudRemoteLogout;

use super::client::{CloudAuthMode, CloudClient};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LogoutRequestBody {
    refresh_token: String,
}

pub(crate) struct HttpCloudRemoteLogout {
    client: Arc<CloudClient>,
}

impl HttpCloudRemoteLogout {
    pub(crate) fn new(client: Arc<CloudClient>) -> Self {
        Self { client }
    }
}

impl CloudRemoteLogout for HttpCloudRemoteLogout {
    fn logout_remote(&self, refresh_token: &str) -> Result<(), CloudSessionError> {
        if refresh_token.trim().is_empty() {
            return Ok(());
        }
        self.client
            .post_ok(
                "/v1/auth/logout",
                &LogoutRequestBody {
                    refresh_token: refresh_token.to_owned(),
                },
                CloudAuthMode::Public,
                None,
            )
            .map_err(Into::<CloudSessionError>::into)
    }
}
