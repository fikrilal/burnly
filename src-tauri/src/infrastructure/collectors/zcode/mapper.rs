use std::collections::BTreeMap;

use chrono::{DateTime, NaiveDate, Utc};
use chrono_tz::Tz;
use thiserror::Error;

use crate::{
    application::collection::{
        CandidateProvenance, CollectionId, CollectionScope, CollectorKey, DailyUsageCandidate,
        ModelUsageCandidate, SessionUsageCandidate,
    },
    application::cost::BurnlyCostCalculator,
    domain::{
        identity::{daily_source_key, session_source_key, IdentityError},
        source::SourceKey,
        usage::{
            CostKind, CurrencyCode, TokenUsage, UsageCost, UsageValidationError, ValuedCostStatus,
        },
    },
};

use super::super::support::{
    checked_add_u64, date_in_scope, local_date_from_millis, provenance, utc_from_millis,
    MappingIdentity,
};
use super::ZCodeModelUsageRow;

const PROFILE_VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ZCodeMappingContext {
    collector: CollectorKey,
    collector_version: String,
    collection_id: CollectionId,
    observed_at: DateTime<Utc>,
}

impl ZCodeMappingContext {
    pub(crate) fn new(
        collector: CollectorKey,
        collector_version: String,
        collection_id: CollectionId,
        observed_at: DateTime<Utc>,
    ) -> Result<Self, ZCodeMappingError> {
        if collector_version.trim().is_empty() {
            return Err(ZCodeMappingError::EmptyCollectorVersion);
        }
        Ok(Self {
            collector,
            collector_version,
            collection_id,
            observed_at,
        })
    }

    fn provenance(&self) -> CandidateProvenance {
        provenance(&MappingIdentity {
            source: SourceKey::ZCode,
            collector: self.collector.clone(),
            collector_version: self.collector_version.clone(),
            profile_version: PROFILE_VERSION,
            collection_id: self.collection_id.clone(),
            observed_at: self.observed_at,
        })
    }
}

pub(crate) fn map_daily(
    rows: Vec<ZCodeModelUsageRow>,
    timezone: &str,
    scope: &CollectionScope,
    context: &ZCodeMappingContext,
    calculator: &BurnlyCostCalculator,
) -> Result<Vec<DailyUsageCandidate>, ZCodeMappingError> {
    let timezone = timezone
        .parse::<Tz>()
        .map_err(|_| ZCodeMappingError::InvalidTimezone)?;
    let mut buckets = BTreeMap::<NaiveDate, ZCodeDailyBucket>::new();

    for row in rows.into_iter().filter(is_completed) {
        let usage_date = local_date_from_millis(
            row.started_at_ms,
            timezone,
            ZCodeMappingError::InvalidTimestamp,
        )?;
        if !date_in_scope(usage_date, scope) {
            continue;
        }
        buckets.entry(usage_date).or_default().add(&row)?;
    }

    buckets
        .into_iter()
        .map(|(usage_date, usage)| {
            let tokens = usage.total.tokens()?;
            let model_breakdowns = usage
                .models
                .into_iter()
                .map(|(model, usage)| {
                    let tokens = usage.tokens()?;
                    let cost = cost(&model, &tokens, calculator);
                    Ok(ModelUsageCandidate {
                        raw_model_id: model,
                        tokens,
                        cost,
                    })
                })
                .collect::<Result<Vec<_>, ZCodeMappingError>>()?;
            let aggregate_cost = aggregate_cost(&model_breakdowns, &tokens, calculator);
            Ok(DailyUsageCandidate {
                provenance: context.provenance(),
                source_key: daily_source_key(SourceKey::ZCode, usage_date, timezone.name())?,
                usage_date,
                aggregation_timezone: timezone.name().to_owned(),
                tokens,
                cost: aggregate_cost,
                model_breakdowns,
            })
        })
        .collect()
}

pub(crate) fn map_sessions(
    rows: Vec<ZCodeModelUsageRow>,
    context: &ZCodeMappingContext,
    calculator: &BurnlyCostCalculator,
) -> Result<Vec<SessionUsageCandidate>, ZCodeMappingError> {
    let mut buckets = BTreeMap::<(String, String), ZCodeSessionAccumulator>::new();

    for row in rows.into_iter().filter(is_completed) {
        buckets
            .entry((row.session_id.clone(), row.model_id.clone()))
            .or_insert_with(|| {
                ZCodeSessionAccumulator::new(row.session_id.clone(), row.model_id.clone())
            })
            .add(&row)?;
    }

    buckets
        .into_values()
        .map(|usage| usage.candidate(context, calculator))
        .collect()
}

#[derive(Debug, Default)]
struct ZCodeDailyBucket {
    total: ZCodeUsageAccumulator,
    models: BTreeMap<String, ZCodeUsageAccumulator>,
}

impl ZCodeDailyBucket {
    fn add(&mut self, row: &ZCodeModelUsageRow) -> Result<(), ZCodeMappingError> {
        self.total.add(row)?;
        self.models
            .entry(row.model_id.clone())
            .or_default()
            .add(row)
    }
}

#[derive(Debug, Default)]
struct ZCodeUsageAccumulator {
    input_tokens: u64,
    output_tokens: u64,
    reasoning_tokens: u64,
    cache_creation_input_tokens: u64,
    cache_read_input_tokens: u64,
    computed_total_tokens: u64,
}

impl ZCodeUsageAccumulator {
    fn add(&mut self, row: &ZCodeModelUsageRow) -> Result<(), ZCodeMappingError> {
        self.input_tokens = checked_add(self.input_tokens, row.input_tokens)?;
        self.output_tokens = checked_add(self.output_tokens, row.output_tokens)?;
        self.reasoning_tokens = checked_add(self.reasoning_tokens, row.reasoning_tokens)?;
        self.cache_creation_input_tokens = checked_add(
            self.cache_creation_input_tokens,
            row.cache_creation_input_tokens,
        )?;
        self.cache_read_input_tokens =
            checked_add(self.cache_read_input_tokens, row.cache_read_input_tokens)?;
        self.computed_total_tokens =
            checked_add(self.computed_total_tokens, row.computed_total_tokens)?;
        Ok(())
    }

    fn tokens(&self) -> Result<TokenUsage, ZCodeMappingError> {
        let disjoint_input = self
            .input_tokens
            .checked_sub(self.cache_read_input_tokens)
            .and_then(|value| value.checked_sub(self.cache_creation_input_tokens))
            .ok_or(ZCodeMappingError::OverlappingCacheTokens)?;
        TokenUsage::new(
            Some(disjoint_input),
            Some(self.output_tokens),
            Some(self.cache_creation_input_tokens),
            Some(self.cache_read_input_tokens),
            self.computed_total_tokens,
        )
        .map_err(Into::into)
    }
}

struct ZCodeSessionAccumulator {
    session_id: String,
    model_id: String,
    usage: ZCodeUsageAccumulator,
    first_activity_ms: i64,
    last_activity_ms: i64,
}

impl ZCodeSessionAccumulator {
    fn new(session_id: String, model_id: String) -> Self {
        Self {
            session_id,
            model_id,
            usage: ZCodeUsageAccumulator::default(),
            first_activity_ms: i64::MAX,
            last_activity_ms: i64::MIN,
        }
    }

    fn add(&mut self, row: &ZCodeModelUsageRow) -> Result<(), ZCodeMappingError> {
        self.first_activity_ms = self.first_activity_ms.min(row.started_at_ms);
        self.last_activity_ms = self
            .last_activity_ms
            .max(row.completed_at_ms.unwrap_or(row.started_at_ms));
        self.usage.add(row)
    }

    fn candidate(
        self,
        context: &ZCodeMappingContext,
        calculator: &BurnlyCostCalculator,
    ) -> Result<SessionUsageCandidate, ZCodeMappingError> {
        let tokens = self.usage.tokens()?;
        let cost = cost(&self.model_id, &tokens, calculator);
        Ok(SessionUsageCandidate {
            provenance: context.provenance(),
            source_key: session_source_key(
                SourceKey::ZCode,
                &format!("{}:{}", self.session_id, self.model_id),
            )?,
            source_session_id: self.session_id,
            project_path: None,
            first_activity_at: Some(timestamp(self.first_activity_ms)?),
            last_activity_at: Some(timestamp(self.last_activity_ms)?),
            tokens: tokens.clone(),
            cost: cost.clone(),
            model_breakdowns: vec![ModelUsageCandidate {
                raw_model_id: self.model_id,
                tokens,
                cost,
            }],
        })
    }
}

fn is_completed(row: &ZCodeModelUsageRow) -> bool {
    row.status == "completed"
}

fn checked_add(left: u64, right: u64) -> Result<u64, ZCodeMappingError> {
    checked_add_u64(left, right, ZCodeMappingError::TokenOverflow)
}

fn cost(model: &str, tokens: &TokenUsage, calculator: &BurnlyCostCalculator) -> UsageCost {
    calculator.calculate(model, tokens).cost
}

/// Daily aggregate cost: the sum of per-model valued micros when breakdowns
/// exist; otherwise price the aggregate tokens with no model.
fn aggregate_cost(
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

fn timestamp(timestamp_ms: i64) -> Result<DateTime<Utc>, ZCodeMappingError> {
    utc_from_millis(timestamp_ms, ZCodeMappingError::InvalidTimestamp)
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub(crate) enum ZCodeMappingError {
    #[error("zcode mapping requires a collector version")]
    EmptyCollectorVersion,
    #[error("zcode mapping requires a valid timezone")]
    InvalidTimezone,
    #[error("zcode mapping received an invalid timestamp")]
    InvalidTimestamp,
    #[error("zcode token total overflowed")]
    TokenOverflow,
    #[error("zcode cache tokens exceed input tokens")]
    OverlappingCacheTokens,
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error(transparent)]
    Usage(#[from] UsageValidationError),
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;
    use crate::application::collection::CollectionScope;

    #[test]
    fn maps_completed_rows_to_daily_usage_with_disjoint_cache_tokens() {
        let context = context();

        let candidates = map_daily(
            rows(),
            "Asia/Jakarta",
            &CollectionScope::Full,
            &context,
            &calculator(),
        )
        .expect("daily");

        assert_eq!(candidates.len(), 1);
        let candidate = &candidates[0];
        assert_eq!(
            candidate.source_key,
            "zcode:daily:v1:Asia/Jakarta:2026-07-02"
        );
        assert_eq!(candidate.tokens.input_tokens(), Some(14_224));
        assert_eq!(candidate.tokens.output_tokens(), Some(3_299));
        assert_eq!(candidate.tokens.cache_read_tokens(), Some(7_360));
        assert_eq!(candidate.tokens.cache_creation_tokens(), Some(0));
        assert_eq!(candidate.tokens.total_tokens(), 24_883);
        assert_eq!(candidate.model_breakdowns.len(), 2);

        let glm_52 = candidate
            .model_breakdowns
            .iter()
            .find(|model| model.raw_model_id == "GLM-5.2")
            .expect("glm 5.2");
        assert_eq!(glm_52.tokens.total_tokens(), 8_610);

        let glm_turbo = candidate
            .model_breakdowns
            .iter()
            .find(|model| model.raw_model_id == "GLM-5-Turbo")
            .expect("glm turbo");
        assert_eq!(glm_turbo.tokens.total_tokens(), 16_273);
    }

    #[test]
    fn maps_completed_rows_to_session_usage_by_session_and_model() {
        let context = context();

        let candidates = map_sessions(rows(), &context, &calculator()).expect("sessions");

        assert_eq!(candidates.len(), 2);
        assert!(candidates
            .iter()
            .all(|candidate| { candidate.source_key.starts_with("zcode:session:v1:sess-") }));
        assert!(candidates
            .iter()
            .any(|candidate| candidate.source_session_id == "sess-main"));
    }

    #[test]
    fn rejects_cache_tokens_that_exceed_input_tokens() {
        let mut rows = rows();
        rows[0].cache_read_input_tokens = 20_000;

        let error = map_daily(
            rows,
            "UTC",
            &CollectionScope::Full,
            &context(),
            &calculator(),
        )
        .expect_err("invalid overlap");

        assert_eq!(error, ZCodeMappingError::OverlappingCacheTokens);
    }

    fn context() -> ZCodeMappingContext {
        ZCodeMappingContext::new(
            CollectorKey::new("zcode").expect("collector"),
            "local".to_owned(),
            CollectionId::new("zcode-test").expect("collection"),
            Utc.with_ymd_and_hms(2026, 7, 2, 1, 0, 0)
                .single()
                .expect("timestamp"),
        )
        .expect("context")
    }

    fn calculator() -> BurnlyCostCalculator {
        BurnlyCostCalculator::new()
    }

    fn rows() -> Vec<ZCodeModelUsageRow> {
        vec![
            ZCodeModelUsageRow {
                id: "usage-main-1".to_owned(),
                session_id: "sess-main".to_owned(),
                turn_id: Some("turn-main-1".to_owned()),
                query_source: "interactive".to_owned(),
                provider_id: "builtin:zai-start-plan".to_owned(),
                model_id: "GLM-5.2".to_owned(),
                status: "completed".to_owned(),
                started_at_ms: 1_782_952_270_000,
                completed_at_ms: Some(1_782_952_275_000),
                input_tokens: 8_488,
                output_tokens: 122,
                reasoning_tokens: 0,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 7_360,
                provider_total_tokens: Some(8_610),
                computed_total_tokens: 8_610,
            },
            ZCodeModelUsageRow {
                id: "usage-subagent-1".to_owned(),
                session_id: "sess-subagent".to_owned(),
                turn_id: Some("turn-subagent-1".to_owned()),
                query_source: "subagent".to_owned(),
                provider_id: "builtin:zai-start-plan".to_owned(),
                model_id: "GLM-5-Turbo".to_owned(),
                status: "completed".to_owned(),
                started_at_ms: 1_782_952_320_000,
                completed_at_ms: Some(1_782_952_324_000),
                input_tokens: 13_096,
                output_tokens: 3_177,
                reasoning_tokens: 0,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
                provider_total_tokens: Some(16_273),
                computed_total_tokens: 16_273,
            },
            ZCodeModelUsageRow {
                id: "usage-running-1".to_owned(),
                session_id: "sess-running".to_owned(),
                turn_id: Some("turn-running-1".to_owned()),
                query_source: "interactive".to_owned(),
                provider_id: "builtin:zai-start-plan".to_owned(),
                model_id: "GLM-5-Turbo".to_owned(),
                status: "running".to_owned(),
                started_at_ms: 1_782_952_330_000,
                completed_at_ms: None,
                input_tokens: 0,
                output_tokens: 0,
                reasoning_tokens: 0,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
                provider_total_tokens: None,
                computed_total_tokens: 0,
            },
        ]
    }
}
