//! `POST /v1/sync/daily-usage` adapter (immutable outbox body).

use std::sync::Arc;

use serde::Deserialize;

use crate::application::collect_sync::WireUploadScope;
use crate::application::ports::collect_sync_remote::{
    CollectSyncRemoteError, DailyUsagePushCounts, DailyUsagePushResult, PushDailyUsageRequest,
};

use super::client::{CloudAuthMode, CloudClient};
use super::collect_sync_error_map::map_cloud_api_error;
use super::sync_device::HttpSyncDeviceClient;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PushDataBody {
    client_device_id: String,
    accepted_at: String,
    client_revision: i64,
    window: PushWindowBody,
    counts: PushCountsBody,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PushWindowBody {
    start_date: String,
    end_date: String,
    scope: ResponseScopeBody,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
enum ResponseScopeBody {
    Full,
    Incremental,
    /// Deprecated backend compatibility only.
    Rolling,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PushCountsBody {
    received: u32,
    upserted: u32,
    removed: u32,
    unchanged: u32,
    rejected: u32,
}

pub(crate) struct HttpDailyUsagePushClient {
    client: Arc<CloudClient>,
}

impl HttpDailyUsagePushClient {
    pub(crate) fn new(client: Arc<CloudClient>) -> Self {
        Self { client }
    }

    pub(crate) fn push_daily_usage(
        &self,
        request: PushDailyUsageRequest,
    ) -> Result<DailyUsagePushResult, CollectSyncRemoteError> {
        if request.idempotency_key.trim().is_empty() {
            return Err(CollectSyncRemoteError::Validation {
                code: Some("VALIDATION_FAILED".into()),
                message: "idempotency key is required".into(),
                field_errors: Vec::new(),
            });
        }
        if request.request_body.trim().is_empty() {
            return Err(CollectSyncRemoteError::Validation {
                code: Some("VALIDATION_FAILED".into()),
                message: "request body is required".into(),
                field_errors: Vec::new(),
            });
        }

        let envelope = self
            .client
            .post_raw_json::<PushDataBody>(
                "/v1/sync/daily-usage",
                request.request_body.as_bytes(),
                CloudAuthMode::Authenticated,
                Some(request.idempotency_key),
            )
            .map_err(map_cloud_api_error)?;

        Ok(DailyUsagePushResult {
            client_device_id: envelope.data.client_device_id,
            accepted_at: envelope.data.accepted_at,
            client_revision: envelope.data.client_revision,
            window_start: envelope.data.window.start_date,
            window_end: envelope.data.window.end_date,
            window_scope: match envelope.data.window.scope {
                ResponseScopeBody::Full => WireUploadScope::Full,
                ResponseScopeBody::Incremental | ResponseScopeBody::Rolling => {
                    WireUploadScope::Incremental
                }
            },
            counts: DailyUsagePushCounts {
                received: envelope.data.counts.received,
                upserted: envelope.data.counts.upserted,
                removed: envelope.data.counts.removed,
                unchanged: envelope.data.counts.unchanged,
                rejected: envelope.data.counts.rejected,
            },
        })
    }
}

/// Combined remote implementing device upsert + daily push.
pub(crate) struct HttpCollectSyncRemote {
    devices: HttpSyncDeviceClient,
    push: HttpDailyUsagePushClient,
}

impl HttpCollectSyncRemote {
    pub(crate) fn new(client: Arc<CloudClient>) -> Self {
        Self {
            devices: HttpSyncDeviceClient::new(client.clone()),
            push: HttpDailyUsagePushClient::new(client),
        }
    }
}

impl crate::application::ports::collect_sync_remote::CollectSyncRemote for HttpCollectSyncRemote {
    fn upsert_device(
        &self,
        request: crate::application::ports::collect_sync_remote::UpsertSyncDeviceRequest,
    ) -> Result<
        crate::application::ports::collect_sync_remote::SyncDeviceSnapshot,
        CollectSyncRemoteError,
    > {
        self.devices.upsert_device(request)
    }

    fn push_daily_usage(
        &self,
        request: PushDailyUsageRequest,
    ) -> Result<DailyUsagePushResult, CollectSyncRemoteError> {
        self.push.push_daily_usage(request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::cloud_session::CloudSessionError;
    use crate::application::ports::cloud_auth_credentials::CloudAuthCredentials;
    use crate::application::ports::clock::Clock;
    use crate::application::ports::collect_sync_remote::CollectSyncRemote;
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

    struct RecordedCall {
        url: String,
        method: CloudHttpMethod,
        body: Option<Vec<u8>>,
        headers: Vec<(String, String)>,
    }

    struct ScriptedTransport {
        responses: Mutex<Vec<CloudRawResponse>>,
        calls: Mutex<Vec<RecordedCall>>,
        call_count: AtomicUsize,
    }

    impl ScriptedTransport {
        fn new(responses: Vec<CloudRawResponse>) -> Self {
            Self {
                responses: Mutex::new(responses),
                calls: Mutex::new(Vec::new()),
                call_count: AtomicUsize::new(0),
            }
        }
    }

    impl CloudHttpTransport for ScriptedTransport {
        fn send(
            &self,
            url: &str,
            method: CloudHttpMethod,
            headers: &[(String, String)],
            body: Option<&[u8]>,
        ) -> Result<CloudRawResponse, super::super::error::CloudApiError> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            self.calls.lock().expect("lock").push(RecordedCall {
                url: url.to_owned(),
                method,
                body: body.map(ToOwned::to_owned),
                headers: headers.to_vec(),
            });
            let mut responses = self.responses.lock().expect("lock");
            if responses.is_empty() {
                return Err(super::super::error::CloudApiError::internal(
                    "no scripted response",
                ));
            }
            Ok(responses.remove(0))
        }
    }

    struct StaticCredentials {
        token: Mutex<String>,
        refresh_ok: bool,
        refresh_calls: AtomicUsize,
    }

    impl CloudAuthCredentials for StaticCredentials {
        fn access_token(&self) -> Option<String> {
            Some(self.token.lock().expect("lock").clone())
        }
        fn is_access_expiring_soon(&self, _: i64, _: i64) -> bool {
            false
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

    fn client(
        transport: Arc<ScriptedTransport>,
        credentials: Arc<StaticCredentials>,
    ) -> Arc<CloudClient> {
        Arc::new(CloudClient::new(
            CloudConfig::new(
                "http://127.0.0.1:4000",
                "http://127.0.0.1:3000",
                "http://127.0.0.1:39201/callback",
                "0.1.20",
            )
            .expect("config"),
            transport,
            Some(credentials),
            Arc::new(FixedClock),
        ))
    }

    #[test]
    fn push_sends_exact_body_and_idempotency_key() {
        let exact_body = r#"{"contractVersion":1,"clientDeviceId":"dev_1","facts":[]}"#;
        let transport = Arc::new(ScriptedTransport::new(vec![CloudRawResponse {
            status: 200,
            body: br#"{"data":{"clientDeviceId":"dev_1","acceptedAt":"2026-07-09T12:00:00.000Z","clientRevision":1,"window":{"startDate":"2026-07-08","endDate":"2026-07-08","scope":"full"},"counts":{"received":0,"upserted":0,"removed":0,"unchanged":0,"rejected":0}}}"#.to_vec(),
            request_id: None,
            retry_after_seconds: None,
        }]));
        let credentials = Arc::new(StaticCredentials {
            token: Mutex::new("access".into()),
            refresh_ok: true,
            refresh_calls: AtomicUsize::new(0),
        });
        let remote = HttpCollectSyncRemote::new(client(transport.clone(), credentials));
        let result = remote
            .push_daily_usage(PushDailyUsageRequest {
                request_body: exact_body.to_owned(),
                idempotency_key: "idem-1".into(),
            })
            .expect("push");

        assert_eq!(result.client_revision, 1);
        assert_eq!(result.window_scope, WireUploadScope::Full);
        let calls = transport.calls.lock().expect("lock");
        assert_eq!(calls.len(), 1);
        assert!(calls[0].url.ends_with("/v1/sync/daily-usage"));
        assert_eq!(calls[0].method, CloudHttpMethod::Post);
        assert_eq!(
            String::from_utf8(calls[0].body.clone().expect("body")).expect("utf8"),
            exact_body
        );
        assert!(calls[0]
            .headers
            .iter()
            .any(|(name, value)| name == "Idempotency-Key" && value == "idem-1"));
        assert!(calls[0]
            .headers
            .iter()
            .any(|(name, value)| name == "Authorization" && value == "Bearer access"));
    }

    #[test]
    fn maps_device_not_found_and_does_not_create_device_via_push() {
        let transport = Arc::new(ScriptedTransport::new(vec![CloudRawResponse {
            status: 404,
            body: br#"{"code":"SYNC_DEVICE_NOT_FOUND","title":"Device not found"}"#.to_vec(),
            request_id: None,
            retry_after_seconds: None,
        }]));
        let credentials = Arc::new(StaticCredentials {
            token: Mutex::new("access".into()),
            refresh_ok: true,
            refresh_calls: AtomicUsize::new(0),
        });
        let remote = HttpCollectSyncRemote::new(client(transport.clone(), credentials));
        let error = remote
            .push_daily_usage(PushDailyUsageRequest {
                request_body: r#"{"contractVersion":1}"#.into(),
                idempotency_key: "idem-1".into(),
            })
            .expect_err("device missing");
        assert!(matches!(error, CollectSyncRemoteError::DeviceNotFound { .. }));
        // Only the daily-usage path was called; no implicit device PUT.
        let calls = transport.calls.lock().expect("lock");
        assert_eq!(calls.len(), 1);
        assert!(calls[0].url.contains("/v1/sync/daily-usage"));
    }

    #[test]
    fn accepts_deprecated_rolling_scope_in_response() {
        let transport = Arc::new(ScriptedTransport::new(vec![CloudRawResponse {
            status: 200,
            body: br#"{"data":{"clientDeviceId":"dev_1","acceptedAt":"2026-07-09T12:00:00.000Z","clientRevision":2,"window":{"startDate":"2026-07-08","endDate":"2026-07-08","scope":"rolling"},"counts":{"received":1,"upserted":1,"removed":0,"unchanged":0,"rejected":0}}}"#.to_vec(),
            request_id: None,
            retry_after_seconds: None,
        }]));
        let credentials = Arc::new(StaticCredentials {
            token: Mutex::new("access".into()),
            refresh_ok: true,
            refresh_calls: AtomicUsize::new(0),
        });
        let remote = HttpCollectSyncRemote::new(client(transport, credentials));
        let result = remote
            .push_daily_usage(PushDailyUsageRequest {
                request_body: r#"{"contractVersion":1}"#.into(),
                idempotency_key: "idem-1".into(),
            })
            .expect("push");
        assert_eq!(result.window_scope, WireUploadScope::Incremental);
    }

    #[test]
    fn maps_rate_limited_with_retry_after() {
        let transport = Arc::new(ScriptedTransport::new(vec![CloudRawResponse {
            status: 429,
            body: br#"{"code":"RATE_LIMITED","title":"Too many requests"}"#.to_vec(),
            request_id: None,
            retry_after_seconds: Some(30),
        }]));
        let credentials = Arc::new(StaticCredentials {
            token: Mutex::new("access".into()),
            refresh_ok: true,
            refresh_calls: AtomicUsize::new(0),
        });
        let remote = HttpCollectSyncRemote::new(client(transport, credentials));
        let error = remote
            .push_daily_usage(PushDailyUsageRequest {
                request_body: r#"{"contractVersion":1}"#.into(),
                idempotency_key: "idem-1".into(),
            })
            .expect_err("rate limited");
        match error {
            CollectSyncRemoteError::RateLimited {
                retry_after_seconds,
                ..
            } => assert_eq!(retry_after_seconds, Some(30)),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn retries_push_once_after_401_when_idempotency_key_present() {
        let transport = Arc::new(ScriptedTransport::new(vec![
            CloudRawResponse {
                status: 401,
                body: br#"{"code":"UNAUTHORIZED","title":"Unauthorized"}"#.to_vec(),
                request_id: None,
                retry_after_seconds: None,
            },
            CloudRawResponse {
                status: 200,
                body: br#"{"data":{"clientDeviceId":"dev_1","acceptedAt":"2026-07-09T12:00:00.000Z","clientRevision":1,"window":{"startDate":"2026-07-08","endDate":"2026-07-08","scope":"incremental"},"counts":{"received":0,"upserted":0,"removed":0,"unchanged":0,"rejected":0}}}"#.to_vec(),
                request_id: None,
                retry_after_seconds: None,
            },
        ]));
        let credentials = Arc::new(StaticCredentials {
            token: Mutex::new("access-old".into()),
            refresh_ok: true,
            refresh_calls: AtomicUsize::new(0),
        });
        let remote = HttpCollectSyncRemote::new(client(transport.clone(), credentials.clone()));
        let result = remote
            .push_daily_usage(PushDailyUsageRequest {
                request_body: r#"{"contractVersion":1}"#.into(),
                idempotency_key: "idem-1".into(),
            })
            .expect("push after refresh");
        assert_eq!(result.client_revision, 1);
        assert_eq!(transport.call_count.load(Ordering::SeqCst), 2);
        assert_eq!(credentials.refresh_calls.load(Ordering::SeqCst), 1);
    }
}
