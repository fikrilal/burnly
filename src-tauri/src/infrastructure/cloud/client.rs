//! Single burnly-api HTTP client: envelope parse, problem mapping, auth retry.

use std::sync::Arc;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

use crate::application::cloud_session::{
    CloudSessionError, ACCESS_TOKEN_EXPIRY_LEEWAY_MS,
};
use crate::application::ports::clock::Clock;
use crate::application::ports::cloud_auth_credentials::CloudAuthCredentials;

use super::config::CloudConfig;
use super::error::{CloudApiError, CloudApiErrorKind, CloudFieldError};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CloudHttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

impl CloudHttpMethod {
    fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
        }
    }

    fn is_write(self) -> bool {
        matches!(self, Self::Post | Self::Put | Self::Patch | Self::Delete)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CloudAuthMode {
    Public,
    Authenticated,
}

#[derive(Debug, Clone)]
pub(crate) struct CloudRequest {
    pub method: CloudHttpMethod,
    pub path: String,
    pub auth: CloudAuthMode,
    pub idempotency_key: Option<String>,
    pub body: Option<Value>,
}

#[derive(Debug, Clone)]
pub(crate) struct CloudRawResponse {
    pub status: u16,
    pub body: Vec<u8>,
    pub request_id: Option<String>,
}

pub(crate) trait CloudHttpTransport: Send + Sync {
    fn send(
        &self,
        url: &str,
        method: CloudHttpMethod,
        headers: &[(String, String)],
        body: Option<&[u8]>,
    ) -> Result<CloudRawResponse, CloudApiError>;
}

pub(crate) struct ReqwestTransport {
    client: reqwest::blocking::Client,
}

impl ReqwestTransport {
    pub(crate) fn new() -> Result<Self, CloudApiError> {
        let client = reqwest::blocking::Client::builder()
            .timeout(DEFAULT_TIMEOUT)
            .build()
            .map_err(|error| CloudApiError::network(error.to_string()))?;
        Ok(Self { client })
    }
}

impl CloudHttpTransport for ReqwestTransport {
    fn send(
        &self,
        url: &str,
        method: CloudHttpMethod,
        headers: &[(String, String)],
        body: Option<&[u8]>,
    ) -> Result<CloudRawResponse, CloudApiError> {
        let mut builder = match method {
            CloudHttpMethod::Get => self.client.get(url),
            CloudHttpMethod::Post => self.client.post(url),
            CloudHttpMethod::Put => self.client.put(url),
            CloudHttpMethod::Patch => self.client.patch(url),
            CloudHttpMethod::Delete => self.client.delete(url),
        };
        for (name, value) in headers {
            builder = builder.header(name, value);
        }
        if let Some(body) = body {
            builder = builder
                .header("Content-Type", "application/json")
                .body(body.to_vec());
        }

        let response = builder.send().map_err(|error| {
            if error.is_timeout() {
                CloudApiError::timeout(error.to_string())
            } else {
                CloudApiError::network(error.to_string())
            }
        })?;

        let status = response.status().as_u16();
        let request_id = response
            .headers()
            .get("x-request-id")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let body = response
            .bytes()
            .map_err(|error| CloudApiError::network(error.to_string()))?
            .to_vec();
        Ok(CloudRawResponse {
            status,
            body,
            request_id,
        })
    }
}

#[derive(Debug, Clone)]
pub(crate) struct CloudEnvelope<T> {
    pub data: T,
    pub meta: Option<Value>,
    pub trace_id: Option<String>,
}

pub(crate) struct CloudClient {
    config: CloudConfig,
    transport: Arc<dyn CloudHttpTransport>,
    credentials: Option<Arc<dyn CloudAuthCredentials>>,
    clock: Arc<dyn Clock>,
}

impl CloudClient {
    pub(crate) fn new(
        config: CloudConfig,
        transport: Arc<dyn CloudHttpTransport>,
        credentials: Option<Arc<dyn CloudAuthCredentials>>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            config,
            transport,
            credentials,
            clock,
        }
    }

    pub(crate) fn config(&self) -> &CloudConfig {
        &self.config
    }

    pub(crate) fn get_json<T: DeserializeOwned>(
        &self,
        path: &str,
        auth: CloudAuthMode,
    ) -> Result<CloudEnvelope<T>, CloudApiError> {
        self.request_json(CloudRequest {
            method: CloudHttpMethod::Get,
            path: path.to_owned(),
            auth,
            idempotency_key: None,
            body: None,
        })
    }

    pub(crate) fn post_json<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
        auth: CloudAuthMode,
        idempotency_key: Option<String>,
    ) -> Result<CloudEnvelope<T>, CloudApiError> {
        let body = serde_json::to_value(body).map_err(|error| CloudApiError::decode(error.to_string()))?;
        self.request_json(CloudRequest {
            method: CloudHttpMethod::Post,
            path: path.to_owned(),
            auth,
            idempotency_key,
            body: Some(body),
        })
    }

    pub(crate) fn put_json<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
        auth: CloudAuthMode,
        idempotency_key: Option<String>,
    ) -> Result<CloudEnvelope<T>, CloudApiError> {
        let body = serde_json::to_value(body).map_err(|error| CloudApiError::decode(error.to_string()))?;
        self.request_json(CloudRequest {
            method: CloudHttpMethod::Put,
            path: path.to_owned(),
            auth,
            idempotency_key,
            body: Some(body),
        })
    }

    /// POST that accepts 2xx with empty body (e.g. logout 204).
    pub(crate) fn post_ok<B: Serialize>(
        &self,
        path: &str,
        body: &B,
        auth: CloudAuthMode,
        idempotency_key: Option<String>,
    ) -> Result<(), CloudApiError> {
        let body =
            serde_json::to_value(body).map_err(|error| CloudApiError::decode(error.to_string()))?;
        let response = self.execute_with_auth_policy(
            &CloudRequest {
                method: CloudHttpMethod::Post,
                path: path.to_owned(),
                auth,
                idempotency_key,
                body: Some(body),
            },
            false,
        )?;
        if (200..300).contains(&response.status) {
            return Ok(());
        }
        Err(parse_problem(&response))
    }

    pub(crate) fn request_json<T: DeserializeOwned>(
        &self,
        request: CloudRequest,
    ) -> Result<CloudEnvelope<T>, CloudApiError> {
        let response = self.execute_with_auth_policy(&request, false)?;
        parse_success_envelope(&response)
    }

    fn execute_with_auth_policy(
        &self,
        request: &CloudRequest,
        already_retried: bool,
    ) -> Result<CloudRawResponse, CloudApiError> {
        if request.auth == CloudAuthMode::Authenticated {
            if let Some(credentials) = &self.credentials {
                if credentials
                    .is_access_expiring_soon(self.clock.now_epoch_ms(), ACCESS_TOKEN_EXPIRY_LEEWAY_MS)
                {
                    let _ = credentials.refresh_single_flight();
                }
            }
        }

        let response = self.execute_once(request)?;
        if response.status != 401 || already_retried || request.auth != CloudAuthMode::Authenticated
        {
            return Ok(response);
        }

        // Only treat unauthorized problem codes as refresh candidates.
        if let Err(error) = ensure_success_status(&response) {
            if !error.is_unauthorized() {
                return Err(error);
            }
        }

        if request.method.is_write() && request.idempotency_key.is_none() {
            return ensure_success_status(&response).map(|()| response);
        }

        let Some(credentials) = &self.credentials else {
            return ensure_success_status(&response).map(|()| response);
        };

        match credentials.refresh_single_flight() {
            Ok(()) => self.execute_with_auth_policy(request, true),
            Err(CloudSessionError::RefreshFailed { code })
                if code.as_deref().is_some_and(|value| {
                    matches!(
                        value,
                        "AUTH_REFRESH_TOKEN_INVALID"
                            | "AUTH_REFRESH_TOKEN_EXPIRED"
                            | "AUTH_REFRESH_TOKEN_REUSED"
                            | "AUTH_SESSION_REVOKED"
                    )
                }) =>
            {
                Err(CloudApiError {
                    kind: CloudApiErrorKind::Unauthorized,
                    message: "session refresh failed".into(),
                    code,
                    status: Some(401),
                    trace_id: response.request_id.clone(),
                    field_errors: Vec::new(),
                })
            }
            Err(_) => ensure_success_status(&response).map(|()| response),
        }
    }

    fn execute_once(&self, request: &CloudRequest) -> Result<CloudRawResponse, CloudApiError> {
        let url = self.config.api_url(&request.path);
        let request_id = Uuid::new_v4().to_string();
        let mut headers = vec![
            ("X-Request-Id".to_owned(), request_id),
            (
                "User-Agent".to_owned(),
                format!("BurnlyDesktop/{}", self.config.app_version()),
            ),
        ];

        if let Some(key) = &request.idempotency_key {
            headers.push(("Idempotency-Key".to_owned(), key.clone()));
        }

        if request.auth == CloudAuthMode::Authenticated {
            let token = self
                .credentials
                .as_ref()
                .and_then(|credentials| credentials.access_token())
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    CloudApiError::from_problem(
                        401,
                        Some("UNAUTHORIZED".into()),
                        "not signed in".into(),
                        None,
                        Vec::new(),
                    )
                })?;
            headers.push(("Authorization".to_owned(), format!("Bearer {token}")));
        }

        let body_bytes = match &request.body {
            Some(value) => Some(
                serde_json::to_vec(value)
                    .map_err(|error| CloudApiError::decode(error.to_string()))?,
            ),
            None => None,
        };

        self.transport.send(
            &url,
            request.method,
            &headers,
            body_bytes.as_deref(),
        )
    }
}

pub(crate) fn parse_success_envelope<T: DeserializeOwned>(
    response: &CloudRawResponse,
) -> Result<CloudEnvelope<T>, CloudApiError> {
    ensure_success_status(response)?;
    if response.body.is_empty() {
        return Err(CloudApiError::decode("empty success body"));
    }
    let root: Value = serde_json::from_slice(&response.body)
        .map_err(|error| CloudApiError::decode(error.to_string()))?;
    let data = root
        .get("data")
        .cloned()
        .ok_or_else(|| CloudApiError::decode("missing data field"))?;
    let meta = root.get("meta").cloned();
    let parsed: T =
        serde_json::from_value(data).map_err(|error| CloudApiError::decode(error.to_string()))?;
    Ok(CloudEnvelope {
        data: parsed,
        meta,
        trace_id: response.request_id.clone(),
    })
}

pub(crate) fn ensure_success_status(response: &CloudRawResponse) -> Result<(), CloudApiError> {
    if (200..300).contains(&response.status) {
        return Ok(());
    }
    Err(parse_problem(response))
}

pub(crate) fn parse_problem(response: &CloudRawResponse) -> CloudApiError {
    let trace_from_header = response.request_id.clone();
    if response.body.is_empty() {
        return CloudApiError::from_problem(
            response.status,
            None,
            format!("HTTP {}", response.status),
            trace_from_header,
            Vec::new(),
        );
    }

    let Ok(root) = serde_json::from_slice::<Value>(&response.body) else {
        return CloudApiError::from_problem(
            response.status,
            None,
            format!("HTTP {}", response.status),
            trace_from_header,
            Vec::new(),
        );
    };

    let code = root
        .get("code")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let trace_id = root
        .get("traceId")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or(trace_from_header);
    let title = root.get("title").and_then(Value::as_str);
    let detail = root.get("detail").and_then(Value::as_str);
    let message = title
        .or(detail)
        .map(str::to_owned)
        .unwrap_or_else(|| format!("HTTP {}", response.status));

    let field_errors = root
        .get("errors")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    Some(CloudFieldError {
                        field: item.get("field")?.as_str()?.to_owned(),
                        code: item
                            .get("code")
                            .and_then(Value::as_str)
                            .unwrap_or("invalid")
                            .to_owned(),
                        message: item
                            .get("message")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_owned(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    CloudApiError::from_problem(response.status, code, message, trace_id, field_errors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::cloud_session::CloudSessionError;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    struct FixedClock {
        now_ms: i64,
    }

    impl Clock for FixedClock {
        fn now_epoch_ms(&self) -> i64 {
            self.now_ms
        }
    }

    struct ScriptedTransport {
        responses: Mutex<Vec<CloudRawResponse>>,
        calls: AtomicUsize,
        last_auth: Mutex<Option<String>>,
    }

    impl ScriptedTransport {
        fn new(responses: Vec<CloudRawResponse>) -> Self {
            Self {
                responses: Mutex::new(responses),
                calls: AtomicUsize::new(0),
                last_auth: Mutex::new(None),
            }
        }
    }

    impl CloudHttpTransport for ScriptedTransport {
        fn send(
            &self,
            _url: &str,
            _method: CloudHttpMethod,
            headers: &[(String, String)],
            _body: Option<&[u8]>,
        ) -> Result<CloudRawResponse, CloudApiError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let auth = headers
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case("Authorization"))
                .map(|(_, value)| value.clone());
            *self.last_auth.lock().expect("lock") = auth;
            let mut guard = self.responses.lock().expect("lock");
            if guard.is_empty() {
                return Err(CloudApiError::internal("no scripted response"));
            }
            Ok(guard.remove(0))
        }
    }

    struct FakeCredentials {
        token: Mutex<String>,
        refresh_calls: AtomicUsize,
        expiring: bool,
        refresh_ok: bool,
    }

    impl CloudAuthCredentials for FakeCredentials {
        fn access_token(&self) -> Option<String> {
            Some(self.token.lock().expect("lock").clone())
        }

        fn is_access_expiring_soon(&self, _now_epoch_ms: i64, _leeway_ms: i64) -> bool {
            self.expiring
        }

        fn refresh_single_flight(&self) -> Result<(), CloudSessionError> {
            self.refresh_calls.fetch_add(1, Ordering::SeqCst);
            if self.refresh_ok {
                *self.token.lock().expect("lock") = "access-refreshed".into();
                Ok(())
            } else {
                Err(CloudSessionError::RefreshFailed {
                    code: Some("AUTH_REFRESH_TOKEN_EXPIRED".into()),
                })
            }
        }
    }

    fn config() -> CloudConfig {
        CloudConfig::new(
            "http://127.0.0.1:4000",
            "http://127.0.0.1:3000",
            "http://127.0.0.1:39201/callback",
            "0.1.20",
        )
        .expect("config")
    }

    #[test]
    fn parses_success_envelope() {
        let body = br#"{"data":{"ok":true},"meta":{"limit":1}}"#.to_vec();
        let response = CloudRawResponse {
            status: 200,
            body,
            request_id: Some("req_1".into()),
        };
        #[derive(Debug, serde::Deserialize, PartialEq)]
        struct Data {
            ok: bool,
        }
        let envelope: CloudEnvelope<Data> = parse_success_envelope(&response).expect("parse");
        assert!(envelope.data.ok);
        assert_eq!(envelope.trace_id.as_deref(), Some("req_1"));
    }

    #[test]
    fn parses_problem_details() {
        let body = br#"{"title":"Unauthorized","status":401,"code":"UNAUTHORIZED","traceId":"t1"}"#
            .to_vec();
        let response = CloudRawResponse {
            status: 401,
            body,
            request_id: Some("hdr".into()),
        };
        let error = parse_problem(&response);
        assert_eq!(error.kind, CloudApiErrorKind::Unauthorized);
        assert_eq!(error.code.as_deref(), Some("UNAUTHORIZED"));
        assert_eq!(error.trace_id.as_deref(), Some("t1"));
    }

    #[test]
    fn retries_get_after_401_when_refresh_succeeds() {
        let transport = Arc::new(ScriptedTransport::new(vec![
            CloudRawResponse {
                status: 401,
                body: br#"{"code":"UNAUTHORIZED","title":"Unauthorized"}"#.to_vec(),
                request_id: None,
            },
            CloudRawResponse {
                status: 200,
                body: br#"{"data":{"value":7}}"#.to_vec(),
                request_id: None,
            },
        ]));
        let credentials = Arc::new(FakeCredentials {
            token: Mutex::new("access-old".into()),
            refresh_calls: AtomicUsize::new(0),
            expiring: false,
            refresh_ok: true,
        });
        let client = CloudClient::new(
            config(),
            transport.clone(),
            Some(credentials.clone()),
            Arc::new(FixedClock { now_ms: 1 }),
        );

        #[derive(Debug, serde::Deserialize)]
        struct Data {
            value: u32,
        }
        let envelope: CloudEnvelope<Data> = client
            .get_json("/v1/me", CloudAuthMode::Authenticated)
            .expect("get");
        assert_eq!(envelope.data.value, 7);
        assert_eq!(transport.calls.load(Ordering::SeqCst), 2);
        assert_eq!(credentials.refresh_calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            transport.last_auth.lock().expect("lock").as_deref(),
            Some("Bearer access-refreshed")
        );
    }

    #[test]
    fn does_not_retry_write_without_idempotency_key() {
        let transport = Arc::new(ScriptedTransport::new(vec![CloudRawResponse {
            status: 401,
            body: br#"{"code":"UNAUTHORIZED","title":"Unauthorized"}"#.to_vec(),
            request_id: None,
        }]));
        let credentials = Arc::new(FakeCredentials {
            token: Mutex::new("access-old".into()),
            refresh_calls: AtomicUsize::new(0),
            expiring: false,
            refresh_ok: true,
        });
        let client = CloudClient::new(
            config(),
            transport.clone(),
            Some(credentials.clone()),
            Arc::new(FixedClock { now_ms: 1 }),
        );

        let result = client.post_json::<_, Value>(
            "/v1/sync/daily-usage",
            &serde_json::json!({}),
            CloudAuthMode::Authenticated,
            None,
        );
        assert!(result.is_err());
        assert_eq!(transport.calls.load(Ordering::SeqCst), 1);
        assert_eq!(credentials.refresh_calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn retries_write_with_idempotency_key_after_refresh() {
        let transport = Arc::new(ScriptedTransport::new(vec![
            CloudRawResponse {
                status: 401,
                body: br#"{"code":"UNAUTHORIZED","title":"Unauthorized"}"#.to_vec(),
                request_id: None,
            },
            CloudRawResponse {
                status: 200,
                body: br#"{"data":{"accepted":true}}"#.to_vec(),
                request_id: None,
            },
        ]));
        let credentials = Arc::new(FakeCredentials {
            token: Mutex::new("access-old".into()),
            refresh_calls: AtomicUsize::new(0),
            expiring: false,
            refresh_ok: true,
        });
        let client = CloudClient::new(
            config(),
            transport.clone(),
            Some(credentials.clone()),
            Arc::new(FixedClock { now_ms: 1 }),
        );

        #[derive(Debug, serde::Deserialize)]
        struct Data {
            accepted: bool,
        }
        let envelope: CloudEnvelope<Data> = client
            .post_json(
                "/v1/sync/daily-usage",
                &serde_json::json!({}),
                CloudAuthMode::Authenticated,
                Some("idem-1".into()),
            )
            .expect("post");
        assert!(envelope.data.accepted);
        assert_eq!(transport.calls.load(Ordering::SeqCst), 2);
        assert_eq!(credentials.refresh_calls.load(Ordering::SeqCst), 1);
    }
}
