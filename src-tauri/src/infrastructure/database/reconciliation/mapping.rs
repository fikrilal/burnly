//! Token, cost, and quality mapping helpers shared by daily and session
//! reconciliation.

use crate::application::collection::{CollectionOutcome, CollectionScope};
use crate::application::ports::usage_store::UsageStoreError;
use crate::domain::usage::{CostKind, DataQuality, TokenUsage, UsageCost, ValuedCostStatus};

pub(super) struct TokenColumns {
    pub(super) input: Option<i64>,
    pub(super) output: Option<i64>,
    pub(super) cache_creation: Option<i64>,
    pub(super) cache_read: Option<i64>,
    pub(super) total: i64,
    pub(super) unclassified: Option<i64>,
}

pub(super) fn token_columns(tokens: &TokenUsage) -> Result<TokenColumns, UsageStoreError> {
    Ok(TokenColumns {
        input: optional_token(tokens.input_tokens())?,
        output: optional_token(tokens.output_tokens())?,
        cache_creation: optional_token(tokens.cache_creation_tokens())?,
        cache_read: optional_token(tokens.cache_read_tokens())?,
        total: token_value(tokens.total_tokens())?,
        unclassified: optional_token(tokens.unclassified_tokens())?,
    })
}

fn token_value(value: u64) -> Result<i64, UsageStoreError> {
    i64::try_from(value).map_err(|_| UsageStoreError::ValueOutOfRange)
}

fn optional_token(value: Option<u64>) -> Result<Option<i64>, UsageStoreError> {
    value.map(token_value).transpose()
}

pub(super) struct DailyCostColumns {
    pub(super) amount_micros: Option<i64>,
    pub(super) currency: Option<String>,
    pub(super) kind: &'static str,
    pub(super) status: &'static str,
}

pub(super) fn daily_cost_columns(cost: &UsageCost) -> Result<DailyCostColumns, UsageStoreError> {
    Ok(match cost {
        UsageCost::Valued {
            amount_micros,
            currency,
            kind,
            status,
        } => DailyCostColumns {
            amount_micros: Some(token_value(*amount_micros)?),
            currency: Some(currency.as_str().to_owned()),
            kind: cost_kind_value(*kind),
            status: valued_status_value(*status),
        },
        UsageCost::NotApplicable { kind } => DailyCostColumns {
            amount_micros: None,
            currency: None,
            kind: cost_kind_value(*kind),
            status: "not_applicable",
        },
        UsageCost::Unavailable { kind } => DailyCostColumns {
            amount_micros: None,
            currency: None,
            kind: cost_kind_value(*kind),
            status: "unavailable",
        },
    })
}

pub(super) struct ModelCostColumns {
    pub(super) amount_micros: Option<i64>,
    pub(super) currency: Option<String>,
    pub(super) status: &'static str,
}

pub(super) fn model_cost_columns(cost: &UsageCost) -> Result<ModelCostColumns, UsageStoreError> {
    Ok(match cost {
        UsageCost::Valued {
            amount_micros,
            currency,
            ..
        } => ModelCostColumns {
            amount_micros: Some(token_value(*amount_micros)?),
            currency: Some(currency.as_str().to_owned()),
            status: "estimated",
        },
        UsageCost::NotApplicable { .. } | UsageCost::Unavailable { .. } => ModelCostColumns {
            amount_micros: None,
            currency: None,
            status: "unavailable",
        },
    })
}

const fn cost_kind_value(kind: CostKind) -> &'static str {
    match kind {
        CostKind::SourceReported => "source_reported",
        CostKind::CollectorCalculated => "collector_calculated",
        CostKind::CollectorMixed => "collector_mixed",
        CostKind::BurnlyCalculated => "burnly_calculated",
        CostKind::Unknown => "unknown",
    }
}

const fn valued_status_value(status: ValuedCostStatus) -> &'static str {
    match status {
        ValuedCostStatus::Available => "available",
        ValuedCostStatus::Estimated => "estimated",
    }
}

pub(super) const fn data_quality_value(quality: DataQuality) -> &'static str {
    match quality {
        DataQuality::Complete => "complete",
        DataQuality::Partial => "partial",
    }
}

/// Absence advances only on a successful full-scope import. Partial imports may
/// be missing records for transient reasons, and incremental imports do not
/// describe the full set of days, so neither may remove records.
pub(super) fn should_evaluate_absence(scope: &CollectionScope, outcome: CollectionOutcome) -> bool {
    matches!(scope, CollectionScope::Full) && !matches!(outcome, CollectionOutcome::Partial)
}
