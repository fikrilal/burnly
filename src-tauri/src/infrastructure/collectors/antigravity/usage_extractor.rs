#![allow(
    dead_code,
    reason = "Antigravity usage extraction is introduced before collection mapping in chunk 4"
)]

use std::collections::BTreeSet;

use serde_json::{Map, Value};
use thiserror::Error;

use super::product_variant::AntigravityProductVariant;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AntigravityUsageRecord {
    pub(crate) variant: AntigravityProductVariant,
    pub(crate) conversation_id: String,
    pub(crate) raw_model_id: String,
    pub(crate) model_label: String,
    pub(crate) api_provider: Option<String>,
    pub(crate) response_id: Option<String>,
    pub(crate) input_tokens: u64,
    pub(crate) output_tokens: u64,
    pub(crate) thinking_output_tokens: u64,
    pub(crate) response_output_tokens: u64,
    pub(crate) cache_read_tokens: u64,
    pub(crate) cache_write_tokens: u64,
    pub(crate) consumed_credits: Option<String>,
    pub(crate) flow_credits_used: Option<String>,
    pub(crate) prompt_credits_used: Option<String>,
}

impl AntigravityUsageRecord {
    pub(crate) fn total_tokens(&self) -> Result<u64, UsageExtractionError> {
        checked_add_all([
            self.input_tokens,
            self.output_tokens,
            self.cache_read_tokens,
            self.cache_write_tokens,
        ])
    }
}

pub(crate) fn extract_usage_from_generator_metadata(
    variant: AntigravityProductVariant,
    conversation_id: &str,
    generator_metadata: &[Value],
) -> Result<Vec<AntigravityUsageRecord>, UsageExtractionError> {
    extract_usage_records(variant, conversation_id, generator_metadata)
}

pub(crate) fn extract_usage_records(
    variant: AntigravityProductVariant,
    conversation_id: &str,
    frames: &[Value],
) -> Result<Vec<AntigravityUsageRecord>, UsageExtractionError> {
    if conversation_id.trim().is_empty() {
        return Err(UsageExtractionError::InvalidConversationId);
    }

    let mut records = Vec::new();
    let mut seen_response_ids = BTreeSet::new();
    for frame in frames {
        visit_value(
            frame,
            &ExtractionContext::default(),
            variant,
            conversation_id,
            &mut seen_response_ids,
            &mut records,
        )?;
    }
    Ok(records)
}

fn visit_value(
    value: &Value,
    context: &ExtractionContext,
    variant: AntigravityProductVariant,
    conversation_id: &str,
    seen_response_ids: &mut BTreeSet<String>,
    records: &mut Vec<AntigravityUsageRecord>,
) -> Result<(), UsageExtractionError> {
    match value {
        Value::Object(object) => {
            let context = context.with_object(object);
            if let Some(record) = record_from_object(object, &context, variant, conversation_id)? {
                if let Some(response_id) = record.response_id.as_deref() {
                    if !seen_response_ids.insert(response_id.to_owned()) {
                        return Ok(());
                    }
                }
                records.push(record);
            }
            for child in object.values() {
                visit_value(
                    child,
                    &context,
                    variant,
                    conversation_id,
                    seen_response_ids,
                    records,
                )?;
            }
        }
        Value::Array(items) => {
            for child in items {
                visit_value(
                    child,
                    context,
                    variant,
                    conversation_id,
                    seen_response_ids,
                    records,
                )?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn record_from_object(
    object: &Map<String, Value>,
    context: &ExtractionContext,
    variant: AntigravityProductVariant,
    conversation_id: &str,
) -> Result<Option<AntigravityUsageRecord>, UsageExtractionError> {
    let counters = UsageCounters::from_object(object)?;
    if !counters.has_usage() {
        return Ok(None);
    }

    let Some(raw_model_id) = context.raw_model_id.clone() else {
        return Ok(None);
    };
    let model_label = context
        .model_display_name
        .clone()
        .or_else(|| context.response_model.clone())
        .unwrap_or_else(|| raw_model_id.clone());

    Ok(Some(AntigravityUsageRecord {
        variant,
        conversation_id: conversation_id.to_owned(),
        raw_model_id,
        model_label,
        api_provider: context.api_provider.clone(),
        response_id: context.response_id.clone(),
        input_tokens: counters.input_tokens,
        output_tokens: counters.output_tokens,
        thinking_output_tokens: counters.thinking_output_tokens,
        response_output_tokens: counters.response_output_tokens,
        cache_read_tokens: counters.cache_read_tokens,
        cache_write_tokens: counters.cache_write_tokens,
        consumed_credits: context.consumed_credits.clone(),
        flow_credits_used: context.flow_credits_used.clone(),
        prompt_credits_used: context.prompt_credits_used.clone(),
    }))
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct ExtractionContext {
    raw_model_id: Option<String>,
    response_model: Option<String>,
    model_display_name: Option<String>,
    api_provider: Option<String>,
    response_id: Option<String>,
    consumed_credits: Option<String>,
    flow_credits_used: Option<String>,
    prompt_credits_used: Option<String>,
}

impl ExtractionContext {
    fn with_object(&self, object: &Map<String, Value>) -> Self {
        Self {
            raw_model_id: string_field(object, "model").or_else(|| self.raw_model_id.clone()),
            response_model: string_field(object, "responseModel")
                .or_else(|| self.response_model.clone()),
            model_display_name: string_field(object, "modelDisplayName")
                .or_else(|| self.model_display_name.clone()),
            api_provider: string_field(object, "apiProvider").or_else(|| self.api_provider.clone()),
            response_id: string_field(object, "responseId").or_else(|| self.response_id.clone()),
            consumed_credits: diagnostic_field(object, "consumedCredits")
                .or_else(|| {
                    nested_diagnostic_field(object, "creditUsageSummary", "consumedCredits")
                })
                .or_else(|| self.consumed_credits.clone()),
            flow_credits_used: diagnostic_field(object, "flowCreditsUsed")
                .or_else(|| {
                    nested_diagnostic_field(object, "creditUsageSummary", "flowCreditsUsed")
                })
                .or_else(|| self.flow_credits_used.clone()),
            prompt_credits_used: diagnostic_field(object, "promptCreditsUsed")
                .or_else(|| {
                    nested_diagnostic_field(object, "creditUsageSummary", "promptCreditsUsed")
                })
                .or_else(|| self.prompt_credits_used.clone()),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct UsageCounters {
    input_tokens: u64,
    output_tokens: u64,
    thinking_output_tokens: u64,
    response_output_tokens: u64,
    cache_read_tokens: u64,
    cache_write_tokens: u64,
}

impl UsageCounters {
    fn from_object(object: &Map<String, Value>) -> Result<Self, UsageExtractionError> {
        Ok(Self {
            input_tokens: u64_field(object, "inputTokens")?,
            output_tokens: u64_field(object, "outputTokens")?,
            thinking_output_tokens: u64_field(object, "thinkingOutputTokens")?,
            response_output_tokens: u64_field(object, "responseOutputTokens")?,
            cache_read_tokens: u64_field(object, "cacheReadTokens")?,
            cache_write_tokens: u64_field(object, "cacheWriteTokens")?,
        })
    }

    const fn has_usage(self) -> bool {
        self.input_tokens > 0
            || self.output_tokens > 0
            || self.thinking_output_tokens > 0
            || self.response_output_tokens > 0
            || self.cache_read_tokens > 0
            || self.cache_write_tokens > 0
    }
}

fn string_field(object: &Map<String, Value>, key: &str) -> Option<String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn u64_field(object: &Map<String, Value>, key: &str) -> Result<u64, UsageExtractionError> {
    let Some(value) = object.get(key) else {
        return Ok(0);
    };
    if let Some(number) = value.as_u64() {
        return Ok(number);
    }
    if let Some(text) = value.as_str() {
        return text
            .parse::<u64>()
            .map_err(|_| UsageExtractionError::InvalidTokenValue);
    }
    Err(UsageExtractionError::InvalidTokenValue)
}

fn diagnostic_field(object: &Map<String, Value>, key: &str) -> Option<String> {
    let value = object.get(key)?;
    match value {
        Value::Number(number) => Some(number.to_string()),
        Value::String(text) => {
            let text = text.trim();
            if text.is_empty() {
                None
            } else {
                Some(text.to_owned())
            }
        }
        _ => None,
    }
}

fn nested_diagnostic_field(object: &Map<String, Value>, parent: &str, key: &str) -> Option<String> {
    object
        .get(parent)
        .and_then(Value::as_object)
        .and_then(|nested| diagnostic_field(nested, key))
}

fn checked_add_all(values: [u64; 4]) -> Result<u64, UsageExtractionError> {
    values.into_iter().try_fold(0_u64, |total, value| {
        total
            .checked_add(value)
            .ok_or(UsageExtractionError::TokenOverflow)
    })
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UsageExtractionError {
    #[error("antigravity conversation id is invalid")]
    InvalidConversationId,
    #[error("antigravity usage token value is invalid")]
    InvalidTokenValue,
    #[error("antigravity usage token total overflowed")]
    TokenOverflow,
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use serde_json::json;

    use super::*;
    use crate::infrastructure::collectors::antigravity::runtime_metadata_client::generator_metadata_items;

    fn fixture_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src/infrastructure/collectors/antigravity/fixtures")
            .join(name)
    }

    #[test]
    fn extracts_usage_records_from_nested_agent_state_frames() {
        let frames = vec![json!({
            "response": {
                "generatorMetadatas": [{
                    "responseId": "response-1",
                    "model": "MODEL_PLACEHOLDER_M16",
                    "responseModel": "gemini-pro-default",
                    "modelDisplayName": "Gemini 3.1 Pro (High)",
                    "apiProvider": "API_PROVIDER_GOOGLE_GEMINI",
                    "creditUsageSummary": {
                        "consumedCredits": 20,
                        "flowCreditsUsed": "1.5",
                        "promptCreditsUsed": 0
                    },
                    "usage": {
                        "inputTokens": 100,
                        "outputTokens": 30,
                        "thinkingOutputTokens": 10,
                        "responseOutputTokens": 20,
                        "cacheReadTokens": 5,
                        "cacheWriteTokens": 3
                    }
                }]
            }
        })];

        let records =
            extract_usage_records(AntigravityProductVariant::App, "conversation-1", &frames)
                .expect("usage records");

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].raw_model_id, "MODEL_PLACEHOLDER_M16");
        assert_eq!(records[0].model_label, "Gemini 3.1 Pro (High)");
        assert_eq!(
            records[0].api_provider.as_deref(),
            Some("API_PROVIDER_GOOGLE_GEMINI")
        );
        assert_eq!(records[0].response_id.as_deref(), Some("response-1"));
        assert_eq!(records[0].input_tokens, 100);
        assert_eq!(records[0].output_tokens, 30);
        assert_eq!(records[0].thinking_output_tokens, 10);
        assert_eq!(records[0].response_output_tokens, 20);
        assert_eq!(records[0].cache_read_tokens, 5);
        assert_eq!(records[0].cache_write_tokens, 3);
        assert_eq!(records[0].consumed_credits.as_deref(), Some("20"));
        assert_eq!(records[0].flow_credits_used.as_deref(), Some("1.5"));
        assert_eq!(records[0].prompt_credits_used.as_deref(), Some("0"));
        assert_eq!(records[0].total_tokens().expect("total"), 138);
    }

    #[test]
    fn deduplicates_records_by_response_id_across_frames() {
        let frame = json!({
            "responseId": "same-response",
            "model": "gemini-flash",
            "inputTokens": 10
        });

        let records = extract_usage_records(
            AntigravityProductVariant::Ide,
            "conversation-1",
            &[frame.clone(), frame],
        )
        .expect("usage records");

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].variant, AntigravityProductVariant::Ide);
    }

    #[test]
    fn ignores_usage_counters_without_model_context() {
        let records = extract_usage_records(
            AntigravityProductVariant::Cli,
            "conversation-1",
            &[json!({"response": {"inputTokens": 10}})],
        )
        .expect("usage records");

        assert!(records.is_empty());
    }

    #[test]
    fn extracts_usage_records_from_generator_metadata_fixture() {
        let fixture =
            fs::read_to_string(fixture_path("generator_metadata.json")).expect("fixture");
        let response: serde_json::Value = serde_json::from_str(&fixture).expect("json");
        let metadata = generator_metadata_items(&response);

        let records = extract_usage_from_generator_metadata(
            AntigravityProductVariant::App,
            "conversation-a",
            &metadata,
        )
        .expect("usage records");

        assert_eq!(records.len(), 3);
        let primary = records
            .iter()
            .find(|record| record.response_id.as_deref() == Some("response-primary"))
            .expect("primary usage record");
        assert_eq!(primary.model_label, "Gemini 3.1 Pro (High)");
        assert!(
            records
                .iter()
                .any(|record| record.response_id.as_deref() == Some("response-retry"))
        );
        assert!(
            records
                .iter()
                .any(|record| record.raw_model_id == "MODEL_PLACEHOLDER_M50")
        );
    }

    #[test]
    fn falls_back_to_response_model_when_display_name_is_missing() {
        let records = extract_usage_from_generator_metadata(
            AntigravityProductVariant::Ide,
            "conversation-b",
            &[json!({
                "chatModel": {
                    "model": "MODEL_PLACEHOLDER_M50",
                    "responseModel": "gemini-flash-default",
                    "usage": {
                        "model": "MODEL_PLACEHOLDER_M50",
                        "inputTokens": 12,
                        "outputTokens": 3
                    }
                }
            })],
        )
        .expect("usage records");

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].model_label, "gemini-flash-default");
    }

    #[test]
    fn collapses_duplicate_response_ids_in_generator_metadata() {
        let frame = json!({
            "chatModel": {
                "model": "gemini",
                "usage": {
                    "model": "gemini",
                    "inputTokens": 10,
                    "responseId": "duplicate-response"
                }
            }
        });

        let records = extract_usage_from_generator_metadata(
            AntigravityProductVariant::App,
            "conversation-a",
            &[frame.clone(), frame],
        )
        .expect("usage records");

        assert_eq!(records.len(), 1);
    }

    #[test]
    fn rejects_invalid_token_values() {
        let error = extract_usage_records(
            AntigravityProductVariant::Cli,
            "conversation-1",
            &[json!({
                "model": "gemini",
                "inputTokens": -1
            })],
        )
        .expect_err("invalid token");

        assert_eq!(error, UsageExtractionError::InvalidTokenValue);
    }
}
