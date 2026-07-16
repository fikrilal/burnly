//! `POST /v1/auth/refresh` adapter for the cloud token refresher port.

use std::sync::Arc;

use serde::Deserialize;
use serde::Serialize;

use crate::application::cloud_session::{CloudSessionError, CloudTokens};
use crate::application::ports::cloud_token_refresher::CloudTokenRefresher;

use super::client::{CloudAuthMode, CloudClient};
use super::jwt::access_expires_at_ms;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RefreshRequestBody {
    refresh_token: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuthResultBody {
    access_token: String,
    refresh_token: String,
}

pub(crate) struct HttpCloudTokenRefresher {
    client: Arc<CloudClient>,
}

impl HttpCloudTokenRefresher {
    pub(crate) fn new(client: Arc<CloudClient>) -> Self {
        Self { client }
    }
}

impl CloudTokenRefresher for HttpCloudTokenRefresher {
    fn refresh(&self, refresh_token: &str) -> Result<CloudTokens, CloudSessionError> {
        if refresh_token.trim().is_empty() {
            return Err(CloudSessionError::NotSignedIn);
        }
        let envelope = self
            .client
            .post_json::<_, AuthResultBody>(
                "/v1/auth/refresh",
                &RefreshRequestBody {
                    refresh_token: refresh_token.to_owned(),
                },
                CloudAuthMode::Public,
                None,
            )
            .map_err(Into::<CloudSessionError>::into)?;

        Ok(CloudTokens {
            access_token: envelope.data.access_token.clone(),
            refresh_token: envelope.data.refresh_token,
            access_expires_at_ms: access_expires_at_ms(&envelope.data.access_token),
        })
    }
}
