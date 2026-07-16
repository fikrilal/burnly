//! Collect-sync IPC — secret-free upload status and manual retry.

use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::application::collect_sync::{
    CollectSync, CollectSyncStatusSink, CollectSyncStatusSnapshot, CollectSyncUiStatus,
};

use super::events::{names as event_names, CollectSyncChangedEvent};
use super::response::{ErrorCategory, IpcError, IpcResponse};

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct CollectSyncStatusResponse {
    status: &'static str,
    last_accepted_at: Option<String>,
    last_error_code: Option<String>,
    last_error_message: Option<String>,
    last_error_retryable: Option<bool>,
}

impl From<CollectSyncStatusSnapshot> for CollectSyncStatusResponse {
    fn from(value: CollectSyncStatusSnapshot) -> Self {
        let status = match value.status {
            CollectSyncUiStatus::SignedOut => "signed_out",
            CollectSyncUiStatus::Idle => "idle",
            CollectSyncUiStatus::Syncing => "syncing",
            CollectSyncUiStatus::Error => "error",
        };
        Self {
            status,
            last_accepted_at: value.last_accepted_at_ms.and_then(epoch_ms_to_rfc3339),
            last_error_code: value.last_error_code,
            last_error_message: value.last_error_message,
            last_error_retryable: value.last_error_retryable,
        }
    }
}

fn epoch_ms_to_rfc3339(ms: i64) -> Option<String> {
    chrono::DateTime::from_timestamp_millis(ms)
        .map(|dt| dt.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
}

fn unavailable_status() -> CollectSyncStatusResponse {
    CollectSyncStatusResponse {
        status: "signed_out",
        last_accepted_at: None,
        last_error_code: None,
        last_error_message: None,
        last_error_retryable: None,
    }
}

#[tauri::command]
pub(super) fn collect_sync_get_status<R: Runtime>(
    app: AppHandle<R>,
) -> IpcResponse<CollectSyncStatusResponse> {
    match app.try_state::<Arc<CollectSync>>() {
        Some(service) => IpcResponse::success(service.status_snapshot().into()),
        None => IpcResponse::success(unavailable_status()),
    }
}

#[tauri::command]
pub(super) fn collect_sync_retry<R: Runtime>(
    app: AppHandle<R>,
) -> IpcResponse<CollectSyncStatusResponse> {
    match app.try_state::<Arc<CollectSync>>() {
        Some(service) => {
            service.retry_now();
            IpcResponse::success(service.status_snapshot().into())
        }
        None => IpcResponse::failure(IpcError::new(
            "collect_sync.unavailable",
            "Cloud upload is not available right now.",
            ErrorCategory::Unavailable,
            false,
        )),
    }
}

/// Emits collect-sync status changes to the frontend (lossy invalidation).
pub(crate) struct CollectSyncEventSink<R: Runtime> {
    app: AppHandle<R>,
}

impl<R: Runtime> CollectSyncEventSink<R> {
    pub(crate) fn new(app: AppHandle<R>) -> Self {
        Self { app }
    }
}

impl<R: Runtime> CollectSyncStatusSink for CollectSyncEventSink<R> {
    fn on_status_changed(&self, snapshot: CollectSyncStatusSnapshot) {
        let status = match snapshot.status {
            CollectSyncUiStatus::SignedOut => "signed_out",
            CollectSyncUiStatus::Idle => "idle",
            CollectSyncUiStatus::Syncing => "syncing",
            CollectSyncUiStatus::Error => "error",
        };
        let _ = self.app.emit(
            event_names::COLLECT_SYNC_CHANGED,
            CollectSyncChangedEvent { status },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_signed_in_idle_status_without_secrets() {
        let response = CollectSyncStatusResponse::from(CollectSyncStatusSnapshot {
            status: CollectSyncUiStatus::Idle,
            last_accepted_at_ms: Some(1_720_483_200_000),
            last_error_code: None,
            last_error_message: None,
            last_error_retryable: None,
        });
        assert_eq!(response.status, "idle");
        assert!(response.last_accepted_at.is_some());
        let json = serde_json::to_value(&response).expect("json");
        assert!(json.get("accessToken").is_none());
        assert!(json.get("idempotencyKey").is_none());
        assert!(json.get("requestBody").is_none());
        assert!(json.get("clientRevision").is_none());
    }

    #[test]
    fn maps_error_retryable_flag() {
        let response = CollectSyncStatusResponse::from(CollectSyncStatusSnapshot {
            status: CollectSyncUiStatus::Error,
            last_accepted_at_ms: None,
            last_error_code: Some("NETWORK".into()),
            last_error_message: Some("down".into()),
            last_error_retryable: Some(true),
        });
        assert_eq!(response.status, "error");
        assert_eq!(response.last_error_retryable, Some(true));
    }
}
