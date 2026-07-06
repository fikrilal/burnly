use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

use crate::application::collection::CollectionScope;
use crate::application::ports::grok_usage_cache::{
    CachedGrokUsageRecord, GrokUnifiedLogCheckpoint, GrokUsageCache, GrokUsageCacheError,
    GrokUsageCacheUpsert,
};

use super::mapper::{self, GrokMappedInference};
use super::model_resolver::GrokModelResolver;
use super::session_index::GrokSessionSummary;
use super::unified_log_reader::{GrokInferenceUsage, UnifiedLogFileMetadata, UnifiedLogReader};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct GrokIngestReport {
    pub(crate) rows_from_log: u32,
    pub(crate) rows_from_cache: u32,
    pub(crate) used_cache_fallback: bool,
    pub(crate) truncation_detected: bool,
}

pub(crate) struct NoOpGrokUsageCache;

impl GrokUsageCache for NoOpGrokUsageCache {
    fn upsert(&self, _records: &[GrokUsageCacheUpsert]) -> Result<(), GrokUsageCacheError> {
        Ok(())
    }

    fn read_for_scope(
        &self,
        _scope: &CollectionScope,
        _aggregation_timezone: &str,
        _session_ids: &[&str],
    ) -> Result<Vec<CachedGrokUsageRecord>, GrokUsageCacheError> {
        Ok(Vec::new())
    }

    fn read_checkpoint(&self) -> Result<Option<GrokUnifiedLogCheckpoint>, GrokUsageCacheError> {
        Ok(None)
    }

    fn write_checkpoint(
        &self,
        _checkpoint: GrokUnifiedLogCheckpoint,
    ) -> Result<(), GrokUsageCacheError> {
        Ok(())
    }
}

#[derive(Clone)]
pub(crate) struct GrokUsageCacheClient {
    cache: Arc<dyn GrokUsageCache>,
}

impl GrokUsageCacheClient {
    pub(crate) fn new(cache: Arc<dyn GrokUsageCache>) -> Self {
        Self { cache }
    }

    pub(crate) fn ingest(
        &self,
        log_path: &Path,
        scope: &CollectionScope,
        aggregation_timezone: &str,
        summaries: &[GrokSessionSummary],
        resolver: &GrokModelResolver,
        collector_version: &str,
    ) -> Result<(Vec<GrokMappedInference>, GrokIngestReport), GrokUsageCacheError> {
        let session_ids = summaries
            .iter()
            .map(|summary| summary.session_id.as_str())
            .collect::<Vec<_>>();
        let metadata = UnifiedLogReader::read_file_metadata(log_path).ok();
        let checkpoint = self.cache.read_checkpoint()?;
        let truncation_detected =
            metadata.is_some_and(|current| is_truncated(&current, checkpoint.as_ref()));
        let log_rows = UnifiedLogReader::read_from_path(log_path)
            .map(|(rows, _)| rows)
            .ok();

        let use_cache_fallback = truncation_detected || log_rows.is_none();
        if use_cache_fallback {
            let cached = self
                .cache
                .read_for_scope(scope, aggregation_timezone, &session_ids)?;
            let rows_from_cache = u32::try_from(cached.len()).unwrap_or(u32::MAX);
            if cached.is_empty() && log_rows.is_none() {
                return Ok((Vec::new(), GrokIngestReport::default()));
            }

            let mut mapped = cached_to_mapped(cached);
            if let Some(log_rows) = log_rows {
                let log_mapped = mapper::map_inferences(log_rows, resolver, summaries);
                mapped = merge_mapped(mapped, log_mapped);
            }

            return Ok((
                mapped,
                GrokIngestReport {
                    rows_from_log: 0,
                    rows_from_cache,
                    used_cache_fallback: rows_from_cache > 0,
                    truncation_detected,
                },
            ));
        }

        let log_rows = log_rows.unwrap_or_default();
        let rows_from_log = u32::try_from(log_rows.len()).unwrap_or(u32::MAX);
        let mapped = mapper::map_inferences(log_rows.clone(), resolver, summaries);
        self.upsert_mapped(&mapped, collector_version, metadata.as_ref())?;
        if let Some(metadata) = metadata {
            self.cache.write_checkpoint(GrokUnifiedLogCheckpoint {
                file_inode: metadata.file_inode,
                file_size: metadata.file_size,
                byte_offset: metadata.file_size,
            })?;
        }

        Ok((
            mapped,
            GrokIngestReport {
                rows_from_log,
                rows_from_cache: 0,
                used_cache_fallback: false,
                truncation_detected: false,
            },
        ))
    }

    fn upsert_mapped(
        &self,
        mapped: &[GrokMappedInference],
        collector_version: &str,
        metadata: Option<&UnifiedLogFileMetadata>,
    ) -> Result<(), GrokUsageCacheError> {
        if mapped.is_empty() {
            return Ok(());
        }

        let log_offset = metadata.map(|value| value.file_size).unwrap_or(0);
        let upserts = mapped
            .iter()
            .map(|row| GrokUsageCacheUpsert {
                record: cached_record_from_mapped(row),
                collector_version: collector_version.to_owned(),
                log_offset,
            })
            .collect::<Vec<_>>();
        self.cache.upsert(&upserts)
    }
}

fn is_truncated(
    current: &UnifiedLogFileMetadata,
    checkpoint: Option<&GrokUnifiedLogCheckpoint>,
) -> bool {
    let Some(checkpoint) = checkpoint else {
        return false;
    };
    if current.file_size < checkpoint.file_size {
        return true;
    }
    if let (Some(current_inode), Some(checkpoint_inode)) =
        (current.file_inode, checkpoint.file_inode)
    {
        return current_inode != checkpoint_inode && current.file_size < checkpoint.file_size;
    }
    false
}

fn cached_record_from_mapped(row: &GrokMappedInference) -> CachedGrokUsageRecord {
    CachedGrokUsageRecord {
        session_id: row.inference.session_id.clone(),
        observed_at: row.inference.observed_at,
        loop_index: row.inference.loop_index,
        pid: row.inference.pid,
        raw_model_id: row.model_id.clone(),
        model_display_name: None,
        project_path: row.project_path.clone(),
        prompt_tokens: row.inference.prompt_tokens,
        cached_prompt_tokens: row.inference.cached_prompt_tokens,
        completion_tokens: row.inference.completion_tokens,
        reasoning_tokens: row.inference.reasoning_tokens,
    }
}

fn cached_to_mapped(records: Vec<CachedGrokUsageRecord>) -> Vec<GrokMappedInference> {
    records
        .into_iter()
        .map(|record| GrokMappedInference {
            inference: GrokInferenceUsage {
                session_id: record.session_id,
                observed_at: record.observed_at,
                pid: record.pid,
                loop_index: record.loop_index,
                prompt_tokens: record.prompt_tokens,
                cached_prompt_tokens: record.cached_prompt_tokens,
                completion_tokens: record.completion_tokens,
                reasoning_tokens: record.reasoning_tokens,
            },
            model_id: record.raw_model_id,
            project_path: record.project_path,
        })
        .collect()
}

fn merge_mapped(
    cached: Vec<GrokMappedInference>,
    log_rows: Vec<GrokMappedInference>,
) -> Vec<GrokMappedInference> {
    let mut seen = cached
        .iter()
        .map(|row| row.inference.dedupe_key())
        .collect::<BTreeSet<_>>();
    let mut merged = cached;
    for row in log_rows {
        if seen.insert(row.inference.dedupe_key()) {
            merged.push(row);
        }
    }
    merged
}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::Mutex;

    use chrono::{TimeZone, Utc};

    use super::*;
    use crate::application::collection::CollectionScope;

    #[derive(Default)]
    pub(crate) struct RecordingGrokUsageCache {
        upserts: Mutex<Vec<GrokUsageCacheUpsert>>,
        records: Mutex<Vec<CachedGrokUsageRecord>>,
        checkpoint: Mutex<Option<GrokUnifiedLogCheckpoint>>,
    }

    impl RecordingGrokUsageCache {
        pub(crate) fn upserts(&self) -> Vec<GrokUsageCacheUpsert> {
            self.upserts.lock().expect("upserts").clone()
        }

        pub(crate) fn seed(self, records: Vec<CachedGrokUsageRecord>) -> Self {
            *self.records.lock().expect("records") = records;
            self
        }

        pub(crate) fn with_checkpoint(self, checkpoint: GrokUnifiedLogCheckpoint) -> Self {
            *self.checkpoint.lock().expect("checkpoint") = Some(checkpoint);
            self
        }
    }

    impl GrokUsageCache for RecordingGrokUsageCache {
        fn upsert(&self, records: &[GrokUsageCacheUpsert]) -> Result<(), GrokUsageCacheError> {
            self.upserts
                .lock()
                .expect("upserts")
                .extend(records.iter().cloned());
            Ok(())
        }

        fn read_for_scope(
            &self,
            _scope: &CollectionScope,
            _aggregation_timezone: &str,
            session_ids: &[&str],
        ) -> Result<Vec<CachedGrokUsageRecord>, GrokUsageCacheError> {
            Ok(self
                .records
                .lock()
                .expect("records")
                .iter()
                .filter(|record| {
                    session_ids.is_empty() || session_ids.contains(&record.session_id.as_str())
                })
                .cloned()
                .collect())
        }

        fn read_checkpoint(&self) -> Result<Option<GrokUnifiedLogCheckpoint>, GrokUsageCacheError> {
            Ok(self.checkpoint.lock().expect("checkpoint").clone())
        }

        fn write_checkpoint(
            &self,
            checkpoint: GrokUnifiedLogCheckpoint,
        ) -> Result<(), GrokUsageCacheError> {
            *self.checkpoint.lock().expect("checkpoint") = Some(checkpoint);
            Ok(())
        }
    }

    pub(crate) fn cached_record(
        session_id: &str,
        loop_index: u32,
        prompt_tokens: u64,
        completion_tokens: u64,
    ) -> CachedGrokUsageRecord {
        let observed_at = if loop_index == 1 {
            Utc.with_ymd_and_hms(2026, 7, 6, 10, 0, 0).unwrap()
        } else {
            Utc.with_ymd_and_hms(2026, 7, 6, 10, 0, 8).unwrap()
        };
        CachedGrokUsageRecord {
            session_id: session_id.to_owned(),
            observed_at,
            loop_index,
            pid: 1001,
            raw_model_id: "grok-composer-2.5-fast".to_owned(),
            model_display_name: Some("Composer 2.5".to_owned()),
            project_path: Some("/tmp/grok-fixture-project".to_owned()),
            prompt_tokens,
            cached_prompt_tokens: 8000,
            completion_tokens,
            reasoning_tokens: 0,
        }
    }

    #[test]
    fn detects_truncation_when_file_size_regresses() {
        assert!(is_truncated(
            &UnifiedLogFileMetadata {
                file_inode: Some(1),
                file_size: 100,
            },
            Some(&GrokUnifiedLogCheckpoint {
                file_inode: Some(1),
                file_size: 500,
                byte_offset: 500,
            }),
        ));
        assert!(!is_truncated(
            &UnifiedLogFileMetadata {
                file_inode: Some(1),
                file_size: 600,
            },
            Some(&GrokUnifiedLogCheckpoint {
                file_inode: Some(1),
                file_size: 500,
                byte_offset: 500,
            }),
        ));
    }

    #[test]
    fn merge_does_not_duplicate_inference_keys() {
        let cached = vec![mapped_inference(1, 12000, 240)];
        let log_rows = vec![
            mapped_inference(1, 12000, 240),
            mapped_inference(2, 15000, 180),
        ];

        let merged = merge_mapped(cached, log_rows);

        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn cached_records_store_usage_counters_without_transcript_fields() {
        let record = cached_record("session-a", 1, 12000, 240);

        assert_eq!(record.prompt_tokens, 12000);
        assert_eq!(record.completion_tokens, 240);
        assert_eq!(record.raw_model_id, "grok-composer-2.5-fast");
    }

    fn mapped_inference(
        loop_index: u32,
        prompt_tokens: u64,
        completion_tokens: u64,
    ) -> GrokMappedInference {
        GrokMappedInference {
            inference: GrokInferenceUsage {
                session_id: "019f0000-0000-7000-8000-000000000001".to_owned(),
                observed_at: Utc.with_ymd_and_hms(2026, 7, 6, 10, 0, 0).unwrap(),
                pid: 1001,
                loop_index,
                prompt_tokens,
                cached_prompt_tokens: 8000,
                completion_tokens,
                reasoning_tokens: 0,
            },
            model_id: "grok-composer-2.5-fast".to_owned(),
            project_path: Some("/tmp/grok-fixture-project".to_owned()),
        }
    }
}
