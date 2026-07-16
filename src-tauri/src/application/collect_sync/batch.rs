//! Deterministic chronological batch construction for daily usage push.

use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::dto::{
    DailyUsageFactDto, DailyUsagePushRequestDto, DailyUsageWindowDto, WireUploadScope,
    COLLECT_CONTRACT_VERSION,
};
use super::scope::UploadScope;

/// Backend contract limits for a single daily-usage request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BatchBuildLimits {
    pub max_facts_per_batch: usize,
    pub max_models_per_fact: usize,
}

impl BatchBuildLimits {
    pub(crate) const fn backend_v1() -> Self {
        Self {
            max_facts_per_batch: 1_000,
            max_models_per_fact: 100,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BatchRequestMeta {
    pub client_device_id: String,
    pub app_version: String,
    pub reporting_timezone: String,
    pub scope: UploadScope,
}

/// One immutable outbox-ready batch prior to revision allocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreparedBatch {
    pub batch_index: u32,
    pub batch_count: u32,
    pub client_revision: i64,
    pub idempotency_key: String,
    pub request_body: String,
    pub payload_hash: String,
    pub window_scope: WireUploadScope,
    pub window_start: String,
    pub window_end: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum BatchBuildError {
    #[error("a daily fact exceeded the maximum model count ({max})")]
    TooManyModels { max: usize },
    #[error("failed to serialize daily usage push request")]
    Serialize,
}

/// Sort facts deterministically and split into contract-sized batches.
///
/// Empty input yields no batches (no pointless outbox rows).
pub(crate) fn build_prepared_batches(
    mut facts: Vec<DailyUsageFactDto>,
    meta: &BatchRequestMeta,
    limits: BatchBuildLimits,
    first_client_revision: i64,
) -> Result<Vec<PreparedBatch>, BatchBuildError> {
    if facts.is_empty() {
        return Ok(Vec::new());
    }

    for fact in &facts {
        if fact.models.len() > limits.max_models_per_fact {
            return Err(BatchBuildError::TooManyModels {
                max: limits.max_models_per_fact,
            });
        }
    }

    facts.sort_by(|left, right| {
        left.usage_date
            .cmp(&right.usage_date)
            .then_with(|| left.identity_key.cmp(&right.identity_key))
    });

    let wire_scope = WireUploadScope::from(&meta.scope);
    let chunks: Vec<&[DailyUsageFactDto]> = facts.chunks(limits.max_facts_per_batch).collect();
    let batch_count = u32::try_from(chunks.len()).unwrap_or(u32::MAX);
    let mut prepared = Vec::with_capacity(chunks.len());

    for (index, chunk) in chunks.into_iter().enumerate() {
        let batch_index = u32::try_from(index).unwrap_or(u32::MAX);
        let window_start = chunk
            .iter()
            .map(|fact| fact.usage_date.as_str())
            .min()
            .unwrap_or("")
            .to_owned();
        let window_end = chunk
            .iter()
            .map(|fact| fact.usage_date.as_str())
            .max()
            .unwrap_or("")
            .to_owned();

        let client_revision = first_client_revision
            .checked_add(i64::try_from(index).unwrap_or(i64::MAX))
            .ok_or(BatchBuildError::Serialize)?;

        let request = DailyUsagePushRequestDto {
            contract_version: COLLECT_CONTRACT_VERSION,
            client_device_id: meta.client_device_id.clone(),
            app_version: meta.app_version.clone(),
            reporting_timezone: meta.reporting_timezone.clone(),
            client_revision,
            window: DailyUsageWindowDto {
                start_date: window_start.clone(),
                end_date: window_end.clone(),
                scope: wire_scope,
            },
            facts: chunk.to_vec(),
        };

        let request_body =
            serde_json::to_string(&request).map_err(|_| BatchBuildError::Serialize)?;
        let payload_hash = format!("{:x}", Sha256::digest(request_body.as_bytes()));
        let idempotency_key = Uuid::new_v4().to_string();

        prepared.push(PreparedBatch {
            batch_index,
            batch_count,
            client_revision,
            idempotency_key,
            request_body,
            payload_hash,
            window_scope: wire_scope,
            window_start,
            window_end,
        });
    }

    Ok(prepared)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::collect_sync::dto::{
        DailyUsageCostDto, DailyUsageModelDto, ModelUsageCostDto,
    };

    fn fact(date: &str, identity_suffix: &str, models: usize) -> DailyUsageFactDto {
        DailyUsageFactDto {
            identity_key: format!("claude-code:daily:v1:UTC:{date}:{identity_suffix}"),
            identity_version: 1,
            source_key: "claude-code".to_owned(),
            usage_date: date.to_owned(),
            aggregation_timezone: "UTC".to_owned(),
            input_tokens: Some(1),
            output_tokens: Some(1),
            cache_creation_tokens: Some(0),
            cache_read_tokens: Some(0),
            total_tokens: 2,
            unclassified_tokens: Some(0),
            cost: DailyUsageCostDto {
                status: "unavailable".to_owned(),
                kind: "unknown".to_owned(),
                amount_micros: None,
                currency: None,
            },
            data_quality: "complete".to_owned(),
            record_state: "active".to_owned(),
            first_seen_at: "2026-07-08T00:00:00.000Z".to_owned(),
            last_seen_at: "2026-07-08T00:00:00.000Z".to_owned(),
            removed_at: None,
            models: (0..models)
                .map(|index| DailyUsageModelDto {
                    raw_model_id: Some(format!("model-{index}")),
                    display_name: None,
                    provider_key: None,
                    input_tokens: Some(1),
                    output_tokens: Some(1),
                    cache_creation_tokens: Some(0),
                    cache_read_tokens: Some(0),
                    total_tokens: Some(2),
                    cost: ModelUsageCostDto {
                        status: "unavailable".to_owned(),
                        amount_micros: None,
                        currency: None,
                    },
                })
                .collect(),
        }
    }

    fn meta() -> BatchRequestMeta {
        BatchRequestMeta {
            client_device_id: "dev_1".to_owned(),
            app_version: "0.1.20".to_owned(),
            reporting_timezone: "UTC".to_owned(),
            scope: UploadScope::Full,
        }
    }

    #[test]
    fn empty_facts_produce_no_batches() {
        let batches =
            build_prepared_batches(Vec::new(), &meta(), BatchBuildLimits::backend_v1(), 1)
                .expect("build");
        assert!(batches.is_empty());
    }

    #[test]
    fn splits_chronologically_and_assigns_stable_windows() {
        let limits = BatchBuildLimits {
            max_facts_per_batch: 2,
            max_models_per_fact: 100,
        };
        let facts = vec![
            fact("2026-07-10", "b", 0),
            fact("2026-07-08", "a", 0),
            fact("2026-07-09", "c", 0),
        ];
        let batches = build_prepared_batches(facts, &meta(), limits, 10).expect("build");
        assert_eq!(batches.len(), 2);
        assert_eq!(batches[0].batch_index, 0);
        assert_eq!(batches[0].batch_count, 2);
        assert_eq!(batches[0].window_start, "2026-07-08");
        assert_eq!(batches[0].window_end, "2026-07-09");
        assert_eq!(batches[1].window_start, "2026-07-10");
        assert_eq!(batches[1].window_end, "2026-07-10");

        let first: DailyUsagePushRequestDto =
            serde_json::from_str(&batches[0].request_body).expect("parse");
        assert_eq!(first.client_revision, 10);
        assert_eq!(first.facts.len(), 2);
        assert_eq!(first.facts[0].usage_date, "2026-07-08");
        assert_eq!(first.facts[1].usage_date, "2026-07-09");

        let second: DailyUsagePushRequestDto =
            serde_json::from_str(&batches[1].request_body).expect("parse");
        assert_eq!(second.client_revision, 11);
    }

    #[test]
    fn rejects_model_overflow() {
        let limits = BatchBuildLimits {
            max_facts_per_batch: 10,
            max_models_per_fact: 2,
        };
        let error = build_prepared_batches(vec![fact("2026-07-08", "a", 3)], &meta(), limits, 1)
            .expect_err("overflow");
        assert_eq!(error, BatchBuildError::TooManyModels { max: 2 });
    }

    #[test]
    fn payload_hash_matches_body() {
        let batches = build_prepared_batches(
            vec![fact("2026-07-08", "a", 0)],
            &meta(),
            BatchBuildLimits::backend_v1(),
            1,
        )
        .expect("build");
        let expected = format!("{:x}", Sha256::digest(batches[0].request_body.as_bytes()));
        assert_eq!(batches[0].payload_hash, expected);
        assert!(!batches[0].idempotency_key.is_empty());
    }
}
