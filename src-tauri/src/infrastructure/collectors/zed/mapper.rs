//! Zed thread mapping.
//!
//! Maps parsed Zed thread usage into Burnly daily and session candidates.
//! Zed reports net input separately from cache_read, so totals never
//! double-count cached tokens. Cost is Burnly-calculated from the embedded
//! models.dev snapshot with the `zed.dev/` provider prefix normalized away.

use std::collections::BTreeMap;

use chrono::{DateTime, NaiveDate, Utc};
use chrono_tz::Tz;
use thiserror::Error;

use crate::application::collection::{
    CandidateProvenance, CollectionId, CollectionScope, CollectorKey, DailyUsageCandidate,
    ModelUsageCandidate, SessionUsageCandidate,
};
use crate::application::cost::BurnlyCostCalculator;
use crate::domain::{
    identity::{daily_source_key, session_source_key, IdentityError},
    source::SourceKey,
    usage::{
        CostKind, CurrencyCode, TokenUsage, UsageCost, UsageValidationError, ValuedCostStatus,
    },
};

use super::super::support::{date_in_scope, provenance, MappingIdentity};
use super::threads_store::{ZedThreadUsage, ZedTokenUsage};

const PROFILE_VERSION: u16 = 1;
const COLLECTOR_KEY: &str = "zed";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ZedMappingContext {
    collector: CollectorKey,
    collector_version: String,
    collection_id: CollectionId,
    observed_at: DateTime<Utc>,
}

impl ZedMappingContext {
    pub(crate) fn new(
        collector_version: String,
        collection_id: CollectionId,
        observed_at: DateTime<Utc>,
    ) -> Result<Self, ZedMappingError> {
        if collector_version.trim().is_empty() {
            return Err(ZedMappingError::EmptyCollectorVersion);
        }
        Ok(Self {
            collector: CollectorKey::new(COLLECTOR_KEY).expect("zed collector key"),
            collector_version,
            collection_id,
            observed_at,
        })
    }

    fn provenance(&self) -> CandidateProvenance {
        provenance(&MappingIdentity {
            source: SourceKey::Zed,
            collector: self.collector.clone(),
            collector_version: self.collector_version.clone(),
            profile_version: PROFILE_VERSION,
            collection_id: self.collection_id.clone(),
            observed_at: self.observed_at,
        })
    }
}

/// Map parsed threads into daily candidates attributed to the thread's local
/// day, and session candidates keyed by thread id.
pub(crate) fn map_threads(
    threads: Vec<ZedThreadUsage>,
    timezone: &str,
    scope: &CollectionScope,
    context: &ZedMappingContext,
    calculator: &BurnlyCostCalculator,
) -> Result<Vec<DailyUsageCandidate>, ZedMappingError> {
    let timezone = timezone
        .parse::<Tz>()
        .map_err(|_| ZedMappingError::InvalidTimezone)?;
    let mut buckets = BTreeMap::<NaiveDate, BTreeMap<String, ZedTokenUsage>>::new();

    for thread in threads {
        let usage_date = thread.updated_at.with_timezone(&timezone).date_naive();
        if !date_in_scope(usage_date, scope) {
            continue;
        }
        let model_usage = buckets.entry(usage_date).or_default();
        let entry = model_usage.entry(thread.model_id.clone()).or_default();
        entry.input_tokens = entry
            .input_tokens
            .saturating_add(thread.tokens.input_tokens);
        entry.output_tokens = entry
            .output_tokens
            .saturating_add(thread.tokens.output_tokens);
        entry.cache_read_tokens = entry
            .cache_read_tokens
            .saturating_add(thread.tokens.cache_read_tokens);
        entry.cache_creation_tokens = entry
            .cache_creation_tokens
            .saturating_add(thread.tokens.cache_creation_tokens);
    }

    buckets
        .into_iter()
        .map(|(usage_date, models)| {
            let mut aggregate = ZedTokenUsage::default();
            let mut model_breakdowns = Vec::with_capacity(models.len());
            for (model_id, model_usage) in models {
                let tokens = zed_tokens(&model_usage)?;
                aggregate.input_tokens = aggregate
                    .input_tokens
                    .saturating_add(model_usage.input_tokens);
                aggregate.output_tokens = aggregate
                    .output_tokens
                    .saturating_add(model_usage.output_tokens);
                aggregate.cache_read_tokens = aggregate
                    .cache_read_tokens
                    .saturating_add(model_usage.cache_read_tokens);
                aggregate.cache_creation_tokens = aggregate
                    .cache_creation_tokens
                    .saturating_add(model_usage.cache_creation_tokens);
                let cost = calculator
                    .calculate(&normalized_model_id(&model_id), &tokens)
                    .cost;
                model_breakdowns.push(ModelUsageCandidate {
                    raw_model_id: model_id,
                    tokens,
                    cost,
                });
            }
            let tokens = zed_tokens(&aggregate)?;
            let aggregate_cost = aggregate_zed_cost(&model_breakdowns, &tokens, calculator);
            Ok(DailyUsageCandidate {
                provenance: context.provenance(),
                source_key: daily_source_key(SourceKey::Zed, usage_date, timezone.name())?,
                usage_date,
                aggregation_timezone: timezone.name().to_owned(),
                tokens: tokens.clone(),
                cost: aggregate_cost,
                model_breakdowns,
            })
        })
        .collect()
}

/// Map parsed threads into session candidates (one per thread).
pub(crate) fn map_sessions(
    threads: Vec<ZedThreadUsage>,
    context: &ZedMappingContext,
    calculator: &BurnlyCostCalculator,
) -> Result<Vec<SessionUsageCandidate>, ZedMappingError> {
    threads
        .into_iter()
        .map(|thread| {
            let tokens = zed_tokens(&thread.tokens)?;
            let cost = calculator
                .calculate(&normalized_model_id(&thread.model_id), &tokens)
                .cost;
            Ok(SessionUsageCandidate {
                provenance: context.provenance(),
                source_key: session_source_key(
                    SourceKey::Zed,
                    &format!("{}:{}", thread.thread_id, thread.model_id),
                )?,
                source_session_id: thread.thread_id,
                project_path: None,
                first_activity_at: Some(thread.created_at),
                last_activity_at: Some(thread.updated_at),
                tokens: tokens.clone(),
                cost: cost.clone(),
                model_breakdowns: vec![ModelUsageCandidate {
                    raw_model_id: thread.model_id,
                    tokens,
                    cost,
                }],
            })
        })
        .collect()
}

/// Strip the `zed.dev/` provider prefix so model ids match the embedded
/// models.dev pricing snapshot (e.g. `zed.dev/gpt-5.6-luna` -> `gpt-5.6-luna`).
fn normalized_model_id(model_id: &str) -> String {
    model_id
        .strip_prefix("zed.dev/")
        .unwrap_or(model_id)
        .to_owned()
}

/// Daily aggregate cost: sum of per-model valued micros when any breakdown is
/// valued; otherwise the calculator's aggregate result.
fn aggregate_zed_cost(
    model_breakdowns: &[ModelUsageCandidate],
    tokens: &TokenUsage,
    calculator: &BurnlyCostCalculator,
) -> UsageCost {
    let mut total_micros = 0_u64;
    let mut saw_valued = false;
    for model in model_breakdowns {
        if let UsageCost::Valued { amount_micros, .. } = model.cost {
            total_micros = total_micros.saturating_add(amount_micros);
            saw_valued = true;
        }
    }
    if saw_valued {
        return UsageCost::Valued {
            amount_micros: total_micros,
            currency: CurrencyCode::new("USD").expect("USD is a valid ISO-shaped currency"),
            kind: CostKind::BurnlyCalculated,
            status: ValuedCostStatus::Estimated,
        };
    }
    calculator.calculate("", tokens).cost
}

/// Zed reports net input separately from cache_read; total is their sum plus
/// output and cache_creation (no double-counting).
fn zed_tokens(usage: &ZedTokenUsage) -> Result<TokenUsage, ZedMappingError> {
    let total = usage
        .input_tokens
        .checked_add(usage.output_tokens)
        .and_then(|v| v.checked_add(usage.cache_read_tokens))
        .and_then(|v| v.checked_add(usage.cache_creation_tokens))
        .ok_or(ZedMappingError::TokenOverflow)?;
    TokenUsage::new(
        Some(usage.input_tokens),
        Some(usage.output_tokens),
        Some(usage.cache_creation_tokens),
        Some(usage.cache_read_tokens),
        total,
    )
    .map_err(Into::into)
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub(crate) enum ZedMappingError {
    #[error("zed mapping requires a collector version")]
    EmptyCollectorVersion,
    #[error("zed mapping requires a valid timezone")]
    InvalidTimezone,
    #[error("zed token total overflowed")]
    TokenOverflow,
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error(transparent)]
    Usage(#[from] UsageValidationError),
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    fn thread(
        id: &str,
        model: &str,
        input: u64,
        output: u64,
        cache_read: u64,
        cache_creation: u64,
    ) -> ZedThreadUsage {
        ZedThreadUsage {
            thread_id: id.to_owned(),
            title: "t".to_owned(),
            model_provider: "zed.dev".to_owned(),
            model_id: model.to_owned(),
            created_at: Utc
                .with_ymd_and_hms(2026, 8, 9, 3, 42, 58)
                .single()
                .expect("ts"),
            updated_at: Utc
                .with_ymd_and_hms(2026, 8, 9, 3, 49, 28)
                .single()
                .expect("ts"),
            tokens: ZedTokenUsage {
                input_tokens: input,
                output_tokens: output,
                cache_read_tokens: cache_read,
                cache_creation_tokens: cache_creation,
            },
        }
    }

    fn context() -> ZedMappingContext {
        ZedMappingContext::new(
            "local".to_owned(),
            CollectionId::new("zed-test").expect("collection"),
            Utc.with_ymd_and_hms(2026, 8, 9, 4, 0, 0)
                .single()
                .expect("ts"),
        )
        .expect("context")
    }

    fn calculator() -> BurnlyCostCalculator {
        BurnlyCostCalculator::new()
    }

    #[test]
    fn maps_thread_to_daily_candidate_with_non_double_counted_total() {
        let threads = vec![thread("t1", "gpt-5.6-luna", 138468, 9644, 1586296, 0)];
        let candidates = map_threads(
            threads,
            "UTC",
            &CollectionScope::Full,
            &context(),
            &calculator(),
        )
        .expect("daily");

        assert_eq!(candidates.len(), 1);
        let c = &candidates[0];
        assert_eq!(c.source_key, "zed:daily:v1:UTC:2026-08-09");
        assert_eq!(c.tokens.input_tokens(), Some(138468));
        assert_eq!(c.tokens.output_tokens(), Some(9644));
        assert_eq!(c.tokens.cache_read_tokens(), Some(1586296));
        assert_eq!(c.tokens.total_tokens(), 138468 + 9644 + 1586296);
        assert_eq!(c.model_breakdowns[0].raw_model_id, "gpt-5.6-luna");
    }

    #[test]
    fn maps_threads_to_session_candidates() {
        let threads = vec![
            thread("t1", "gpt-5.6-luna", 138468, 9644, 1586296, 0),
            thread("t2", "gemini-3.5-flash", 873218, 2418, 0, 0),
        ];
        let candidates = map_sessions(threads, &context(), &calculator()).expect("sessions");

        assert_eq!(candidates.len(), 2);
        assert!(candidates[0].source_key.starts_with("zed:session:v1:t1:"));
        assert_eq!(candidates[0].tokens.total_tokens(), 138468 + 9644 + 1586296);
        assert_eq!(candidates[1].source_session_id, "t2");
        assert_eq!(candidates[1].tokens.cache_read_tokens(), Some(0));
    }

    #[test]
    fn respects_incremental_scope() {
        let threads = vec![thread("t1", "m", 10, 1, 0, 0)];
        let scope = CollectionScope::incremental(
            NaiveDate::from_ymd_opt(2026, 8, 10).expect("date"),
            NaiveDate::from_ymd_opt(2026, 8, 10).expect("date"),
        )
        .expect("scope");

        let candidates =
            map_threads(threads, "UTC", &scope, &context(), &calculator()).expect("daily");
        assert!(candidates.is_empty());
    }

    #[test]
    fn aggregates_threads_on_same_day() {
        let threads = vec![
            thread("t1", "m", 100, 10, 50, 0),
            thread("t2", "m", 200, 20, 0, 5),
        ];
        let candidates = map_threads(
            threads,
            "UTC",
            &CollectionScope::Full,
            &context(),
            &calculator(),
        )
        .expect("daily");

        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0].tokens.total_tokens(),
            100 + 10 + 50 + 200 + 20 + 5
        );
        assert_eq!(candidates[0].model_breakdowns.len(), 1);
    }

    #[test]
    fn aggregates_multi_model_threads_on_same_day_per_model() {
        // Regression: two threads on the same day using different models must
        // produce separate per-model breakdowns, not attribute everything to
        // the first model.
        let threads = vec![
            thread("t1", "gpt-5.6-luna", 100, 10, 50, 0),
            thread("t2", "claude-sonnet-5", 200, 20, 0, 5),
        ];
        let candidates = map_threads(
            threads,
            "UTC",
            &CollectionScope::Full,
            &context(),
            &calculator(),
        )
        .expect("daily");

        assert_eq!(candidates.len(), 1);
        let daily = &candidates[0];
        // Aggregate is the sum of both models.
        assert_eq!(daily.tokens.total_tokens(), 100 + 10 + 50 + 200 + 20 + 5);
        // Per-model breakdowns are separate.
        assert_eq!(daily.model_breakdowns.len(), 2);
        let luna = daily
            .model_breakdowns
            .iter()
            .find(|m| m.raw_model_id == "gpt-5.6-luna")
            .expect("luna breakdown");
        assert_eq!(luna.tokens.total_tokens(), 100 + 10 + 50);
        let claude = daily
            .model_breakdowns
            .iter()
            .find(|m| m.raw_model_id == "claude-sonnet-5")
            .expect("claude breakdown");
        assert_eq!(claude.tokens.total_tokens(), 200 + 20 + 5);
    }
}
