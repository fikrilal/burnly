//! Wire-ready daily usage push DTOs (contract version 1).
//!
//! Serialized bodies are stored immutably in the outbox; field names must match
//! burnly-api OpenAPI camelCase.

use serde::{Deserialize, Serialize};

use super::scope::UploadScope;

pub(crate) const COLLECT_CONTRACT_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum WireUploadScope {
    Full,
    Incremental,
}

impl From<&UploadScope> for WireUploadScope {
    fn from(value: &UploadScope) -> Self {
        match value {
            UploadScope::Full => Self::Full,
            UploadScope::Incremental { .. } => Self::Incremental,
        }
    }
}

impl WireUploadScope {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Incremental => "incremental",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DailyUsagePushRequestDto {
    pub contract_version: u32,
    pub client_device_id: String,
    pub app_version: String,
    pub reporting_timezone: String,
    pub client_revision: i64,
    pub window: DailyUsageWindowDto,
    pub facts: Vec<DailyUsageFactDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DailyUsageWindowDto {
    pub start_date: String,
    pub end_date: String,
    pub scope: WireUploadScope,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DailyUsageFactDto {
    pub identity_key: String,
    pub identity_version: u16,
    pub source_key: String,
    pub usage_date: String,
    pub aggregation_timezone: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_creation_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read_tokens: Option<u64>,
    pub total_tokens: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unclassified_tokens: Option<u64>,
    pub cost: DailyUsageCostDto,
    pub data_quality: String,
    pub record_state: String,
    pub first_seen_at: String,
    pub last_seen_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub removed_at: Option<String>,
    pub models: Vec<DailyUsageModelDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DailyUsageCostDto {
    pub status: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount_micros: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DailyUsageModelDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_model_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_creation_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_tokens: Option<u64>,
    pub cost: ModelUsageCostDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModelUsageCostDto {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount_micros: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_canonical_camel_case_keys() {
        let request = DailyUsagePushRequestDto {
            contract_version: COLLECT_CONTRACT_VERSION,
            client_device_id: "dev_1".to_owned(),
            app_version: "0.1.20".to_owned(),
            reporting_timezone: "UTC".to_owned(),
            client_revision: 1,
            window: DailyUsageWindowDto {
                start_date: "2026-07-08".to_owned(),
                end_date: "2026-07-08".to_owned(),
                scope: WireUploadScope::Full,
            },
            facts: vec![DailyUsageFactDto {
                identity_key: "claude-code:daily:v1:UTC:2026-07-08".to_owned(),
                identity_version: 1,
                source_key: "claude-code".to_owned(),
                usage_date: "2026-07-08".to_owned(),
                aggregation_timezone: "UTC".to_owned(),
                input_tokens: Some(100),
                output_tokens: Some(50),
                cache_creation_tokens: Some(0),
                cache_read_tokens: Some(0),
                total_tokens: 150,
                unclassified_tokens: Some(0),
                cost: DailyUsageCostDto {
                    status: "unavailable".to_owned(),
                    kind: "unknown".to_owned(),
                    amount_micros: None,
                    currency: None,
                },
                data_quality: "complete".to_owned(),
                record_state: "active".to_owned(),
                first_seen_at: "2026-07-08T10:00:00.000Z".to_owned(),
                last_seen_at: "2026-07-08T12:00:00.000Z".to_owned(),
                removed_at: None,
                models: vec![],
            }],
        };

        let value = serde_json::to_value(&request).expect("json");
        let object = value.as_object().expect("object");
        assert!(object.contains_key("contractVersion"));
        assert!(object.contains_key("clientDeviceId"));
        assert!(object.contains_key("clientRevision"));
        assert!(object.contains_key("reportingTimezone"));
        assert_eq!(value["window"]["scope"], "full");
        assert_eq!(value["facts"][0]["identityKey"], request.facts[0].identity_key);
        assert!(value["facts"][0].get("projectId").is_none());
        assert!(value["facts"][0].get("project_id").is_none());
        assert!(value["facts"][0].get("sourceSessionId").is_none());
    }
}
