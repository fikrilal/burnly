use chrono::{DateTime, NaiveDate, Utc};
use thiserror::Error;

use crate::{
    application::collection::{
        CandidateProvenance, CollectionId, CollectorKey, DailyUsageCandidate, ModelUsageCandidate,
    },
    domain::{
        source::SourceKey,
        usage::{
            CostKind, CurrencyCode, DataQuality, TokenUsage, UsageCost, UsageValidationError,
            ValuedCostStatus,
        },
    },
};

use super::envelopes::claude_daily::{ClaudeDailyReport, ClaudeDailyRow, ModelBreakdown};

const IDENTITY_VERSION: u16 = 1;
const COST_MICROS_PER_UNIT: f64 = 1_000_000.0;

pub(crate) fn map_daily(
    report: ClaudeDailyReport,
    context: MappingContext,
) -> Result<Vec<DailyUsageCandidate>, MappingError> {
    report
        .daily
        .into_iter()
        .map(|row| map_row(row, &context))
        .collect()
}

fn map_row(
    row: ClaudeDailyRow,
    context: &MappingContext,
) -> Result<DailyUsageCandidate, MappingError> {
    let usage_date =
        NaiveDate::parse_from_str(&row.date, "%Y-%m-%d").map_err(|_| MappingError::InvalidDate)?;
    let tokens = TokenUsage::new(
        Some(row.input_tokens),
        Some(row.output_tokens),
        Some(row.cache_creation_tokens),
        Some(row.cache_read_tokens),
        row.total_tokens,
    )?;
    let cost = map_cost(row.total_cost, row.total_tokens)?;
    let model_breakdowns = row
        .model_breakdowns
        .into_iter()
        .map(map_model)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(DailyUsageCandidate {
        provenance: context.provenance(),
        source_key: daily_source_key(usage_date),
        usage_date,
        aggregation_timezone: context.aggregation_timezone.clone(),
        tokens,
        cost,
        model_breakdowns,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MappingContext {
    collector: CollectorKey,
    collector_version: String,
    profile_version: u16,
    collection_id: CollectionId,
    observed_at: DateTime<Utc>,
    aggregation_timezone: String,
}

impl MappingContext {
    pub(crate) fn new(
        collector: CollectorKey,
        collector_version: String,
        profile_version: u16,
        collection_id: CollectionId,
        observed_at: DateTime<Utc>,
        aggregation_timezone: String,
    ) -> Result<Self, MappingError> {
        if collector_version.trim().is_empty() {
            return Err(MappingError::EmptyCollectorVersion);
        }
        if profile_version == 0 {
            return Err(MappingError::InvalidProfileVersion);
        }
        if aggregation_timezone.trim().is_empty() {
            return Err(MappingError::EmptyAggregationTimezone);
        }
        Ok(Self {
            collector,
            collector_version,
            profile_version,
            collection_id,
            observed_at,
            aggregation_timezone,
        })
    }

    fn provenance(&self) -> CandidateProvenance {
        CandidateProvenance {
            source: SourceKey::ClaudeCode,
            collector: self.collector.clone(),
            collector_version: self.collector_version.clone(),
            profile_version: self.profile_version,
            collection_id: self.collection_id.clone(),
            observed_at: self.observed_at,
            data_quality: DataQuality::Complete,
            warnings: Vec::new(),
        }
    }
}

fn map_model(model: ModelBreakdown) -> Result<ModelUsageCandidate, MappingError> {
    let total_tokens = model
        .input_tokens
        .checked_add(model.output_tokens)
        .and_then(|value| value.checked_add(model.cache_creation_tokens))
        .and_then(|value| value.checked_add(model.cache_read_tokens))
        .ok_or(MappingError::TokenOverflow)?;
    Ok(ModelUsageCandidate {
        raw_model_id: model.model_name,
        tokens: TokenUsage::new(
            Some(model.input_tokens),
            Some(model.output_tokens),
            Some(model.cache_creation_tokens),
            Some(model.cache_read_tokens),
            total_tokens,
        )?,
        cost: map_cost(model.cost, total_tokens)?,
    })
}

fn map_cost(value: f64, total_tokens: u64) -> Result<UsageCost, MappingError> {
    if !value.is_finite() || value < 0.0 {
        return Err(MappingError::InvalidCost);
    }
    if value == 0.0 {
        return Ok(if total_tokens == 0 {
            UsageCost::NotApplicable {
                kind: CostKind::CollectorCalculated,
            }
        } else {
            UsageCost::Unavailable {
                kind: CostKind::CollectorCalculated,
            }
        });
    }

    let micros = value * COST_MICROS_PER_UNIT;
    if !micros.is_finite() || micros.round() > u64::MAX as f64 {
        return Err(MappingError::CostOutOfRange);
    }
    Ok(UsageCost::Valued {
        amount_micros: micros.round() as u64,
        currency: CurrencyCode::new("USD").expect("USD is a valid ISO-shaped currency"),
        kind: CostKind::CollectorCalculated,
        status: ValuedCostStatus::Estimated,
    })
}

fn daily_source_key(date: NaiveDate) -> String {
    format!("daily:v{IDENTITY_VERSION}:{date}")
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub(crate) enum MappingError {
    #[error("Claude daily mapping requires a collector version")]
    EmptyCollectorVersion,
    #[error("Claude daily mapping requires a positive profile version")]
    InvalidProfileVersion,
    #[error("Claude daily mapping requires an aggregation timezone")]
    EmptyAggregationTimezone,
    #[error("Claude daily mapping received an invalid date")]
    InvalidDate,
    #[error("Claude daily model token total overflowed")]
    TokenOverflow,
    #[error("Claude daily mapping received an invalid cost")]
    InvalidCost,
    #[error("Claude daily cost exceeded the supported micro-unit range")]
    CostOutOfRange,
    #[error(transparent)]
    Usage(#[from] UsageValidationError),
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;
    use crate::infrastructure::collectors::ccusage::envelopes::claude_daily::decode;

    const VALID: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/collectors/ccusage/claude-daily/valid.json"
    ));
    const EMPTY: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/collectors/ccusage/claude-daily/empty.json"
    ));
    const ADDITIVE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/collectors/ccusage/claude-daily/additive-fields.json"
    ));

    #[test]
    fn maps_authoritative_daily_usage_with_deterministic_identity() {
        let candidates = map_daily(
            decode(VALID).expect("decoded fixture"),
            context("Asia/Jakarta"),
        )
        .expect("mapped candidates");

        let first = &candidates[0];
        assert_eq!(first.source_key, "daily:v1:2026-06-13");
        assert_eq!(first.aggregation_timezone, "Asia/Jakarta");
        assert_eq!(first.tokens.total_tokens(), 1_650);
        assert_eq!(first.tokens.unclassified_tokens(), Some(0));
        assert_eq!(first.model_breakdowns.len(), 2);
        assert_eq!(first.model_breakdowns[0].raw_model_id, "claude-sonnet-4");
        assert_eq!(
            first.cost,
            UsageCost::Valued {
                amount_micros: 420_000,
                currency: CurrencyCode::new("USD").expect("currency"),
                kind: CostKind::CollectorCalculated,
                status: ValuedCostStatus::Estimated,
            }
        );
        assert_eq!(first.provenance.data_quality, DataQuality::Complete);
        assert!(first.provenance.warnings.is_empty());

        let additive = map_daily(
            decode(ADDITIVE).expect("decoded additive fixture"),
            context("UTC"),
        )
        .expect("mapped additive fixture");
        assert_eq!(additive[0].tokens.total_tokens(), 20);
        assert_eq!(additive[0].tokens.unclassified_tokens(), Some(2));
    }

    #[test]
    fn preserves_empty_collection_and_zero_cost_semantics() {
        assert!(
            map_daily(decode(EMPTY).expect("decoded fixture"), context("UTC"),)
                .expect("mapped empty report")
                .is_empty()
        );

        assert_eq!(
            map_cost(0.0, 0).expect("zero usage cost"),
            UsageCost::NotApplicable {
                kind: CostKind::CollectorCalculated
            }
        );
        assert_eq!(
            map_cost(0.0, 1).expect("missing pricing"),
            UsageCost::Unavailable {
                kind: CostKind::CollectorCalculated
            }
        );
    }

    #[test]
    fn rejects_invalid_mapping_context_and_values() {
        assert_eq!(
            build_context("20.0.11", 1, " ").expect_err("empty timezone"),
            MappingError::EmptyAggregationTimezone
        );
        assert_eq!(
            build_context(" ", 1, "UTC").expect_err("empty version"),
            MappingError::EmptyCollectorVersion
        );
        assert_eq!(
            build_context("20.0.11", 0, "UTC").expect_err("invalid profile"),
            MappingError::InvalidProfileVersion
        );
        assert_eq!(map_cost(-0.1, 1), Err(MappingError::InvalidCost));
        assert_eq!(map_cost(f64::MAX, 1), Err(MappingError::CostOutOfRange));
        assert_eq!(
            map_model(ModelBreakdown {
                model_name: "claude-sonnet-4".to_owned(),
                input_tokens: u64::MAX,
                output_tokens: 1,
                cache_creation_tokens: 0,
                cache_read_tokens: 0,
                cost: 1.0,
            }),
            Err(MappingError::TokenOverflow)
        );
    }

    fn context(timezone: &str) -> MappingContext {
        build_context("20.0.11", 1, timezone).expect("mapping context")
    }

    fn build_context(
        collector_version: &str,
        profile_version: u16,
        timezone: &str,
    ) -> Result<MappingContext, MappingError> {
        MappingContext::new(
            CollectorKey::new("ccusage").expect("collector key"),
            collector_version.to_owned(),
            profile_version,
            CollectionId::new("collection-1").expect("collection id"),
            Utc.with_ymd_and_hms(2026, 6, 14, 12, 0, 0)
                .single()
                .expect("timestamp"),
            timezone.to_owned(),
        )
    }
}
