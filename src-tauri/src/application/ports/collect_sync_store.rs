//! Durable collect-sync state and outbox port.

#![allow(
    dead_code,
    reason = "Port surface is for collect-sync composition across phase chunks"
)]

use thiserror::Error;

use crate::application::collect_sync::{
    BatchBuildError, BatchRequestMeta, PreparedBatch, UploadScope, WireUploadScope,
};

/// Account + install identity for collect-sync isolation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct CollectSyncAccountKey {
    pub user_id: String,
    pub client_device_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BaselineStatus {
    None,
    InProgress,
    Complete,
}

impl BaselineStatus {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::InProgress => "in_progress",
            Self::Complete => "complete",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "none" => Some(Self::None),
            "in_progress" => Some(Self::InProgress),
            "complete" => Some(Self::Complete),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CollectSyncState {
    pub account: CollectSyncAccountKey,
    pub next_client_revision: i64,
    pub baseline_status: BaselineStatus,
    pub pending_scope: Option<UploadScope>,
    pub active_generation_id: Option<String>,
    pub last_attempt_at_ms: Option<i64>,
    pub last_accepted_at_ms: Option<i64>,
    pub last_error_code: Option<String>,
    pub last_error_message: Option<String>,
    pub last_error_retryable: Option<bool>,
    pub device_metadata_fingerprint: Option<String>,
    pub device_registered_revision: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OutboxBatchStatus {
    Pending,
    Accepted,
}

impl OutboxBatchStatus {
    #[allow(dead_code)] // used by later status/IPC mapping
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Accepted => "accepted",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "pending" => Some(Self::Pending),
            "accepted" => Some(Self::Accepted),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OutboxBatch {
    pub id: i64,
    pub account: CollectSyncAccountKey,
    pub generation_id: String,
    pub batch_index: u32,
    pub batch_count: u32,
    pub client_revision: i64,
    pub idempotency_key: String,
    pub request_body: String,
    pub payload_hash: String,
    pub window_scope: WireUploadScope,
    pub window_start: String,
    pub window_end: String,
    pub status: OutboxBatchStatus,
    pub created_at_ms: i64,
    pub accepted_at_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CreateGenerationInput {
    pub account: CollectSyncAccountKey,
    pub generation_id: String,
    pub meta: BatchRequestMeta,
    pub prepared_batches: Vec<PreparedBatch>,
    pub now_ms: i64,
    /// When true, marks baseline as in progress (full first upload).
    pub marks_baseline_in_progress: bool,
    /// Clear durable pending scope after materializing this generation.
    pub clear_pending_scope: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CreateGenerationResult {
    pub state: CollectSyncState,
    pub batches: Vec<OutboxBatch>,
}

#[allow(dead_code)] // full surface is for chunk 01+; later orchestration uses all methods
pub(crate) trait CollectSyncStore: Send + Sync {
    fn load_state(
        &self,
        account: &CollectSyncAccountKey,
    ) -> Result<Option<CollectSyncState>, CollectSyncStoreError>;

    fn ensure_state(
        &self,
        account: &CollectSyncAccountKey,
        now_ms: i64,
    ) -> Result<CollectSyncState, CollectSyncStoreError>;

    /// Merge `scope` into durable pending scope for the account/device.
    fn merge_pending_scope(
        &self,
        account: &CollectSyncAccountKey,
        scope: UploadScope,
        now_ms: i64,
    ) -> Result<UploadScope, CollectSyncStoreError>;

    /// Persist an immutable outbox generation. Fails if pending batches exist.
    fn create_generation(
        &self,
        input: CreateGenerationInput,
    ) -> Result<CreateGenerationResult, CollectSyncStoreError>;

    fn list_pending_batches(
        &self,
        account: &CollectSyncAccountKey,
    ) -> Result<Vec<OutboxBatch>, CollectSyncStoreError>;

    fn mark_batch_accepted(
        &self,
        account: &CollectSyncAccountKey,
        batch_id: i64,
        accepted_at_ms: i64,
    ) -> Result<OutboxBatch, CollectSyncStoreError>;

    fn count_pending_batches(
        &self,
        account: &CollectSyncAccountKey,
    ) -> Result<u32, CollectSyncStoreError>;
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub(crate) enum CollectSyncStoreError {
    #[error("collect sync state was not found")]
    NotFound,
    #[error("a pending outbox generation already exists")]
    PendingGenerationExists,
    #[error("prepared batch client revisions do not match allocated revisions")]
    RevisionMismatch,
    #[error("batch construction failed")]
    BatchBuild(#[from] BatchBuildError),
    #[error("invalid stored collect sync state")]
    InvalidState,
    #[error("the collect sync store backend failed")]
    Backend,
}
