//! Local export mapping and durable outbox types for desktop collect sync.
//!
//! Chunk 01: pure types, batch construction, and scope merge only. Network and
//! refresh wiring arrive in later chunks.

#![allow(
    dead_code,
    reason = "Collect-sync surface is consumed by later phase chunks"
)]

mod batch;
mod dto;
mod export;
mod scope;
mod service;

#[allow(unused_imports)] // re-exported for later chunks and store adapters
pub(crate) use batch::{
    build_prepared_batches, BatchBuildError, BatchBuildLimits, BatchRequestMeta, PreparedBatch,
};
#[allow(unused_imports)]
pub(crate) use dto::{
    DailyUsageCostDto, DailyUsageFactDto, DailyUsageModelDto, DailyUsagePushRequestDto,
    DailyUsageWindowDto, ModelUsageCostDto, WireUploadScope,
};
#[allow(unused_imports)]
pub(crate) use export::{
    map_exported_fact, ExportMapError, ExportedDailyFact, ExportedDailyModel,
};
#[allow(unused_imports)]
pub(crate) use scope::{merge_upload_scopes, ScopeError, StoredUploadScope, UploadScope};
#[allow(unused_imports)]
pub(crate) use service::{
    CollectSync, CollectSyncConfig, CollectSyncStatusSink, CollectSyncStatusSnapshot,
    CollectSyncUiStatus, CommittedDailyTarget, CommittedDailyUpload, NoopCollectSyncStatusSink,
};
