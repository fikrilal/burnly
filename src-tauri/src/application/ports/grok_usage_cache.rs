use chrono::{DateTime, Utc};
use thiserror::Error;

use crate::application::collection::CollectionScope;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CachedGrokUsageRecord {
    pub(crate) session_id: String,
    pub(crate) observed_at: DateTime<Utc>,
    pub(crate) loop_index: u32,
    pub(crate) pid: u64,
    pub(crate) raw_model_id: String,
    pub(crate) model_display_name: Option<String>,
    pub(crate) project_path: Option<String>,
    pub(crate) prompt_tokens: u64,
    pub(crate) cached_prompt_tokens: u64,
    pub(crate) completion_tokens: u64,
    pub(crate) reasoning_tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GrokUsageCacheUpsert {
    pub(crate) record: CachedGrokUsageRecord,
    pub(crate) collector_version: String,
    pub(crate) log_offset: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GrokUnifiedLogCheckpoint {
    pub(crate) file_inode: Option<u64>,
    pub(crate) file_size: u64,
    pub(crate) byte_offset: u64,
}

#[derive(Debug, Error)]
pub(crate) enum GrokUsageCacheError {
    #[error("grok usage cache storage failed")]
    Storage,
    #[error("grok usage cache scope is invalid")]
    InvalidScope,
}

pub(crate) trait GrokUsageCache: Send + Sync {
    fn upsert(&self, records: &[GrokUsageCacheUpsert]) -> Result<(), GrokUsageCacheError>;

    fn read_for_scope(
        &self,
        scope: &CollectionScope,
        aggregation_timezone: &str,
        session_ids: &[&str],
    ) -> Result<Vec<CachedGrokUsageRecord>, GrokUsageCacheError>;

    fn read_checkpoint(&self) -> Result<Option<GrokUnifiedLogCheckpoint>, GrokUsageCacheError>;

    fn write_checkpoint(
        &self,
        checkpoint: GrokUnifiedLogCheckpoint,
    ) -> Result<(), GrokUsageCacheError>;
}
