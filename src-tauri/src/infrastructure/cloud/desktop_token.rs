//! `POST /v1/auth/desktop/token` adapter for desktop web handoff.

use std::sync::Arc;

use serde::Deserialize;
use serde::Serialize;

use crate::application::cloud_session::{AccountSummary, CloudTokens};
use crate::application::ports::desktop_token_exchanger::{
    DesktopTokenExchangeError, DesktopTokenExchangeRequest, DesktopTokenExchangeResult,
    DesktopTokenExchanger,
};

use super::client::{CloudAuthMode, CloudClient};
use super::error::CloudApiError;
use super::jwt::access_expires_at_ms;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopTokenRequestBody {
    code: String,
    code_verifier: String,
    redirect_uri: String,
    client: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    device_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    device_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuthUserBody {
    id: String,
    email: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuthResultBody {
    user: AuthUserBody,
    access_token: String,
    refresh_token: String,
}

pub(crate) struct HttpDesktopTokenExchanger {
    client: Arc<CloudClient>,
}

impl HttpDesktopTokenExchanger {
    pub(crate) fn new(client: Arc<CloudClient>) -> Self {
        Self { client }
    }
}

impl DesktopTokenExchanger for HttpDesktopTokenExchanger {
    fn exchange(
        &self,
        request: DesktopTokenExchangeRequest,
    ) -> Result<DesktopTokenExchangeResult, DesktopTokenExchangeError> {
        let body = DesktopTokenRequestBody {
            code: request.code,
            code_verifier: request.code_verifier,
            redirect_uri: request.redirect_uri,
            client: "desktop",
            device_id: request.device_id,
            device_name: if request.device_name.trim().is_empty() {
                None
            } else {
                Some(request.device_name)
            },
        };

        let envelope = self
            .client
            .post_json::<_, AuthResultBody>(
                "/v1/auth/desktop/token",
                &body,
                CloudAuthMode::Public,
                None,
            )
            .map_err(map_api_error)?;

        let access = envelope.data.access_token;
        Ok(DesktopTokenExchangeResult {
            tokens: CloudTokens {
                access_expires_at_ms: access_expires_at_ms(&access),
                access_token: access,
                refresh_token: envelope.data.refresh_token,
            },
            account: AccountSummary {
                user_id: envelope.data.user.id,
                email: envelope.data.user.email,
            },
        })
    }
}

fn map_api_error(error: CloudApiError) -> DesktopTokenExchangeError {
    DesktopTokenExchangeError {
        code: error.code.clone(),
        message: error.message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::ports::clock::Clock;
    use crate::infrastructure::cloud::client::{
        CloudHttpMethod, CloudHttpTransport, CloudRawResponse,
    };
    use crate::infrastructure::cloud::config::CloudConfig;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    struct FixedClock;
    impl Clock for FixedClock {
        fn now_epoch_ms(&self) -> i64 {
            1
        }
    }

    struct ScriptedTransport {
        response: Mutex<Option<CloudRawResponse>>,
        calls: AtomicUsize,
    }

    impl CloudHttpTransport for ScriptedTransport {
        fn send(
            &self,
            _url: &str,
            _method: CloudHttpMethod,
            _headers: &[(String, String)],
            _body: Option<&[u8]>,
        ) -> Result<CloudRawResponse, CloudApiError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.response
                .lock()
                .expect("lock")
                .take()
                .ok_or_else(|| CloudApiError::internal("no response"))
        }
    }

    #[test]
    fn maps_successful_auth_result() {
        use base64::Engine as _;
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(br#"{"exp":1700000000}"#);
        let access = format!("aaa.{payload}.sig");
        let body = format!(
            r#"{{"data":{{"user":{{"id":"user-1","email":"dev@burnly.dev"}},"accessToken":"{access}","refreshToken":"refresh-1"}}}}"#
        );

        let transport = Arc::new(ScriptedTransport {
            response: Mutex::new(Some(CloudRawResponse {
                status: 200,
                body: body.into_bytes(),
                request_id: None,
            })),
            calls: AtomicUsize::new(0),
        });
        let client = Arc::new(CloudClient::new(
            CloudConfig::new(
                "http://127.0.0.1:4000",
                "http://127.0.0.1:3000",
                "http://127.0.0.1:39201/callback",
                "0.1.20",
            )
            .expect("config"),
            transport.clone(),
            None,
            Arc::new(FixedClock),
        ));
        let exchanger = HttpDesktopTokenExchanger::new(client);
        let result = exchanger
            .exchange(DesktopTokenExchangeRequest {
                code: "code-1".into(),
                code_verifier: "verifier-1".into(),
                redirect_uri: "http://127.0.0.1:39201/callback".into(),
                device_id: Some("dev_1".into()),
                device_name: "host".into(),
            })
            .expect("exchange");
        assert_eq!(result.account.email, "dev@burnly.dev");
        assert_eq!(result.tokens.refresh_token, "refresh-1");
        assert_eq!(transport.calls.load(Ordering::SeqCst), 1);
    }
}
