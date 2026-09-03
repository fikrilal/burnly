use chrono::{DateTime, Utc};
use thiserror::Error;

use crate::application::collection::CollectionScope;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AntigravityTimestampOrigin {
    SourceReported,
    FirstSeen,
    LegacyUnknown,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AntigravityCalendarAttribution {
    Dated,
    UndatedBaseline,
}

impl AntigravityCalendarAttribution {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Dated => "dated",
            Self::UndatedBaseline => "undated_baseline",
        }
    }

    pub(crate) fn from_str(s: &str) -> Option<Self> {
        match s {
            "dated" => Some(Self::Dated),
            "undated_baseline" => Some(Self::UndatedBaseline),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CachedAntigravityUsageRecord {
    pub(crate) variant: String,
    pub(crate) conversation_id: String,
    pub(crate) response_id: Option<String>,
    pub(crate) raw_model_id: String,
    pub(crate) model_label: String,
    pub(crate) api_provider: Option<String>,
    pub(crate) source_record_index: Option<i64>,
    pub(crate) input_tokens: u64,
    pub(crate) output_tokens: u64,
    pub(crate) thinking_output_tokens: u64,
    pub(crate) response_output_tokens: u64,
    pub(crate) cache_read_tokens: u64,
    pub(crate) cache_write_tokens: u64,
    pub(crate) observed_at: Option<DateTime<Utc>>,
    pub(crate) timestamp_origin: AntigravityTimestampOrigin,
    pub(crate) calendar_attribution: AntigravityCalendarAttribution,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AntigravityUsageCacheUpsert {
    pub(crate) record: CachedAntigravityUsageRecord,
    pub(crate) legacy_fallback_at: Option<DateTime<Utc>>,
    pub(crate) collector_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AntigravityUsageCacheReconcileResult {
    pub(crate) records: Vec<CachedAntigravityUsageRecord>,
    pub(crate) legacy_records_repaired: u32,
}

#[derive(Debug, Error)]
pub(crate) enum AntigravityUsageCacheError {
    #[error("antigravity usage cache storage failed")]
    Storage,
    #[error("antigravity usage cache scope is invalid")]
    InvalidScope,
}

pub(crate) trait AntigravityUsageCache: Send + Sync {
    fn reconcile(
        &self,
        records: &[AntigravityUsageCacheUpsert],
        collected_at: DateTime<Utc>,
    ) -> Result<AntigravityUsageCacheReconcileResult, AntigravityUsageCacheError>;

    fn read_for_scope(
        &self,
        scope: &CollectionScope,
        aggregation_timezone: &str,
        conversations: &[(&str, &str)],
    ) -> Result<Vec<CachedAntigravityUsageRecord>, AntigravityUsageCacheError>;
}
