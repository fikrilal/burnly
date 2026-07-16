//! `PUT /v1/sync/devices/{clientDeviceId}` adapter.

use std::sync::Arc;

use serde::Deserialize;
use serde::Serialize;

use crate::application::ports::collect_sync_remote::{
    CollectSyncRemoteError, SyncDeviceSnapshot, UpsertSyncDeviceRequest,
};

use super::client::{CloudAuthMode, CloudClient};
use super::collect_sync_error_map::map_cloud_api_error;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpsertDeviceBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    display_name: Option<String>,
    platform: &'static str,
    app_version: String,
    reporting_timezone: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeviceDataBody {
    client_device_id: String,
    display_name: Option<String>,
    platform: String,
    app_version: String,
    reporting_timezone: String,
    last_sync_at: Option<String>,
    created_at: String,
    updated_at: String,
}

pub(crate) struct HttpSyncDeviceClient {
    client: Arc<CloudClient>,
}

impl HttpSyncDeviceClient {
    pub(crate) fn new(client: Arc<CloudClient>) -> Self {
        Self { client }
    }

    pub(crate) fn upsert_device(
        &self,
        request: UpsertSyncDeviceRequest,
    ) -> Result<SyncDeviceSnapshot, CollectSyncRemoteError> {
        if request.client_device_id.trim().is_empty() {
            return Err(CollectSyncRemoteError::Validation {
                code: Some("VALIDATION_FAILED".into()),
                message: "client device id is required".into(),
                field_errors: Vec::new(),
            });
        }
        if request.reporting_timezone.trim().is_empty() {
            return Err(CollectSyncRemoteError::Validation {
                code: Some("VALIDATION_FAILED".into()),
                message: "reporting timezone is required".into(),
                field_errors: Vec::new(),
            });
        }

        let path = format!(
            "/v1/sync/devices/{}",
            urlencoding_path_segment(&request.client_device_id)
        );
        let body = UpsertDeviceBody {
            display_name: request
                .display_name
                .filter(|value| !value.trim().is_empty()),
            platform: request.platform.as_str(),
            app_version: request.app_version,
            reporting_timezone: request.reporting_timezone,
        };

        let envelope = self
            .client
            .put_json::<_, DeviceDataBody>(&path, &body, CloudAuthMode::Authenticated, None)
            .map_err(map_cloud_api_error)?;

        Ok(SyncDeviceSnapshot {
            client_device_id: envelope.data.client_device_id,
            display_name: envelope.data.display_name,
            platform: envelope.data.platform,
            app_version: envelope.data.app_version,
            reporting_timezone: envelope.data.reporting_timezone,
            last_sync_at: envelope.data.last_sync_at,
            created_at: envelope.data.created_at,
            updated_at: envelope.data.updated_at,
        })
    }
}

/// Percent-encode a single path segment without encoding unreserved characters.
fn urlencoding_path_segment(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(char::from(byte));
            }
            _ => {
                use std::fmt::Write as _;
                let _ = write!(encoded, "%{byte:02X}");
            }
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::ports::cloud_auth_credentials::CloudAuthCredentials;
    use crate::application::ports::clock::Clock;
    use crate::application::ports::collect_sync_remote::CollectSyncPlatform;
    use crate::infrastructure::cloud::client::{
        CloudHttpMethod, CloudHttpTransport, CloudRawResponse,
    };
    use crate::infrastructure::cloud::config::CloudConfig;
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

    struct RecordingTransport {
        calls: Mutex<Vec<RecordedCall>>,
        response: CloudRawResponse,
    }

    impl CloudHttpTransport for RecordingTransport {
        fn send(
            &self,
            url: &str,
            method: CloudHttpMethod,
            headers: &[(String, String)],
            body: Option<&[u8]>,
        ) -> Result<CloudRawResponse, super::super::error::CloudApiError> {
            self.calls.lock().expect("lock").push(RecordedCall {
                url: url.to_owned(),
                method,
                body: body.map(ToOwned::to_owned),
                headers: headers.to_vec(),
            });
            Ok(self.response.clone())
        }
    }

    struct StaticCredentials;
    impl CloudAuthCredentials for StaticCredentials {
        fn access_token(&self) -> Option<String> {
            Some("access-token".into())
        }
        fn is_access_expiring_soon(&self, _: i64, _: i64) -> bool {
            false
        }
        fn refresh_single_flight(
            &self,
        ) -> Result<(), crate::application::cloud_session::CloudSessionError> {
            Ok(())
        }
    }

    #[test]
    fn upsert_device_sends_authenticated_put_with_contract_fields() {
        let transport = Arc::new(RecordingTransport {
            calls: Mutex::new(Vec::new()),
            response: CloudRawResponse {
                status: 200,
                body: br#"{"data":{"clientDeviceId":"dev_1","displayName":"host","platform":"linux","appVersion":"0.1.20","reportingTimezone":"UTC","lastSyncAt":null,"createdAt":"2026-07-09T10:00:00.000Z","updatedAt":"2026-07-09T10:00:00.000Z"}}"#.to_vec(),
                request_id: None,
                retry_after_seconds: None,
            },
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
            Some(Arc::new(StaticCredentials)),
            Arc::new(FixedClock),
        ));
        let adapter = HttpSyncDeviceClient::new(client);
        let snapshot = adapter
            .upsert_device(UpsertSyncDeviceRequest {
                client_device_id: "dev_1".into(),
                display_name: Some("host".into()),
                platform: CollectSyncPlatform::Linux,
                app_version: "0.1.20".into(),
                reporting_timezone: "UTC".into(),
            })
            .expect("upsert");

        assert_eq!(snapshot.client_device_id, "dev_1");
        let calls = transport.calls.lock().expect("lock");
        assert_eq!(calls.len(), 1);
        assert!(calls[0].url.ends_with("/v1/sync/devices/dev_1"));
        assert_eq!(calls[0].method, CloudHttpMethod::Put);
        let body: serde_json::Value =
            serde_json::from_slice(calls[0].body.as_ref().expect("body")).expect("json");
        assert_eq!(body["displayName"], "host");
        assert_eq!(body["platform"], "linux");
        assert_eq!(body["appVersion"], "0.1.20");
        assert_eq!(body["reportingTimezone"], "UTC");
        assert!(body.get("accessToken").is_none());
        assert!(calls[0]
            .headers
            .iter()
            .any(|(name, value)| name == "Authorization" && value == "Bearer access-token"));
    }
}
