//! DeepSeek Harness usage mapping.
//!
//! Maps folded session observations into Burnly daily and session candidates.
//! Cost is Burnly-calculated from the embedded models.dev snapshot when the
//! route model is priced; otherwise it remains unavailable.

use std::collections::BTreeMap;

use chrono::{DateTime, NaiveDate, Utc};
use chrono_tz::Tz;
use thiserror::Error;

use crate::application::collection::{
    CandidateProvenance, CollectionId, CollectionScope, CollectorKey, DailyUsageCandidate,
    ModelUsageCandidate, SessionUsageCandidate,
};
use crate::application::cost::BurnlyCostCalculator;
use crate::domain::identity::{daily_source_key, session_source_key, IdentityError};
use crate::domain::source::SourceKey;
use crate::domain::usage::{
    CostKind, CurrencyCode, TokenUsage, UsageCost, UsageValidationError, ValuedCostStatus,
};

use super::session_log_reader::SessionLogHeader;
use super::usage_fold::UsageObservation;
use crate::infrastructure::collectors::support::{
    checked_add_u64, date_in_scope, local_date_from_millis, provenance, utc_from_millis,
    MappingIdentity,
};

pub(crate) const COLLECTOR_KEY: &str = "deepseek-harness";
pub(crate) const PROFILE_VERSION: u16 = 1;
const UNKNOWN_MODEL: &str = "unknown";

/// One decoded session with its folded usage contributions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionUsage {
    pub(crate) header: SessionLogHeader,
    pub(crate) observations: Vec<UsageObservation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DeepSeekHarnessMappingContext {
    collector: CollectorKey,
    collector_version: String,
    collection_id: CollectionId,
    observed_at: DateTime<Utc>,
}

impl DeepSeekHarnessMappingContext {
    pub(crate) fn new(
        collector_version: String,
        collection_id: CollectionId,
        observed_at: DateTime<Utc>,
    ) -> Result<Self, DeepSeekHarnessMappingError> {
        if collector_version.trim().is_empty() {
            return Err(DeepSeekHarnessMappingError::EmptyCollectorVersion);
        }
        Ok(Self {
            collector: CollectorKey::new(COLLECTOR_KEY)
                .map_err(|_| DeepSeekHarnessMappingError::CollectorKey)?,
            collector_version,
            collection_id,
            observed_at,
        })
    }

    fn provenance(&self) -> CandidateProvenance {
        provenance(&MappingIdentity {
            source: SourceKey::DeepSeekHarness,
            collector: self.collector.clone(),
            collector_version: self.collector_version.clone(),
            profile_version: PROFILE_VERSION,
            collection_id: self.collection_id.clone(),
            observed_at: self.observed_at,
        })
    }
}

pub(crate) fn map_daily(
    sessions: &[SessionUsage],
    timezone: &str,
    scope: &CollectionScope,
    context: &DeepSeekHarnessMappingContext,
    calculator: &BurnlyCostCalculator,
) -> Result<Vec<DailyUsageCandidate>, DeepSeekHarnessMappingError> {
    let timezone = timezone
        .parse::<Tz>()
        .map_err(|_| DeepSeekHarnessMappingError::InvalidTimezone)?;
    let mut buckets = BTreeMap::<NaiveDate, DailyBucket>::new();

    for session in sessions {
        for observation in &session.observations {
            let usage_date = local_date_from_millis(
                observation.observed_at_ms,
                timezone,
                DeepSeekHarnessMappingError::InvalidTimestamp,
            )?;
            if !date_in_scope(usage_date, scope) {
                continue;
            }
            buckets.entry(usage_date).or_default().add(observation)?;
        }
    }

    buckets
        .into_iter()
        .map(|(usage_date, bucket)| {
            let tokens = bucket.total.tokens()?;
            let model_breakdowns = build_model_breakdowns(bucket.models, calculator)?;
            let cost = aggregate_cost(&model_breakdowns, &tokens, calculator);
            Ok(DailyUsageCandidate {
                provenance: context.provenance(),
                source_key: daily_source_key(
                    SourceKey::DeepSeekHarness,
                    usage_date,
                    timezone.name(),
                )?,
                usage_date,
                aggregation_timezone: timezone.name().to_owned(),
                tokens,
                cost,
                model_breakdowns,
            })
        })
        .collect()
}

pub(crate) fn map_sessions(
    sessions: Vec<SessionUsage>,
    context: &DeepSeekHarnessMappingContext,
    calculator: &BurnlyCostCalculator,
) -> Result<Vec<SessionUsageCandidate>, DeepSeekHarnessMappingError> {
    let mut candidates = Vec::new();

    for session in sessions {
        if session.observations.is_empty() {
            continue;
        }

        let mut total = TokenAccumulator::default();
        let mut models = BTreeMap::<String, TokenAccumulator>::new();
        let mut first_activity_at_ms = session.header.created_at_ms;
        let mut last_activity_at_ms = session.header.created_at_ms;

        for observation in &session.observations {
            total.add(&observation.tokens)?;
            models
                .entry(model_label(observation))
                .or_default()
                .add(&observation.tokens)?;
            first_activity_at_ms = first_activity_at_ms.min(observation.observed_at_ms);
            last_activity_at_ms = last_activity_at_ms.max(observation.observed_at_ms);
        }

        let tokens = total.tokens()?;
        let model_breakdowns = build_model_breakdowns(models, calculator)?;
        let cost = aggregate_cost(&model_breakdowns, &tokens, calculator);
        candidates.push(SessionUsageCandidate {
            provenance: context.provenance(),
            source_key: session_source_key(SourceKey::DeepSeekHarness, &session.header.id)?,
            source_session_id: session.header.id,
            project_path: session.header.cwd,
            first_activity_at: Some(utc_from_millis(
                first_activity_at_ms,
                DeepSeekHarnessMappingError::InvalidTimestamp,
            )?),
            last_activity_at: Some(utc_from_millis(
                last_activity_at_ms,
                DeepSeekHarnessMappingError::InvalidTimestamp,
            )?),
            tokens,
            cost,
            model_breakdowns,
        });
    }

    Ok(candidates)
}

fn build_model_breakdowns(
    models: BTreeMap<String, TokenAccumulator>,
    calculator: &BurnlyCostCalculator,
) -> Result<Vec<ModelUsageCandidate>, DeepSeekHarnessMappingError> {
    models
        .into_iter()
        .map(|(model, accumulator)| {
            let tokens = accumulator.tokens()?;
            let cost = calculator.calculate(&model, &tokens).cost;
            Ok(ModelUsageCandidate {
                raw_model_id: model,
                tokens,
                cost,
            })
        })
        .collect()
}

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

fn model_label(observation: &UsageObservation) -> String {
    observation
        .route
        .as_ref()
        .map(|route| route.model.clone())
        .unwrap_or_else(|| UNKNOWN_MODEL.to_owned())
}

#[derive(Debug, Default)]
struct DailyBucket {
    total: TokenAccumulator,
    models: BTreeMap<String, TokenAccumulator>,
}

impl DailyBucket {
    fn add(&mut self, observation: &UsageObservation) -> Result<(), DeepSeekHarnessMappingError> {
        self.total.add(&observation.tokens)?;
        self.models
            .entry(model_label(observation))
            .or_default()
            .add(&observation.tokens)
    }
}

#[derive(Debug, Default)]
struct TokenAccumulator {
    input_tokens: OptionalTokenAccumulator,
    output_tokens: OptionalTokenAccumulator,
    cache_creation_tokens: OptionalTokenAccumulator,
    cache_read_tokens: OptionalTokenAccumulator,
    total_tokens: u64,
}

impl TokenAccumulator {
    fn add(&mut self, usage: &TokenUsage) -> Result<(), DeepSeekHarnessMappingError> {
        self.total_tokens = checked_add_u64(
            self.total_tokens,
            usage.total_tokens(),
            DeepSeekHarnessMappingError::TokenOverflow,
        )?;
        self.input_tokens.add(usage.input_tokens())?;
        self.output_tokens.add(usage.output_tokens())?;
        self.cache_creation_tokens
            .add(usage.cache_creation_tokens())?;
        self.cache_read_tokens.add(usage.cache_read_tokens())?;
        Ok(())
    }

    fn tokens(&self) -> Result<TokenUsage, DeepSeekHarnessMappingError> {
        TokenUsage::new(
            self.input_tokens.value(),
            self.output_tokens.value(),
            self.cache_creation_tokens.value(),
            self.cache_read_tokens.value(),
            self.total_tokens,
        )
        .map_err(Into::into)
    }
}

#[derive(Debug, Clone, Copy, Default)]
enum OptionalTokenAccumulator {
    #[default]
    NotStarted,
    Known(u64),
    Unknown,
}

impl OptionalTokenAccumulator {
    fn add(&mut self, value: Option<u64>) -> Result<(), DeepSeekHarnessMappingError> {
        *self = match (*self, value) {
            (Self::NotStarted, Some(value)) => Self::Known(value),
            (Self::NotStarted, None) => Self::Unknown,
            (Self::Known(left), Some(right)) => Self::Known(checked_add_u64(
                left,
                right,
                DeepSeekHarnessMappingError::TokenOverflow,
            )?),
            (Self::Known(_), None) | (Self::Unknown, _) => Self::Unknown,
        };
        Ok(())
    }

    const fn value(self) -> Option<u64> {
        match self {
            Self::Known(value) => Some(value),
            Self::NotStarted | Self::Unknown => None,
        }
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub(crate) enum DeepSeekHarnessMappingError {
    #[error("mapping requires a collector version")]
    EmptyCollectorVersion,

    #[error("mapping requires a valid collector key")]
    CollectorKey,

    #[error("mapping requires a valid timezone")]
    InvalidTimezone,

    #[error("mapping received an invalid timestamp")]
    InvalidTimestamp,

    #[error("mapping token total overflowed")]
    TokenOverflow,

    #[error(transparent)]
    Identity(#[from] IdentityError),

    #[error(transparent)]
    Usage(#[from] UsageValidationError),
}

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, TimeZone, Utc};

    use super::*;
    use crate::application::collection::CollectionId;
    use crate::domain::usage::CostKind;
    use crate::infrastructure::collectors::deepseek_harness::event_parser::SessionModelRoute;

    fn utc_ms(year: i32, month: u32, day: u32, hour: u32) -> i64 {
        Utc.with_ymd_and_hms(year, month, day, hour, 0, 0)
            .single()
            .expect("timestamp")
            .timestamp_millis()
    }

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("date")
    }

    fn header(id: &str, created_at_ms: i64, cwd: Option<&str>) -> SessionLogHeader {
        SessionLogHeader {
            version: 4,
            id: id.to_owned(),
            created_at_ms,
            cwd: cwd.map(str::to_owned),
            is_seeded: false,
            origin: None,
            parent_session: None,
            delegation_depth: None,
            agent_preset: None,
        }
    }

    fn tokens(input: u64, output: u64, cache_read: Option<u64>, total: u64) -> TokenUsage {
        TokenUsage::new(Some(input), Some(output), None, cache_read, total)
            .expect("valid token usage")
    }

    fn observation(
        seq: u64,
        observed_at_ms: i64,
        model: Option<&str>,
        tokens: TokenUsage,
    ) -> UsageObservation {
        UsageObservation {
            seq,
            observed_at_ms,
            turn: 1,
            step: seq,
            route: model.map(|model| SessionModelRoute {
                provider: "provider".to_owned(),
                model: model.to_owned(),
            }),
            tokens,
        }
    }

    fn context() -> DeepSeekHarnessMappingContext {
        DeepSeekHarnessMappingContext::new(
            "test".to_owned(),
            CollectionId::new("collection-1").expect("collection id"),
            Utc.with_ymd_and_hms(2026, 1, 3, 0, 0, 0)
                .single()
                .expect("observed at"),
        )
        .expect("context")
    }

    #[test]
    fn daily_buckets_by_local_date_and_model() {
        let first_day = utc_ms(2026, 1, 1, 10);
        let second_day = utc_ms(2026, 1, 2, 10);
        let sessions = vec![SessionUsage {
            header: header("session-1", first_day, None),
            observations: vec![
                observation(1, first_day, Some("model-a"), tokens(1, 1, None, 2)),
                observation(2, second_day, Some("model-b"), tokens(3, 1, None, 4)),
            ],
        }];

        let candidates = map_daily(
            &sessions,
            "UTC",
            &CollectionScope::Full,
            &context(),
            &BurnlyCostCalculator::new(),
        )
        .expect("daily mapping");

        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].usage_date, date(2026, 1, 1));
        assert_eq!(candidates[0].tokens.total_tokens(), 2);
        assert_eq!(candidates[0].model_breakdowns[0].raw_model_id, "model-a");
        assert_eq!(candidates[1].usage_date, date(2026, 1, 2));
        assert_eq!(candidates[1].tokens.total_tokens(), 4);
        assert_eq!(candidates[1].model_breakdowns[0].raw_model_id, "model-b");
    }

    #[test]
    fn daily_scope_filters_to_incremental_range() {
        let first_day = utc_ms(2026, 1, 1, 10);
        let second_day = utc_ms(2026, 1, 2, 10);
        let sessions = vec![SessionUsage {
            header: header("session-1", first_day, None),
            observations: vec![
                observation(1, first_day, Some("model-a"), tokens(1, 1, None, 2)),
                observation(2, second_day, Some("model-b"), tokens(3, 1, None, 4)),
            ],
        }];
        let scope =
            CollectionScope::incremental(date(2026, 1, 2), date(2026, 1, 2)).expect("scope");

        let candidates = map_daily(
            &sessions,
            "UTC",
            &scope,
            &context(),
            &BurnlyCostCalculator::new(),
        )
        .expect("daily mapping");

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].usage_date, date(2026, 1, 2));
        assert_eq!(candidates[0].tokens.total_tokens(), 4);
    }

    #[test]
    fn daily_preserves_unknown_optional_token_buckets() {
        let day = utc_ms(2026, 1, 1, 10);
        let sessions = vec![SessionUsage {
            header: header("session-1", day, None),
            observations: vec![
                observation(1, day, Some("model-a"), tokens(1, 1, Some(3), 5)),
                observation(2, day, Some("model-a"), tokens(1, 1, None, 2)),
            ],
        }];

        let candidates = map_daily(
            &sessions,
            "UTC",
            &CollectionScope::Full,
            &context(),
            &BurnlyCostCalculator::new(),
        )
        .expect("daily mapping");

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].tokens.total_tokens(), 7);
        assert_eq!(candidates[0].tokens.cache_read_tokens(), None);
        assert_eq!(
            candidates[0].model_breakdowns[0].tokens.cache_read_tokens(),
            None
        );
    }

    #[test]
    fn session_maps_activity_bounds_and_project_path() {
        let created_at = utc_ms(2026, 1, 1, 9);
        let first_activity = utc_ms(2026, 1, 1, 10);
        let last_activity = utc_ms(2026, 1, 1, 11);
        let sessions = vec![SessionUsage {
            header: header("session-1", created_at, Some("/redacted/project")),
            observations: vec![
                observation(1, first_activity, Some("model-a"), tokens(1, 1, None, 2)),
                observation(2, last_activity, Some("model-a"), tokens(3, 1, None, 4)),
            ],
        }];

        let candidates = map_sessions(sessions, &context(), &BurnlyCostCalculator::new())
            .expect("session mapping");

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].source_session_id, "session-1");
        assert_eq!(
            candidates[0].source_key,
            "deepseek-harness:session:v1:session-1"
        );
        assert_eq!(
            candidates[0].project_path.as_deref(),
            Some("/redacted/project")
        );
        assert_eq!(candidates[0].tokens.total_tokens(), 6);
        assert_eq!(
            candidates[0].first_activity_at,
            Some(Utc.timestamp_millis_opt(created_at).single().unwrap())
        );
        assert_eq!(
            candidates[0].last_activity_at,
            Some(Utc.timestamp_millis_opt(last_activity).single().unwrap())
        );
    }

    #[test]
    fn session_skips_sessions_without_observations() {
        let created_at = utc_ms(2026, 1, 1, 9);
        let sessions = vec![SessionUsage {
            header: header("session-1", created_at, None),
            observations: Vec::new(),
        }];

        let candidates = map_sessions(sessions, &context(), &BurnlyCostCalculator::new())
            .expect("session mapping");

        assert!(candidates.is_empty());
    }

    #[test]
    fn unpriced_model_cost_is_unavailable() {
        let day = utc_ms(2026, 1, 1, 10);
        let sessions = vec![SessionUsage {
            header: header("session-1", day, None),
            observations: vec![observation(
                1,
                day,
                Some("unpriced-test-model"),
                tokens(1, 1, None, 2),
            )],
        }];

        let candidates = map_daily(
            &sessions,
            "UTC",
            &CollectionScope::Full,
            &context(),
            &BurnlyCostCalculator::new(),
        )
        .expect("daily mapping");

        assert!(matches!(
            candidates[0].cost,
            UsageCost::Unavailable {
                kind: CostKind::BurnlyCalculated
            }
        ));
        assert!(matches!(
            candidates[0].model_breakdowns[0].cost,
            UsageCost::Unavailable {
                kind: CostKind::BurnlyCalculated
            }
        ));
    }

    #[test]
    fn token_accumulator_rejects_overflow() {
        let day = utc_ms(2026, 1, 1, 10);
        let sessions = vec![SessionUsage {
            header: header("session-1", day, None),
            observations: vec![
                observation(
                    1,
                    day,
                    Some("model-a"),
                    TokenUsage::new(Some(0), Some(0), None, None, u64::MAX).expect("max total"),
                ),
                observation(2, day, Some("model-a"), tokens(0, 1, None, 1)),
            ],
        }];

        let error = map_daily(
            &sessions,
            "UTC",
            &CollectionScope::Full,
            &context(),
            &BurnlyCostCalculator::new(),
        )
        .expect_err("overflow");

        assert_eq!(error, DeepSeekHarnessMappingError::TokenOverflow);
    }

    #[test]
    fn invalid_timezone_is_rejected() {
        let sessions = Vec::new();

        let error = map_daily(
            &sessions,
            "Not/AZone",
            &CollectionScope::Full,
            &context(),
            &BurnlyCostCalculator::new(),
        )
        .expect_err("invalid timezone");

        assert_eq!(error, DeepSeekHarnessMappingError::InvalidTimezone);
    }
}
