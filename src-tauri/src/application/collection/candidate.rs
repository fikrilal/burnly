use chrono::{DateTime, NaiveDate, Utc};

use crate::domain::source::SourceKey;
use crate::domain::usage::{DataQuality, TokenUsage, UsageCost};

use super::{CollectionId, CollectorKey};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CandidateProvenance {
    pub source: SourceKey,
    pub collector: CollectorKey,
    pub collector_version: String,
    pub profile_version: u16,
    pub collection_id: CollectionId,
    pub observed_at: DateTime<Utc>,
    pub data_quality: DataQuality,
    pub warnings: Vec<CandidateWarning>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CandidateWarning {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DailyUsageCandidate {
    pub provenance: CandidateProvenance,
    pub source_key: String,
    pub usage_date: NaiveDate,
    pub aggregation_timezone: String,
    pub tokens: TokenUsage,
    pub cost: UsageCost,
    pub model_breakdowns: Vec<ModelUsageCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ModelUsageCandidate {
    pub raw_model_id: String,
    pub tokens: TokenUsage,
    pub cost: UsageCost,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionUsageCandidate {
    pub provenance: CandidateProvenance,
    pub source_key: String,
    pub source_session_id: String,
    pub project_path: Option<String>,
    pub first_activity_at: Option<DateTime<Utc>>,
    pub last_activity_at: Option<DateTime<Utc>>,
    pub tokens: TokenUsage,
    pub cost: UsageCost,
    pub model_breakdowns: Vec<ModelUsageCandidate>,
}
