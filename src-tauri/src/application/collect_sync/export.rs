//! Exported daily facts from local SQLite (allowlisted fields only).

use chrono::{SecondsFormat, TimeZone, Utc};

use super::dto::{
    DailyUsageCostDto, DailyUsageFactDto, DailyUsageModelDto, ModelUsageCostDto,
};

/// One daily usage parent row plus model children, ready for wire mapping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExportedDailyFact {
    pub identity_key: String,
    pub identity_version: u16,
    pub source_key: String,
    pub usage_date: String,
    pub aggregation_timezone: String,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_creation_tokens: Option<u64>,
    pub cache_read_tokens: Option<u64>,
    pub total_tokens: u64,
    pub unclassified_tokens: Option<u64>,
    pub cost_status: String,
    pub cost_kind: String,
    pub cost_amount_micros: Option<i64>,
    pub cost_currency: Option<String>,
    pub data_quality: String,
    pub record_state: String,
    pub first_seen_at_ms: i64,
    pub last_seen_at_ms: i64,
    pub removed_at_ms: Option<i64>,
    pub models: Vec<ExportedDailyModel>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExportedDailyModel {
    pub raw_model_id: Option<String>,
    pub display_name: Option<String>,
    pub provider_key: Option<String>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_creation_tokens: Option<u64>,
    pub cache_read_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    pub cost_status: String,
    pub cost_amount_micros: Option<i64>,
    pub cost_currency: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum ExportMapError {
    #[error("invalid timestamp for export")]
    InvalidTimestamp,
}

pub(crate) fn map_exported_fact(
    fact: ExportedDailyFact,
) -> Result<DailyUsageFactDto, ExportMapError> {
    Ok(DailyUsageFactDto {
        identity_key: fact.identity_key,
        identity_version: fact.identity_version,
        source_key: fact.source_key,
        usage_date: fact.usage_date,
        aggregation_timezone: fact.aggregation_timezone,
        input_tokens: fact.input_tokens,
        output_tokens: fact.output_tokens,
        cache_creation_tokens: fact.cache_creation_tokens,
        cache_read_tokens: fact.cache_read_tokens,
        total_tokens: fact.total_tokens,
        unclassified_tokens: fact.unclassified_tokens,
        cost: DailyUsageCostDto {
            status: fact.cost_status,
            kind: fact.cost_kind,
            amount_micros: fact.cost_amount_micros,
            currency: fact.cost_currency,
        },
        data_quality: fact.data_quality,
        record_state: fact.record_state,
        first_seen_at: ms_to_rfc3339(fact.first_seen_at_ms)?,
        last_seen_at: ms_to_rfc3339(fact.last_seen_at_ms)?,
        removed_at: fact
            .removed_at_ms
            .map(ms_to_rfc3339)
            .transpose()?,
        models: fact
            .models
            .into_iter()
            .map(|model| DailyUsageModelDto {
                raw_model_id: model.raw_model_id,
                display_name: model.display_name,
                provider_key: model.provider_key,
                input_tokens: model.input_tokens,
                output_tokens: model.output_tokens,
                cache_creation_tokens: model.cache_creation_tokens,
                cache_read_tokens: model.cache_read_tokens,
                total_tokens: model.total_tokens,
                cost: ModelUsageCostDto {
                    status: model.cost_status,
                    amount_micros: model.cost_amount_micros,
                    currency: model.cost_currency,
                },
            })
            .collect(),
    })
}

fn ms_to_rfc3339(ms: i64) -> Result<String, ExportMapError> {
    Utc.timestamp_millis_opt(ms)
        .single()
        .map(|datetime| datetime.to_rfc3339_opts(SecondsFormat::Millis, true))
        .ok_or(ExportMapError::InvalidTimestamp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_timestamps_and_cost_without_forbidden_fields() {
        let dto = map_exported_fact(ExportedDailyFact {
            identity_key: "claude-code:daily:v1:UTC:2026-07-08".to_owned(),
            identity_version: 1,
            source_key: "claude-code".to_owned(),
            usage_date: "2026-07-08".to_owned(),
            aggregation_timezone: "UTC".to_owned(),
            input_tokens: Some(10),
            output_tokens: None,
            cache_creation_tokens: None,
            cache_read_tokens: None,
            total_tokens: 10,
            unclassified_tokens: None,
            cost_status: "estimated".to_owned(),
            cost_kind: "collector_calculated".to_owned(),
            cost_amount_micros: Some(100),
            cost_currency: Some("USD".to_owned()),
            data_quality: "complete".to_owned(),
            record_state: "active".to_owned(),
            first_seen_at_ms: 1_720_483_200_000,
            last_seen_at_ms: 1_720_483_260_000,
            removed_at_ms: None,
            models: vec![],
        })
        .expect("map");

        assert!(dto.first_seen_at.ends_with('Z'));
        assert!(dto.first_seen_at.contains('T'));
        assert_eq!(dto.cost.status, "estimated");
        let json = serde_json::to_value(&dto).expect("json");
        assert!(json.get("projectId").is_none());
        assert!(json.get("projectPath").is_none());
        assert!(json.get("sourceSessionId").is_none());
    }
}
