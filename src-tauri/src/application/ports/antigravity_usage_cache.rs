use chrono::{DateTime, Utc};
use thiserror::Error;

use crate::application::collection::CollectionScope;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CachedAntigravityUsageRecord {
    pub(crate) variant: String,
    pub(crate) conversation_id: String,
    pub(crate) response_id: Option<String>,
    pub(crate) raw_model_id: String,
    pub(crate) model_label: String,
    pub(crate) api_provider: Option<String>,
    pub(crate) input_tokens: u64,
    pub(crate) output_tokens: u64,
    pub(crate) thinking_output_tokens: u64,
    pub(crate) response_output_tokens: u64,
    pub(crate) cache_read_tokens: u64,
    pub(crate) cache_write_tokens: u64,
    pub(crate) observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AntigravityUsageCacheUpsert {
    pub(crate) record: CachedAntigravityUsageRecord,
    pub(crate) collector_version: String,
}

#[derive(Debug, Error)]
pub(crate) enum AntigravityUsageCacheError {
    #[error("antigravity usage cache storage failed")]
    Storage,
    #[error("antigravity usage cache scope is invalid")]
    InvalidScope,
}

pub(crate) trait AntigravityUsageCache: Send + Sync {
    fn upsert(&self, records: &[AntigravityUsageCacheUpsert]) -> Result<(), AntigravityUsageCacheError>;

    fn read_for_scope(
        &self,
        scope: &CollectionScope,
        aggregation_timezone: &str,
        conversations: &[(&str, &str)],
    ) -> Result<Vec<CachedAntigravityUsageRecord>, AntigravityUsageCacheError>;
}