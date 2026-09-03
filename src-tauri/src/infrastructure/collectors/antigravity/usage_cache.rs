use std::sync::Arc;

use chrono::{DateTime, Utc};

use crate::application::collection::CollectionScope;
use crate::application::ports::antigravity_usage_cache::{
    AntigravityCalendarAttribution, AntigravityTimestampOrigin, AntigravityUsageCache,
    AntigravityUsageCacheError, AntigravityUsageCacheReconcileResult, AntigravityUsageCacheUpsert,
    CachedAntigravityUsageRecord,
};

use super::mapper::ConversationUsage;
use super::product_variant::AntigravityProductVariant;
use super::{AntigravityUsageRecord, ConversationDatabase};

const CACHE_SCOPE_CONVERSATION_BATCH_SIZE: usize = 200;
const CACHE_RECONCILIATION_CONVERSATION_BATCH_SIZE: usize = 20;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct AntigravityCacheSupplementReport {
    pub(crate) records_read: u32,
    pub(crate) records_used: u32,
    pub(crate) used_cache: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct AntigravityCacheResolutionReport {
    pub(crate) source_reported_records: u32,
    pub(crate) first_seen_records: u32,
    pub(crate) legacy_records_repaired: u32,
    pub(crate) unresolved_legacy_records: u32,
}

impl AntigravityCacheResolutionReport {
    fn include(&mut self, result: &AntigravityUsageCacheReconcileResult) {
        self.legacy_records_repaired = self
            .legacy_records_repaired
            .saturating_add(result.legacy_records_repaired);
        for record in &result.records {
            let counter = match record.timestamp_origin {
                AntigravityTimestampOrigin::SourceReported => &mut self.source_reported_records,
                AntigravityTimestampOrigin::FirstSeen => &mut self.first_seen_records,
                AntigravityTimestampOrigin::LegacyUnknown => &mut self.unresolved_legacy_records,
                AntigravityTimestampOrigin::Unresolved => continue,
            };
            *counter = counter.saturating_add(1);
        }
    }
}

pub(crate) struct NoOpAntigravityUsageCache;

impl AntigravityUsageCache for NoOpAntigravityUsageCache {
    fn reconcile(
        &self,
        records: &[AntigravityUsageCacheUpsert],
        collected_at: DateTime<Utc>,
    ) -> Result<AntigravityUsageCacheReconcileResult, AntigravityUsageCacheError> {
        let records = records
            .iter()
            .map(|entry| {
                let mut record = entry.record.clone();
                if record.timestamp_origin == AntigravityTimestampOrigin::Unresolved {
                    record.observed_at = Some(collected_at);
                    record.timestamp_origin = AntigravityTimestampOrigin::FirstSeen;
                }
                Ok(record)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(AntigravityUsageCacheReconcileResult {
            records,
            legacy_records_repaired: 0,
        })
    }

    fn read_for_scope(
        &self,
        _scope: &CollectionScope,
        _aggregation_timezone: &str,
        _conversations: &[(&str, &str)],
    ) -> Result<Vec<CachedAntigravityUsageRecord>, AntigravityUsageCacheError> {
        Ok(Vec::new())
    }
}

#[derive(Clone)]
pub(crate) struct AntigravityUsageCacheClient {
    cache: Arc<dyn AntigravityUsageCache>,
}

impl AntigravityUsageCacheClient {
    pub(crate) fn new(cache: Arc<dyn AntigravityUsageCache>) -> Self {
        Self { cache }
    }

    pub(crate) fn reconcile_usage(
        &self,
        usage: &mut [ConversationUsage],
        collector_version: &str,
        collected_at: DateTime<Utc>,
    ) -> Result<AntigravityCacheResolutionReport, AntigravityUsageCacheError> {
        let mut report = AntigravityCacheResolutionReport::default();
        for conversations in usage.chunks_mut(CACHE_RECONCILIATION_CONVERSATION_BATCH_SIZE) {
            let mut upserts = Vec::new();
            for conversation in conversations.iter() {
                for record in &conversation.records {
                    upserts.push(AntigravityUsageCacheUpsert {
                        record: cached_record_from_usage(record, conversation.database.modified_at),
                        legacy_fallback_at: record.legacy_fallback_at,
                        collector_version: collector_version.to_owned(),
                    });
                }
            }
            let result = self.cache.reconcile(&upserts, collected_at)?;
            report.include(&result);
            for conversation in conversations {
                conversation.records = result
                    .records
                    .iter()
                    .filter(|record| {
                        record.variant == conversation.database.variant.as_str()
                            && record.conversation_id == conversation.database.conversation_id
                    })
                    .cloned()
                    .filter_map(|record| {
                        usage_record_from_cached(record, conversation.database.variant)
                    })
                    .collect();
            }
        }
        Ok(report)
    }

    pub(crate) fn supplement_usage(
        &self,
        scope: &CollectionScope,
        aggregation_timezone: &str,
        conversations: &[ConversationDatabase],
        usage: &mut Vec<ConversationUsage>,
    ) -> Result<AntigravityCacheSupplementReport, AntigravityUsageCacheError> {
        let conversation_keys = conversations
            .iter()
            .map(|conversation| {
                (
                    conversation.variant.as_str(),
                    conversation.conversation_id.as_str(),
                )
            })
            .collect::<Vec<_>>();
        let mut cached = Vec::new();
        for keys in conversation_keys.chunks(CACHE_SCOPE_CONVERSATION_BATCH_SIZE) {
            cached.extend(
                self.cache
                    .read_for_scope(scope, aggregation_timezone, keys)?,
            );
        }
        let records_read = cached.len().try_into().unwrap_or(u32::MAX);
        if cached.is_empty() {
            return Ok(AntigravityCacheSupplementReport {
                records_read,
                ..AntigravityCacheSupplementReport::default()
            });
        }

        let mut records_used = 0_u32;
        for conversation in conversations {
            let cached_for_conversation = cached
                .iter()
                .filter(|record| {
                    record.conversation_id == conversation.conversation_id
                        && record.variant == conversation.variant.as_str()
                })
                .cloned()
                .collect::<Vec<_>>();
            if cached_for_conversation.is_empty() {
                continue;
            }

            let records = cached_for_conversation
                .into_iter()
                .filter_map(|record| usage_record_from_cached(record, conversation.variant))
                .collect::<Vec<_>>();
            if records.is_empty() {
                continue;
            }
            records_used =
                records_used.saturating_add(records.len().try_into().unwrap_or(u32::MAX));
            if let Some(existing) = usage.iter_mut().find(|entry| {
                entry.database.conversation_id == conversation.conversation_id
                    && entry.database.variant == conversation.variant
            }) {
                existing.records.extend(records);
            } else {
                usage.push(ConversationUsage {
                    database: conversation.clone(),
                    records,
                });
            }
        }

        Ok(AntigravityCacheSupplementReport {
            records_read,
            records_used,
            used_cache: records_used > 0,
        })
    }
}

fn cached_record_from_usage(
    record: &AntigravityUsageRecord,
    observed_at: DateTime<Utc>,
) -> CachedAntigravityUsageRecord {
    let (resolved_at, timestamp_origin) = match record.timestamp_origin {
        AntigravityTimestampOrigin::Unresolved
            if record.variant != AntigravityProductVariant::Cli =>
        {
            (
                Some(record.observed_at.unwrap_or(observed_at)),
                AntigravityTimestampOrigin::LegacyUnknown,
            )
        }
        _ => (record.observed_at, record.timestamp_origin),
    };
    CachedAntigravityUsageRecord {
        variant: record.variant.as_str().to_owned(),
        conversation_id: record.conversation_id.clone(),
        response_id: record.response_id.clone(),
        raw_model_id: record.raw_model_id.clone(),
        model_label: record.model_label.clone(),
        api_provider: record.api_provider.clone(),
        source_record_index: record.source_record_index,
        input_tokens: record.input_tokens,
        output_tokens: record.output_tokens,
        thinking_output_tokens: record.thinking_output_tokens,
        response_output_tokens: record.response_output_tokens,
        cache_read_tokens: record.cache_read_tokens,
        cache_write_tokens: record.cache_write_tokens,
        observed_at: resolved_at,
        timestamp_origin,
        calendar_attribution: AntigravityCalendarAttribution::Dated,
    }
}

fn usage_record_from_cached(
    record: CachedAntigravityUsageRecord,
    variant: AntigravityProductVariant,
) -> Option<AntigravityUsageRecord> {
    if record.variant != variant.as_str() {
        return None;
    }
    let observed_at = record.observed_at?;

    Some(AntigravityUsageRecord {
        variant,
        conversation_id: record.conversation_id,
        raw_model_id: record.raw_model_id,
        model_label: record.model_label,
        api_provider: record.api_provider,
        response_id: record.response_id,
        source_record_index: record.source_record_index,
        observed_at: Some(observed_at),
        timestamp_origin: record.timestamp_origin,
        legacy_fallback_at: None,
        input_tokens: record.input_tokens,
        output_tokens: record.output_tokens,
        thinking_output_tokens: record.thinking_output_tokens,
        response_output_tokens: record.response_output_tokens,
        cache_read_tokens: record.cache_read_tokens,
        cache_write_tokens: record.cache_write_tokens,
        consumed_credits: None,
        flow_credits_used: None,
        prompt_credits_used: None,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::Mutex;

    use chrono::TimeZone;

    use super::*;
    use crate::application::ports::antigravity_usage_cache::AntigravityUsageCache;

    #[derive(Default)]
    pub(crate) struct RecordingUsageCache {
        upserts: Mutex<Vec<AntigravityUsageCacheUpsert>>,
        records: Mutex<Vec<CachedAntigravityUsageRecord>>,
    }

    impl RecordingUsageCache {
        pub(crate) fn upserts(&self) -> Vec<AntigravityUsageCacheUpsert> {
            self.upserts.lock().expect("upserts").clone()
        }
    }

    impl AntigravityUsageCache for RecordingUsageCache {
        fn reconcile(
            &self,
            records: &[AntigravityUsageCacheUpsert],
            collected_at: DateTime<Utc>,
        ) -> Result<AntigravityUsageCacheReconcileResult, AntigravityUsageCacheError> {
            self.upserts
                .lock()
                .expect("upserts")
                .extend(records.iter().cloned());
            let records = records
                .iter()
                .map(|entry| {
                    let mut record = entry.record.clone();
                    if record.timestamp_origin == AntigravityTimestampOrigin::Unresolved {
                        record.observed_at = Some(collected_at);
                        record.timestamp_origin = AntigravityTimestampOrigin::FirstSeen;
                    }
                    record
                })
                .collect();
            Ok(AntigravityUsageCacheReconcileResult {
                records,
                legacy_records_repaired: 0,
            })
        }

        fn read_for_scope(
            &self,
            _scope: &CollectionScope,
            _aggregation_timezone: &str,
            conversations: &[(&str, &str)],
        ) -> Result<Vec<CachedAntigravityUsageRecord>, AntigravityUsageCacheError> {
            Ok(self
                .records
                .lock()
                .expect("records")
                .iter()
                .filter(|record| {
                    conversations.iter().any(|(variant, conversation_id)| {
                        record.variant == *variant && record.conversation_id == *conversation_id
                    })
                })
                .cloned()
                .collect())
        }
    }

    impl RecordingUsageCache {
        pub(crate) fn seed(self, records: Vec<CachedAntigravityUsageRecord>) -> Self {
            *self.records.lock().expect("records") = records;
            self
        }
    }

    pub(crate) fn conversation(
        variant: AntigravityProductVariant,
        conversation_id: &str,
        modified_at: DateTime<Utc>,
    ) -> ConversationDatabase {
        ConversationDatabase {
            variant,
            conversation_id: conversation_id.to_owned(),
            path: conversation_id.into(),
            modified_at,
        }
    }

    pub(crate) fn cached_record(
        variant: AntigravityProductVariant,
        conversation_id: &str,
        response_id: &str,
        input_tokens: u64,
        output_tokens: u64,
    ) -> CachedAntigravityUsageRecord {
        CachedAntigravityUsageRecord {
            variant: variant.as_str().to_owned(),
            conversation_id: conversation_id.to_owned(),
            response_id: Some(response_id.to_owned()),
            raw_model_id: "gemini".to_owned(),
            model_label: "Gemini Flash".to_owned(),
            api_provider: None,
            source_record_index: None,
            input_tokens,
            output_tokens,
            thinking_output_tokens: 0,
            response_output_tokens: output_tokens,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            observed_at: Some(Utc.with_ymd_and_hms(2026, 7, 2, 8, 0, 0).unwrap()),
            timestamp_origin: AntigravityTimestampOrigin::SourceReported,
            calendar_attribution: AntigravityCalendarAttribution::Dated,
        }
    }

    #[test]
    fn supplements_missing_conversation_usage_from_cache() {
        let cache = RecordingUsageCache::default().seed(vec![cached_record(
            AntigravityProductVariant::App,
            "conversation-a",
            "response-1",
            40,
            8,
        )]);
        let client = AntigravityUsageCacheClient::new(Arc::new(cache));
        let conversations = vec![conversation(
            AntigravityProductVariant::App,
            "conversation-a",
            Utc.with_ymd_and_hms(2026, 7, 2, 8, 0, 0).unwrap(),
        )];
        let mut usage = Vec::new();

        let report = client
            .supplement_usage(&CollectionScope::Full, "UTC", &conversations, &mut usage)
            .expect("supplement");

        assert!(report.used_cache);
        assert_eq!(usage.len(), 1);
        assert_eq!(usage[0].records[0].input_tokens, 40);
    }

    #[test]
    fn supplements_existing_conversation_usage_with_cached_records() {
        let cache = RecordingUsageCache::default().seed(vec![cached_record(
            AntigravityProductVariant::App,
            "conversation-a",
            "response-cached",
            40,
            8,
        )]);
        let client = AntigravityUsageCacheClient::new(Arc::new(cache));
        let conversation = conversation(
            AntigravityProductVariant::App,
            "conversation-a",
            Utc.with_ymd_and_hms(2026, 7, 2, 8, 0, 0).unwrap(),
        );
        let mut usage = vec![ConversationUsage {
            database: conversation.clone(),
            records: vec![usage_record_from_cached(
                cached_record(
                    AntigravityProductVariant::App,
                    "conversation-a",
                    "response-live",
                    10,
                    2,
                ),
                AntigravityProductVariant::App,
            )
            .expect("record")],
        }];

        let report = client
            .supplement_usage(&CollectionScope::Full, "UTC", &[conversation], &mut usage)
            .expect("supplement");

        assert!(report.used_cache);
        assert_eq!(usage.len(), 1);
        assert_eq!(usage[0].records.len(), 2);
        assert!(usage[0]
            .records
            .iter()
            .any(|record| record.response_id.as_deref() == Some("response-live")));
        assert!(usage[0]
            .records
            .iter()
            .any(|record| record.response_id.as_deref() == Some("response-cached")));
    }
}
