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

use super::super::support::{checked_add_u64, date_in_scope, provenance, MappingIdentity};
use super::{AntigravityUsageRecord, ConversationDatabase};

const PROFILE_VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AntigravityMappingContext {
    collector: CollectorKey,
    collector_version: String,
    collection_id: CollectionId,
    observed_at: DateTime<Utc>,
}

impl AntigravityMappingContext {
    pub(crate) fn new(
        collector: CollectorKey,
        collector_version: String,
        collection_id: CollectionId,
        observed_at: DateTime<Utc>,
    ) -> Result<Self, AntigravityMappingError> {
        if collector_version.trim().is_empty() {
            return Err(AntigravityMappingError::EmptyCollectorVersion);
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
            source: SourceKey::Antigravity,
            collector: self.collector.clone(),
            collector_version: self.collector_version.clone(),
            profile_version: PROFILE_VERSION,
            collection_id: self.collection_id.clone(),
            observed_at: self.observed_at,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConversationUsage {
    pub(crate) database: ConversationDatabase,
    pub(crate) records: Vec<AntigravityUsageRecord>,
}

pub(crate) fn map_daily(
    conversations: Vec<ConversationUsage>,
    timezone: &str,
    scope: &CollectionScope,
    context: &AntigravityMappingContext,
    calculator: &BurnlyCostCalculator,
) -> Result<Vec<DailyUsageCandidate>, AntigravityMappingError> {
    let timezone = timezone
        .parse::<Tz>()
        .map_err(|_| AntigravityMappingError::InvalidTimezone)?;
    let mut buckets = BTreeMap::<NaiveDate, AntigravityDailyBucket>::new();

    for conversation in conversations {
        for record in conversation.records {
            let activity_at = record
                .observed_at
                .unwrap_or(conversation.database.modified_at);
            let usage_date = activity_at.with_timezone(&timezone).date_naive();
            if !date_in_scope(usage_date, scope) {
                continue;
            }
            buckets.entry(usage_date).or_default().add(&record)?;
        }
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
                .collect::<Result<Vec<_>, AntigravityMappingError>>()?;
            let aggregate_cost = aggregate_cost(&model_breakdowns, &tokens, calculator);
            Ok(DailyUsageCandidate {
                provenance: context.provenance(),
                source_key: daily_source_key(SourceKey::Antigravity, usage_date, timezone.name())?,
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
    conversations: Vec<ConversationUsage>,
    context: &AntigravityMappingContext,
    calculator: &BurnlyCostCalculator,
) -> Result<Vec<SessionUsageCandidate>, AntigravityMappingError> {
    let mut candidates = Vec::new();
    for conversation in conversations {
        let mut usage = AntigravityUsageAccumulator::default();
        let mut models = BTreeMap::<String, AntigravityUsageAccumulator>::new();
        let mut first_activity_at = None;
        let mut last_activity_at = None;
        for record in conversation.records {
            let activity_at = record
                .observed_at
                .unwrap_or(conversation.database.modified_at);
            first_activity_at = Some(
                first_activity_at
                    .map_or(activity_at, |current| std::cmp::min(current, activity_at)),
            );
            last_activity_at = Some(
                last_activity_at.map_or(activity_at, |current| std::cmp::max(current, activity_at)),
            );
            usage.add(&record)?;
            models
                .entry(record.model_label.clone())
                .or_default()
                .add(&record)?;
        }
        if usage.is_empty() {
            continue;
        }

        let tokens = usage.tokens()?;
        let model_breakdowns = models
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
            .collect::<Result<Vec<_>, AntigravityMappingError>>()?;
        let aggregate_cost = aggregate_cost(&model_breakdowns, &tokens, calculator);
        let source_session_id = format!(
            "{}:{}",
            conversation.database.variant.as_str(),
            conversation.database.conversation_id
        );

        candidates.push(SessionUsageCandidate {
            provenance: context.provenance(),
            source_key: session_source_key(SourceKey::Antigravity, &source_session_id)?,
            source_session_id,
            project_path: None,
            first_activity_at,
            last_activity_at,
            tokens,
            cost: aggregate_cost,
            model_breakdowns,
        });
    }
    Ok(candidates)
}

#[derive(Debug, Default)]
struct AntigravityDailyBucket {
    total: AntigravityUsageAccumulator,
    models: BTreeMap<String, AntigravityUsageAccumulator>,
}

impl AntigravityDailyBucket {
    fn add(&mut self, record: &AntigravityUsageRecord) -> Result<(), AntigravityMappingError> {
        self.total.add(record)?;
        self.models
            .entry(record.model_label.clone())
            .or_default()
            .add(record)
    }
}

#[derive(Debug, Default)]
struct AntigravityUsageAccumulator {
    input_tokens: u64,
    output_tokens: u64,
    cache_creation_tokens: u64,
    cache_read_tokens: u64,
}

impl AntigravityUsageAccumulator {
    fn add(&mut self, record: &AntigravityUsageRecord) -> Result<(), AntigravityMappingError> {
        self.input_tokens = checked_add(self.input_tokens, record.input_tokens)?;
        self.output_tokens = checked_add(self.output_tokens, record.output_tokens)?;
        self.cache_creation_tokens =
            checked_add(self.cache_creation_tokens, record.cache_write_tokens)?;
        self.cache_read_tokens = checked_add(self.cache_read_tokens, record.cache_read_tokens)?;
        Ok(())
    }

    const fn is_empty(&self) -> bool {
        self.input_tokens == 0
            && self.output_tokens == 0
            && self.cache_creation_tokens == 0
            && self.cache_read_tokens == 0
    }

    fn tokens(&self) -> Result<TokenUsage, AntigravityMappingError> {
        let total = checked_add(
            checked_add(
                checked_add(self.input_tokens, self.output_tokens)?,
                self.cache_creation_tokens,
            )?,
            self.cache_read_tokens,
        )?;
        TokenUsage::new(
            Some(self.input_tokens),
            Some(self.output_tokens),
            Some(self.cache_creation_tokens),
            Some(self.cache_read_tokens),
            total,
        )
        .map_err(Into::into)
    }
}

fn checked_add(left: u64, right: u64) -> Result<u64, AntigravityMappingError> {
    checked_add_u64(left, right, AntigravityMappingError::TokenOverflow)
}

fn cost(model: &str, tokens: &TokenUsage, calculator: &BurnlyCostCalculator) -> UsageCost {
    calculator.calculate(model, tokens).cost
}

/// Daily/session aggregate cost: the sum of per-model valued micros when
/// breakdowns exist; otherwise price the aggregate tokens with no model.
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

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub(crate) enum AntigravityMappingError {
    #[error("antigravity mapping requires a collector version")]
    EmptyCollectorVersion,
    #[error("antigravity mapping requires a valid timezone")]
    InvalidTimezone,
    #[error("antigravity token total overflowed")]
    TokenOverflow,
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error(transparent)]
    Usage(#[from] UsageValidationError),
}

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, TimeZone};

    use super::*;
    use crate::infrastructure::collectors::antigravity::product_variant::AntigravityProductVariant;

    #[test]
    fn maps_records_to_daily_usage_by_modified_date_and_model_label() {
        let candidates = map_daily(
            conversations(),
            "Asia/Jakarta",
            &CollectionScope::Full,
            &context(),
            &calculator(),
        )
        .expect("daily candidates");

        assert_eq!(candidates.len(), 1);
        let candidate = &candidates[0];
        assert_eq!(
            candidate.source_key,
            "antigravity:daily:v1:Asia/Jakarta:2026-07-02"
        );
        assert_eq!(candidate.tokens.input_tokens(), Some(180));
        assert_eq!(candidate.tokens.output_tokens(), Some(50));
        assert_eq!(candidate.tokens.cache_creation_tokens(), Some(3));
        assert_eq!(candidate.tokens.cache_read_tokens(), Some(12));
        assert_eq!(candidate.tokens.total_tokens(), 245);
        assert_eq!(candidate.model_breakdowns.len(), 2);

        let pro = candidate
            .model_breakdowns
            .iter()
            .find(|model| model.raw_model_id == "Gemini 3.1 Pro (High)")
            .expect("pro model");
        assert_eq!(pro.tokens.total_tokens(), 138);
    }

    #[test]
    fn maps_records_to_session_usage_by_conversation_variant() {
        let candidates =
            map_sessions(conversations(), &context(), &calculator()).expect("session candidates");

        assert_eq!(candidates.len(), 2);
        assert!(candidates.iter().any(|candidate| {
            candidate.source_session_id == "antigravity:app-conversation"
                && candidate.tokens.total_tokens() == 138
        }));
        assert!(candidates.iter().any(|candidate| {
            candidate.source_session_id == "antigravity-ide:ide-conversation"
                && candidate.tokens.total_tokens() == 107
        }));
    }

    #[test]
    fn applies_incremental_daily_scope() {
        let candidates = map_daily(
            conversations(),
            "UTC",
            &CollectionScope::incremental(
                NaiveDate::from_ymd_opt(2026, 7, 3).expect("date"),
                NaiveDate::from_ymd_opt(2026, 7, 3).expect("date"),
            )
            .expect("scope"),
            &context(),
            &calculator(),
        )
        .expect("daily candidates");

        assert!(candidates.is_empty());
    }

    #[test]
    fn maps_daily_usage_by_record_observed_date_when_available() {
        let mut conversations = conversations();
        conversations[0].database.modified_at = Utc
            .with_ymd_and_hms(2026, 7, 5, 18, 0, 0)
            .single()
            .expect("timestamp");
        conversations[0].records[0].observed_at = Some(
            Utc.with_ymd_and_hms(2026, 7, 1, 18, 0, 0)
                .single()
                .expect("timestamp"),
        );
        conversations.truncate(1);

        let candidates = map_daily(
            conversations,
            "UTC",
            &CollectionScope::incremental(
                NaiveDate::from_ymd_opt(2026, 7, 1).expect("date"),
                NaiveDate::from_ymd_opt(2026, 7, 1).expect("date"),
            )
            .expect("scope"),
            &context(),
            &calculator(),
        )
        .expect("daily candidates");

        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0].source_key,
            "antigravity:daily:v1:UTC:2026-07-01"
        );
    }

    fn conversations() -> Vec<ConversationUsage> {
        vec![
            ConversationUsage {
                database: database(
                    AntigravityProductVariant::App,
                    "app-conversation",
                    Utc.with_ymd_and_hms(2026, 7, 1, 18, 0, 0)
                        .single()
                        .expect("timestamp"),
                ),
                records: vec![record(
                    AntigravityProductVariant::App,
                    "app-conversation",
                    "MODEL_PLACEHOLDER_M16",
                    "Gemini 3.1 Pro (High)",
                    100,
                    30,
                    5,
                    3,
                )],
            },
            ConversationUsage {
                database: database(
                    AntigravityProductVariant::Ide,
                    "ide-conversation",
                    Utc.with_ymd_and_hms(2026, 7, 1, 19, 0, 0)
                        .single()
                        .expect("timestamp"),
                ),
                records: vec![record(
                    AntigravityProductVariant::Ide,
                    "ide-conversation",
                    "gemini-flash",
                    "gemini-flash",
                    80,
                    20,
                    7,
                    0,
                )],
            },
        ]
    }

    fn database(
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

    #[allow(
        clippy::too_many_arguments,
        reason = "test fixture keeps token fields explicit"
    )]
    fn record(
        variant: AntigravityProductVariant,
        conversation_id: &str,
        raw_model_id: &str,
        model_label: &str,
        input_tokens: u64,
        output_tokens: u64,
        cache_read_tokens: u64,
        cache_write_tokens: u64,
    ) -> AntigravityUsageRecord {
        AntigravityUsageRecord {
            variant,
            conversation_id: conversation_id.to_owned(),
            raw_model_id: raw_model_id.to_owned(),
            model_label: model_label.to_owned(),
            api_provider: Some("API_PROVIDER_GOOGLE_GEMINI".to_owned()),
            response_id: Some(format!("{conversation_id}:{raw_model_id}")),
            observed_at: None,
            input_tokens,
            output_tokens,
            thinking_output_tokens: 0,
            response_output_tokens: output_tokens,
            cache_read_tokens,
            cache_write_tokens,
            consumed_credits: None,
            flow_credits_used: None,
            prompt_credits_used: None,
        }
    }

    fn context() -> AntigravityMappingContext {
        AntigravityMappingContext::new(
            CollectorKey::new("antigravity").expect("collector key"),
            "local-rpc".to_owned(),
            CollectionId::new("antigravity-test").expect("collection id"),
            Utc.with_ymd_and_hms(2026, 7, 2, 8, 0, 0)
                .single()
                .expect("timestamp"),
        )
        .expect("context")
    }

    fn calculator() -> BurnlyCostCalculator {
        BurnlyCostCalculator::new()
    }
}
