//! Remote port for collect-sync device registration and daily usage push.

#![allow(
    dead_code,
    reason = "Port is consumed by collect-sync orchestration in later chunks"
)]

use thiserror::Error;

use crate::application::collect_sync::WireUploadScope;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CollectSyncPlatform {
    Linux,
    Macos,
    Windows,
}

impl CollectSyncPlatform {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Linux => "linux",
            Self::Macos => "macos",
            Self::Windows => "windows",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UpsertSyncDeviceRequest {
    pub client_device_id: String,
    pub display_name: Option<String>,
    pub platform: CollectSyncPlatform,
    pub app_version: String,
    pub reporting_timezone: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SyncDeviceSnapshot {
    pub client_device_id: String,
    pub display_name: Option<String>,
    pub platform: String,
    pub app_version: String,
    pub reporting_timezone: String,
    pub last_sync_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Exact stored outbox body + idempotency key for one push attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PushDailyUsageRequest {
    pub request_body: String,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DailyUsagePushCounts {
    pub received: u32,
    pub upserted: u32,
    pub removed: u32,
    pub unchanged: u32,
    pub rejected: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DailyUsagePushResult {
    pub client_device_id: String,
    pub accepted_at: String,
    pub client_revision: i64,
    pub window_start: String,
    pub window_end: String,
    /// Desktop only emits full/incremental; deprecated rolling is mapped to incremental.
    pub window_scope: WireUploadScope,
    pub counts: DailyUsagePushCounts,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CollectSyncFieldError {
    pub field: String,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub(crate) enum CollectSyncRemoteError {
    #[error("network error: {message}")]
    Network { message: String },
    #[error("request timed out: {message}")]
    Timeout { message: String },
    #[error("unauthorized: {message}")]
    Unauthorized {
        code: Option<String>,
        message: String,
    },
    #[error("forbidden: {message}")]
    Forbidden {
        code: Option<String>,
        message: String,
    },
    #[error("validation failed: {message}")]
    Validation {
        code: Option<String>,
        message: String,
        field_errors: Vec<CollectSyncFieldError>,
    },
    #[error("rate limited: {message}")]
    RateLimited {
        code: Option<String>,
        message: String,
        retry_after_seconds: Option<u64>,
    },
    #[error("sync device not found")]
    DeviceNotFound { message: String },
    #[error("sync contract unsupported: {message}")]
    ContractUnsupported { message: String },
    #[error("idempotency in progress: {message}")]
    IdempotencyInProgress { message: String },
    #[error("conflict: {message}")]
    Conflict {
        code: Option<String>,
        message: String,
    },
    #[error("payload too large: {message}")]
    PayloadTooLarge { message: String },
    #[error("cloud problem: {message}")]
    Problem {
        code: Option<String>,
        status: Option<u16>,
        message: String,
        trace_id: Option<String>,
    },
    #[error("decode error: {message}")]
    Decode { message: String },
    #[error("internal error: {message}")]
    Internal { message: String },
}

pub(crate) trait CollectSyncRemote: Send + Sync {
    fn upsert_device(
        &self,
        request: UpsertSyncDeviceRequest,
    ) -> Result<SyncDeviceSnapshot, CollectSyncRemoteError>;

    fn push_daily_usage(
        &self,
        request: PushDailyUsageRequest,
    ) -> Result<DailyUsagePushResult, CollectSyncRemoteError>;
}
