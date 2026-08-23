use chrono::{DateTime, NaiveDate, Utc};
use thiserror::Error;

use crate::{
    application::collection::{
        CandidateProvenance, CollectionId, CollectorKey, DailyUsageCandidate, ModelUsageCandidate,
        SessionUsageCandidate,
    },
    domain::{
        identity::{daily_source_key, session_source_key, IdentityError},
        source::SourceKey,
        usage::{
            CostKind, CurrencyCode, DataQuality, TokenUsage, UsageCost, UsageValidationError,
            ValuedCostStatus,
        },
    },
};

use super::envelopes::claude_daily::{ClaudeDailyReport, ClaudeDailyRow, ModelBreakdown};
use super::envelopes::claude_session::{ClaudeSessionReport, ClaudeSessionRow};
use super::envelopes::codex_daily::{CodexDailyReport, CodexDailyRow, CodexModelBreakdown};
use super::envelopes::codex_session::{
    first_activity_timestamp as codex_first_activity_timestamp,
    last_activity_timestamp as codex_last_activity_timestamp, CodexSessionReport, CodexSessionRow,
};
use super::envelopes::pi_daily::{PiDailyReport, PiDailyRow, PiModelBreakdown};
use super::envelopes::pi_session::{PiSessionReport, PiSessionRow};

const COST_MICROS_PER_UNIT: f64 = 1_000_000.0;

/// Stable model identity for Pi days/sessions that used several models with
/// no per-model token split available from the collector. Keeping it stable lets
/// the usage stay visible and reconcile with the daily total across every view.
const PI_MULTIPLE_MODELS: &str = "Multiple models";

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
        source_key: daily_source_key(context.source, usage_date, &context.aggregation_timezone)?,
        usage_date,
        aggregation_timezone: context.aggregation_timezone.clone(),
        tokens,
        cost,
        model_breakdowns,
    })
}

pub(crate) fn map_session(
    report: ClaudeSessionReport,
    context: MappingContext,
) -> Result<Vec<SessionUsageCandidate>, MappingError> {
    report
        .sessions
        .into_iter()
        .map(|row| map_session_row(row, &context))
        .collect()
}

fn map_session_row(
    row: ClaudeSessionRow,
    context: &MappingContext,
) -> Result<SessionUsageCandidate, MappingError> {
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

    let first_activity_at = DateTime::parse_from_rfc3339(&row.first_activity)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|_| MappingError::InvalidDate)?;

    let last_activity_at = DateTime::parse_from_rfc3339(&row.last_activity)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|_| MappingError::InvalidDate)?;

    let project_path = row.project_path.filter(|path| !path.trim().is_empty());

    Ok(SessionUsageCandidate {
        provenance: context.provenance(),
        source_key: session_source_key(context.source, &row.session_id)?,
        source_session_id: row.session_id,
        project_path,
        first_activity_at: Some(first_activity_at),
        last_activity_at: Some(last_activity_at),
        tokens,
        cost,
        model_breakdowns,
    })
}

pub(crate) fn map_codex_daily(
    report: CodexDailyReport,
    context: MappingContext,
) -> Result<Vec<DailyUsageCandidate>, MappingError> {
    report
        .daily
        .into_iter()
        .map(|row| map_codex_row(row, &context))
        .collect()
}

fn map_codex_row(
    row: CodexDailyRow,
    context: &MappingContext,
) -> Result<DailyUsageCandidate, MappingError> {
    let usage_date =
        NaiveDate::parse_from_str(&row.date, "%Y-%m-%d").map_err(|_| MappingError::InvalidDate)?;
    let tokens = TokenUsage::new(
        Some(row.input_tokens),
        Some(row.output_tokens),
        row.cache_creation_tokens,
        row.cache_read_tokens,
        row.total_tokens,
    )?;
    let cost = map_cost(row.total_cost, row.total_tokens)?;
    let model_breakdowns = row
        .models
        .into_iter()
        .map(|(model_name, model)| map_codex_model(model_name, model))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(DailyUsageCandidate {
        provenance: context.provenance(),
        source_key: daily_source_key(context.source, usage_date, &context.aggregation_timezone)?,
        usage_date,
        aggregation_timezone: context.aggregation_timezone.clone(),
        tokens,
        cost,
        model_breakdowns,
    })
}

fn map_codex_model(
    model_name: String,
    model: CodexModelBreakdown,
) -> Result<ModelUsageCandidate, MappingError> {
    let total_tokens = model
        .input_tokens
        .checked_add(model.output_tokens)
        .and_then(|value| value.checked_add(model.cache_creation_tokens.unwrap_or(0)))
        .and_then(|value| value.checked_add(model.cache_read_tokens.unwrap_or(0)))
        .and_then(|value| value.checked_add(model.reasoning_output_tokens))
        .ok_or(MappingError::TokenOverflow)?;
    let total_tokens = model.total_tokens.unwrap_or(total_tokens);
    Ok(ModelUsageCandidate {
        raw_model_id: model_name,
        tokens: TokenUsage::new(
            Some(model.input_tokens),
            Some(model.output_tokens),
            model.cache_creation_tokens,
            model.cache_read_tokens,
            total_tokens,
        )?,
        cost: map_optional_cost(model.cost, total_tokens)?,
    })
}

pub(crate) fn map_codex_session(
    report: CodexSessionReport,
    context: MappingContext,
) -> Result<Vec<SessionUsageCandidate>, MappingError> {
    report
        .sessions
        .into_iter()
        .map(|row| map_codex_session_row(row, &context))
        .collect()
}

fn map_codex_session_row(
    row: CodexSessionRow,
    context: &MappingContext,
) -> Result<SessionUsageCandidate, MappingError> {
    let tokens = TokenUsage::new(
        Some(row.input_tokens),
        Some(row.output_tokens),
        row.cache_creation_tokens,
        row.cache_read_tokens,
        row.total_tokens,
    )?;
    let cost = map_cost(row.total_cost, row.total_tokens)?;
    let first_activity_at = codex_first_activity_timestamp(&row)
        .map(parse_timestamp)
        .transpose()?;
    let last_activity_at = codex_last_activity_timestamp(&row)
        .as_deref()
        .map(parse_timestamp)
        .transpose()?;
    let model_breakdowns = row
        .models
        .into_iter()
        .map(|(model_name, model)| map_codex_model(model_name, model))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(SessionUsageCandidate {
        provenance: context.provenance(),
        source_key: session_source_key(context.source, &row.session_id)?,
        source_session_id: row.session_id,
        project_path: if row.directory.trim().is_empty() {
            None
        } else {
            Some(row.directory)
        },
        first_activity_at,
        last_activity_at,
        tokens,
        cost,
        model_breakdowns,
    })
}

pub(crate) fn map_pi_daily(
    report: PiDailyReport,
    context: MappingContext,
) -> Result<Vec<DailyUsageCandidate>, MappingError> {
    report
        .daily
        .into_iter()
        .map(|row| map_pi_daily_row(row, &context))
        .collect()
}

fn map_pi_daily_row(
    row: PiDailyRow,
    context: &MappingContext,
) -> Result<DailyUsageCandidate, MappingError> {
    let usage_date =
        NaiveDate::parse_from_str(&row.date, "%Y-%m-%d").map_err(|_| MappingError::InvalidDate)?;
    let tokens = TokenUsage::new(
        Some(row.input_tokens),
        Some(row.output_tokens),
        row.cache_creation_tokens,
        row.cache_read_tokens,
        row.total_tokens,
    )?;
    let cost = map_cost(row.total_cost, row.total_tokens)?;
    let model_breakdowns =
        pi_model_breakdowns(row.model_breakdowns, row.models_used, &tokens, &cost)?;
    Ok(DailyUsageCandidate {
        provenance: context.provenance(),
        source_key: daily_source_key(context.source, usage_date, &context.aggregation_timezone)?,
        usage_date,
        aggregation_timezone: context.aggregation_timezone.clone(),
        tokens,
        cost,
        model_breakdowns,
    })
}

fn map_pi_model(model: PiModelBreakdown) -> Result<ModelUsageCandidate, MappingError> {
    let total_tokens = model
        .input_tokens
        .checked_add(model.output_tokens)
        .and_then(|value| value.checked_add(model.cache_creation_tokens.unwrap_or(0)))
        .and_then(|value| value.checked_add(model.cache_read_tokens.unwrap_or(0)))
        .ok_or(MappingError::TokenOverflow)?;
    let total_tokens = model.total_tokens.unwrap_or(total_tokens);
    Ok(ModelUsageCandidate {
        raw_model_id: model.model_name,
        tokens: TokenUsage::new(
            Some(model.input_tokens),
            Some(model.output_tokens),
            model.cache_creation_tokens,
            model.cache_read_tokens,
            total_tokens,
        )?,
        cost: map_cost(model.cost, total_tokens)?,
    })
}

/// Pi's ccusage adapter reports each day's/session's totals plus the set of
/// model names used, but never a per-model token split (even with `--breakdown`),
/// and its sessions may carry no timestamps to reconstruct a daily split from.
/// Because Pi aggregates the whole period, the split is either fully recoverable or
/// not at all:
/// - one model used: the whole total provably belongs to it, attributed exactly;
/// - several models used: the tokens cannot be divided, so they are kept together
///   under a single stable "Multiple models" entry rather than fabricating a split
///   or dropping the usage (which would desync the per-model view from the total).
fn pi_model_breakdowns(
    explicit: Vec<PiModelBreakdown>,
    models_used: Vec<String>,
    row_tokens: &TokenUsage,
    row_cost: &UsageCost,
) -> Result<Vec<ModelUsageCandidate>, MappingError> {
    if !explicit.is_empty() {
        return explicit.into_iter().map(map_pi_model).collect();
    }
    let raw_model_id = match models_used.as_slice() {
        [] => return Ok(Vec::new()),
        [model_name] => model_name.clone(),
        _ => PI_MULTIPLE_MODELS.to_owned(),
    };
    Ok(vec![ModelUsageCandidate {
        raw_model_id,
        tokens: row_tokens.clone(),
        cost: row_cost.clone(),
    }])
}

/// Pi sessions carry only `modelsUsed` (no per-model split), so they reuse the
/// aggregate-label policy. Pi `projectPath` is intentionally not persisted,
/// matching the fixture privacy harness.
pub(crate) fn map_pi_session(
    report: PiSessionReport,
    context: MappingContext,
) -> Result<Vec<SessionUsageCandidate>, MappingError> {
    report
        .sessions
        .into_iter()
        .map(|row| map_pi_session_row(row, &context))
        .collect()
}

fn map_pi_session_row(
    row: PiSessionRow,
    context: &MappingContext,
) -> Result<SessionUsageCandidate, MappingError> {
    let tokens = TokenUsage::new(
        Some(row.input_tokens),
        Some(row.output_tokens),
        row.cache_creation_tokens,
        row.cache_read_tokens,
        row.total_tokens,
    )?;
    let cost = map_cost(row.total_cost, row.total_tokens)?;
    let model_breakdowns = pi_model_breakdowns(Vec::new(), row.models_used, &tokens, &cost)?;

    let first_activity_at = row
        .first_activity
        .as_deref()
        .map(parse_timestamp)
        .transpose()?;

    let last_activity_at = row
        .last_activity
        .as_deref()
        .map(parse_timestamp)
        .transpose()?;

    Ok(SessionUsageCandidate {
        provenance: context.provenance(),
        source_key: session_source_key(context.source, &row.session_id)?,
        source_session_id: row.session_id,
        project_path: None,
        first_activity_at,
        last_activity_at,
        tokens,
        cost,
        model_breakdowns,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MappingContext {
    source: SourceKey,
    collector: CollectorKey,
    collector_version: String,
    profile_version: u16,
    collection_id: CollectionId,
    observed_at: DateTime<Utc>,
    aggregation_timezone: String,
}

impl MappingContext {
    pub(crate) fn new(
        source: SourceKey,
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
            source,
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
            source: self.source,
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

fn map_optional_cost(value: Option<f64>, total_tokens: u64) -> Result<UsageCost, MappingError> {
    value.map_or_else(
        || {
            Ok(if total_tokens == 0 {
                UsageCost::NotApplicable {
                    kind: CostKind::CollectorCalculated,
                }
            } else {
                UsageCost::Unavailable {
                    kind: CostKind::CollectorCalculated,
                }
            })
        },
        |cost| map_cost(cost, total_tokens),
    )
}

fn parse_timestamp(value: &str) -> Result<DateTime<Utc>, MappingError> {
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|_| MappingError::InvalidDate)
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
    Identity(#[from] IdentityError),
    #[error(transparent)]
    Usage(#[from] UsageValidationError),
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;
    use crate::application::collection::CollectorFailureCode;
    use crate::infrastructure::collectors::ccusage::envelopes::claude_daily::decode;
    use crate::infrastructure::collectors::ccusage::envelopes::claude_session::decode as decode_claude_session;
    use crate::infrastructure::collectors::ccusage::envelopes::codex_daily::decode as decode_codex_daily;
    use crate::infrastructure::collectors::ccusage::envelopes::codex_session::decode as decode_codex_session;
    use crate::infrastructure::collectors::ccusage::envelopes::pi_daily::decode as decode_pi_daily;
    use crate::infrastructure::collectors::ccusage::envelopes::pi_session::decode as decode_pi_session;

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
    const CLAUDE_SESSION_VALID: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/collectors/ccusage/claude-session/valid.json"
    ));
    const CLAUDE_SESSION_EMPTY: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/collectors/ccusage/claude-session/empty.json"
    ));
    const CLAUDE_SESSION_REAL_SHAPE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/collectors/ccusage/claude-session/real-shape.json"
    ));
    const CODEX_DAILY_VALID: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/collectors/ccusage/codex-daily/valid.json"
    ));
    const CODEX_DAILY_REAL_SHAPE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/collectors/ccusage/codex-daily/real-shape.json"
    ));
    const CODEX_SESSION_VALID: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/collectors/ccusage/codex-session/valid.json"
    ));
    const CODEX_SESSION_REAL_SHAPE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/collectors/ccusage/codex-session/real-shape.json"
    ));
    const PI_DAILY_VALID: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/collectors/ccusage/pi-daily/valid.json"
    ));
    const PI_DAILY_EMPTY: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/collectors/ccusage/pi-daily/empty.json"
    ));
    const PI_DAILY_REAL_SHAPE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/collectors/ccusage/pi-daily/real-shape.json"
    ));
    const PI_DAILY_INVALID_JSON: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/collectors/ccusage/pi-daily/invalid-json.json"
    ));
    const PI_DAILY_INCOMPATIBLE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/collectors/ccusage/pi-daily/incompatible-envelope.json"
    ));
    const PI_SESSION_VALID: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/collectors/ccusage/pi-session/valid.json"
    ));
    const PI_SESSION_EMPTY: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/collectors/ccusage/pi-session/empty.json"
    ));
    const PI_SESSION_REAL_SHAPE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/collectors/ccusage/pi-session/real-shape.json"
    ));

    #[test]
    fn maps_authoritative_daily_usage_with_deterministic_identity() {
        let candidates = map_daily(
            decode(VALID).expect("decoded fixture"),
            context("Asia/Jakarta"),
        )
        .expect("mapped candidates");

        let first = &candidates[0];
        assert_eq!(
            first.source_key,
            "claude-code:daily:v1:Asia/Jakarta:2026-06-13"
        );
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
    fn maps_claude_session_real_shape_with_deterministic_identity() {
        let context = build_context(SourceKey::ClaudeCode, "20.0.19", 1, "UTC").expect("context");
        let candidates = map_session(
            decode_claude_session(CLAUDE_SESSION_REAL_SHAPE).expect("decoded real-shape"),
            context,
        )
        .expect("mapped real-shape");

        assert_eq!(candidates.len(), 3);
        let first = &candidates[0];
        assert_eq!(first.source_key, "claude-code:session:v1:session-1");
        assert_eq!(first.source_session_id, "session-1");
        assert_eq!(first.project_path, None);
        assert_eq!(first.tokens.total_tokens(), 6_249_672);
        assert_eq!(first.tokens.input_tokens(), Some(353));
        assert_eq!(first.tokens.output_tokens(), Some(104_423));
        assert_eq!(first.model_breakdowns.len(), 1);
        assert_eq!(first.model_breakdowns[0].raw_model_id, "claude-sonnet-4-6");
        assert_eq!(
            first.first_activity_at,
            Some(
                Utc.with_ymd_and_hms(2026, 5, 8, 12, 46, 44)
                    .single()
                    .expect("timestamp")
                    + chrono::Duration::milliseconds(17)
            )
        );
        assert_eq!(
            first.last_activity_at,
            Some(
                Utc.with_ymd_and_hms(2026, 5, 8, 14, 49, 34)
                    .single()
                    .expect("timestamp")
                    + chrono::Duration::milliseconds(353)
            )
        );
    }

    #[test]
    fn maps_claude_session_valid_and_empty_fixtures() {
        let context = build_context(SourceKey::ClaudeCode, "20.0.19", 1, "UTC").expect("context");
        let candidates = map_session(
            decode_claude_session(CLAUDE_SESSION_VALID).expect("decoded valid"),
            context.clone(),
        )
        .expect("mapped valid");
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].source_key, "claude-code:session:v1:session-1");
        assert_eq!(candidates[0].tokens.total_tokens(), 1_650);
        assert_eq!(candidates[0].model_breakdowns.len(), 2);

        assert!(map_session(
            decode_claude_session(CLAUDE_SESSION_EMPTY).expect("decoded empty"),
            context,
        )
        .expect("mapped empty")
        .is_empty());
    }

    #[test]
    fn maps_claude_session_project_path_when_present() {
        let input = r#"{
          "sessions": [{
            "sessionId": "path-1",
            "firstActivity": "2026-06-13T10:00:00.000Z",
            "lastActivity": "2026-06-13T11:00:00.000Z",
            "inputTokens": 10,
            "outputTokens": 5,
            "cacheCreationTokens": 0,
            "cacheReadTokens": 0,
            "totalTokens": 15,
            "totalCost": 0.01,
            "modelsUsed": ["claude-sonnet-4"],
            "modelBreakdowns": [],
            "projectPath": "/tmp/sanitized-project"
          }],
          "totals": {
            "inputTokens": 10,
            "outputTokens": 5,
            "cacheCreationTokens": 0,
            "cacheReadTokens": 0,
            "totalTokens": 15,
            "totalCost": 0.01
          }
        }"#;
        let context = build_context(SourceKey::ClaudeCode, "20.0.19", 1, "UTC").expect("context");
        let candidates = map_session(
            decode_claude_session(input).expect("decoded with path"),
            context,
        )
        .expect("mapped with path");
        assert_eq!(
            candidates[0].project_path.as_deref(),
            Some("/tmp/sanitized-project")
        );
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
            build_context(SourceKey::ClaudeCode, "20.0.19", 1, " ").expect_err("empty timezone"),
            MappingError::EmptyAggregationTimezone
        );
        assert_eq!(
            build_context(SourceKey::ClaudeCode, " ", 1, "UTC").expect_err("empty version"),
            MappingError::EmptyCollectorVersion
        );
        assert_eq!(
            build_context(SourceKey::ClaudeCode, "20.0.19", 0, "UTC").expect_err("invalid profile"),
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

    #[test]
    fn maps_codex_daily_usage_with_deterministic_identity() {
        let context =
            build_context(SourceKey::Codex, "20.0.19", 1, "Asia/Jakarta").expect("context");
        let candidates = map_codex_daily(
            decode_codex_daily(CODEX_DAILY_VALID).expect("decoded fixture"),
            context,
        )
        .expect("mapped candidates");

        assert_eq!(candidates.len(), 2);
        let first = &candidates[0];
        assert_eq!(first.source_key, "codex:daily:v1:Asia/Jakarta:2026-06-13");
        assert_eq!(first.aggregation_timezone, "Asia/Jakarta");
        assert_eq!(first.tokens.total_tokens(), 1_650);
        assert_eq!(first.tokens.input_tokens(), Some(1000));
        assert_eq!(first.tokens.output_tokens(), Some(400));
        assert_eq!(first.model_breakdowns.len(), 2);

        let pro_model = first
            .model_breakdowns
            .iter()
            .find(|m| m.raw_model_id == "gemini-2.5-pro")
            .expect("pro model");
        assert_eq!(pro_model.tokens.total_tokens(), 1080); // 700 + 300 + 80 reasoning

        let flash_model = first
            .model_breakdowns
            .iter()
            .find(|m| m.raw_model_id == "gemini-2.5-flash")
            .expect("flash model");
        assert_eq!(flash_model.tokens.total_tokens(), 420); // 300 + 100 + 20 reasoning

        assert_eq!(
            first.cost,
            UsageCost::Valued {
                amount_micros: 420_000,
                currency: CurrencyCode::new("USD").expect("currency"),
                kind: CostKind::CollectorCalculated,
                status: ValuedCostStatus::Estimated,
            }
        );
    }

    #[test]
    fn maps_real_codex_daily_shape_without_model_costs() {
        let context =
            build_context(SourceKey::Codex, "20.0.19", 1, "Asia/Jakarta").expect("context");
        let candidates = map_codex_daily(
            decode_codex_daily(CODEX_DAILY_REAL_SHAPE).expect("decoded fixture"),
            context,
        )
        .expect("mapped candidates");

        assert_eq!(candidates.len(), 1);
        let first = &candidates[0];
        assert_eq!(first.source_key, "codex:daily:v1:Asia/Jakarta:2026-06-24");
        assert_eq!(first.tokens.cache_read_tokens(), Some(400));
        assert_eq!(first.tokens.unclassified_tokens(), Some(0));
        assert_eq!(first.model_breakdowns.len(), 1);
        assert_eq!(
            first.model_breakdowns[0].cost,
            UsageCost::Unavailable {
                kind: CostKind::CollectorCalculated,
            }
        );
    }

    #[test]
    fn maps_codex_session_usage_with_deterministic_identity() {
        let context = build_context(SourceKey::Codex, "20.0.19", 1, "UTC").expect("context");
        let candidates = map_codex_session(
            decode_codex_session(CODEX_SESSION_VALID).expect("decoded fixture"),
            context,
        )
        .expect("mapped candidates");

        assert_eq!(candidates.len(), 1);
        let first = &candidates[0];
        assert_eq!(first.source_key, "codex:session:v1:session-1");
        assert_eq!(first.source_session_id, "session-1");
        assert_eq!(
            first.project_path,
            Some("/tmp/burnly-fixture/project".to_owned())
        );
        assert_eq!(first.tokens.total_tokens(), 1_650);
        assert_eq!(first.tokens.input_tokens(), Some(1000));
        assert_eq!(first.tokens.output_tokens(), Some(400));
        assert_eq!(first.model_breakdowns.len(), 2);

        let pro_model = first
            .model_breakdowns
            .iter()
            .find(|m| m.raw_model_id == "gemini-2.5-pro")
            .expect("pro model");
        assert_eq!(pro_model.tokens.total_tokens(), 1080);

        assert_eq!(
            first.cost,
            UsageCost::Valued {
                amount_micros: 420_000,
                currency: CurrencyCode::new("USD").expect("currency"),
                kind: CostKind::CollectorCalculated,
                status: ValuedCostStatus::Estimated,
            }
        );
    }

    #[test]
    fn maps_real_codex_session_shape_with_nullable_activity_bounds() {
        let context = build_context(SourceKey::Codex, "20.0.19", 1, "UTC").expect("context");
        let candidates = map_codex_session(
            decode_codex_session(CODEX_SESSION_REAL_SHAPE).expect("decoded fixture"),
            context,
        )
        .expect("mapped candidates");

        assert_eq!(candidates.len(), 1);
        let first = &candidates[0];
        assert_eq!(first.source_key, "codex:session:v1:session-1");
        assert_eq!(first.first_activity_at, None);
        assert_eq!(
            first
                .last_activity_at
                .map(|timestamp| timestamp.to_rfc3339()),
            Some("2026-06-24T08:33:53.771+00:00".to_owned())
        );
        assert_eq!(first.tokens.cache_read_tokens(), Some(400));
    }

    #[test]
    fn maps_pi_daily_usage_with_deterministic_identity_and_preserved_label() {
        let context = build_context(SourceKey::Pi, "20.0.19", 1, "Asia/Jakarta").expect("context");
        let candidates = map_pi_daily(
            decode_pi_daily(PI_DAILY_VALID).expect("decoded fixture"),
            context,
        )
        .expect("mapped candidates");

        assert_eq!(candidates.len(), 2);
        let first = &candidates[0];
        assert_eq!(first.source_key, "pi:daily:v1:Asia/Jakarta:2026-06-13");
        assert_eq!(first.aggregation_timezone, "Asia/Jakarta");
        assert_eq!(first.tokens.total_tokens(), 550);
        assert_eq!(first.tokens.cache_read_tokens(), Some(400));
        assert_eq!(first.model_breakdowns.len(), 1);
        // The `[pi]`-prefixed label is preserved exactly as ccusage emits it.
        assert_eq!(first.model_breakdowns[0].raw_model_id, "[pi] gpt-5.4-mini");

        // The second day used several models with no per-model split, so the
        // tokens stay together under the shared aggregate-label policy.
        let second = &candidates[1];
        assert_eq!(second.source_key, "pi:daily:v1:Asia/Jakarta:2026-06-14");
        assert_eq!(second.tokens.total_tokens(), 1_100);
        assert_eq!(second.model_breakdowns.len(), 1);
        assert_eq!(second.model_breakdowns[0].raw_model_id, "Multiple models");
    }

    #[test]
    fn maps_real_pi_daily_shape_with_single_model() {
        let context = build_context(SourceKey::Pi, "20.0.19", 1, "UTC").expect("context");
        let candidates = map_pi_daily(
            decode_pi_daily(PI_DAILY_REAL_SHAPE).expect("decoded fixture"),
            context,
        )
        .expect("mapped candidates");

        assert_eq!(candidates.len(), 1);
        let first = &candidates[0];
        assert_eq!(first.source_key, "pi:daily:v1:UTC:2026-07-01");
        assert_eq!(first.tokens.total_tokens(), 1_200);
        assert_eq!(first.tokens.cache_read_tokens(), Some(400));
        assert_eq!(first.tokens.unclassified_tokens(), Some(0));
        assert_eq!(first.model_breakdowns.len(), 1);
        assert_eq!(first.model_breakdowns[0].raw_model_id, "[pi] gpt-5.4-mini");
    }

    #[test]
    fn preserves_empty_pi_daily_collection() {
        let context = build_context(SourceKey::Pi, "20.0.19", 1, "UTC").expect("context");
        assert!(map_pi_daily(
            decode_pi_daily(PI_DAILY_EMPTY).expect("decoded fixture"),
            context,
        )
        .expect("mapped empty report")
        .is_empty());
    }

    #[test]
    fn rejects_malformed_pi_daily_json() {
        assert_eq!(
            decode_pi_daily(PI_DAILY_INVALID_JSON)
                .expect_err("invalid json")
                .code,
            CollectorFailureCode::InvalidJson
        );
        assert_eq!(
            decode_pi_daily(PI_DAILY_INCOMPATIBLE)
                .expect_err("incompatible envelope")
                .code,
            CollectorFailureCode::IncompatibleEnvelope
        );
    }

    #[test]
    fn maps_pi_session_usage_with_deterministic_identity_and_preserved_label() {
        let context = build_context(SourceKey::Pi, "20.0.19", 1, "UTC").expect("context");
        let candidates = map_pi_session(
            decode_pi_session(PI_SESSION_VALID).expect("decoded fixture"),
            context,
        )
        .expect("mapped candidates");

        assert_eq!(candidates.len(), 1);
        let first = &candidates[0];
        assert_eq!(first.source_key, "pi:session:v1:session-1");
        assert_eq!(first.source_session_id, "session-1");
        // Pi `projectPath` is intentionally not persisted.
        assert_eq!(first.project_path, None);
        assert_eq!(first.tokens.total_tokens(), 550);
        assert_eq!(first.tokens.cache_read_tokens(), Some(400));
        assert_eq!(first.model_breakdowns.len(), 1);
        assert_eq!(first.model_breakdowns[0].raw_model_id, "[pi] gpt-5.4-mini");
        assert_eq!(
            first
                .first_activity_at
                .map(|timestamp| timestamp.to_rfc3339()),
            Some("2026-07-01T00:57:01.464+00:00".to_owned())
        );
    }

    #[test]
    fn maps_real_pi_session_shape_with_nullable_activity_bounds() {
        let context = build_context(SourceKey::Pi, "20.0.19", 1, "UTC").expect("context");
        let candidates = map_pi_session(
            decode_pi_session(PI_SESSION_REAL_SHAPE).expect("decoded fixture"),
            context,
        )
        .expect("mapped candidates");

        assert_eq!(candidates.len(), 1);
        let first = &candidates[0];
        assert_eq!(first.source_key, "pi:session:v1:session-1");
        assert_eq!(first.first_activity_at, None);
        assert_eq!(first.last_activity_at, None);
        assert_eq!(first.tokens.cache_read_tokens(), Some(400));
    }

    #[test]
    fn preserves_empty_pi_session_collection() {
        let context = build_context(SourceKey::Pi, "20.0.19", 1, "UTC").expect("context");
        assert!(map_pi_session(
            decode_pi_session(PI_SESSION_EMPTY).expect("decoded fixture"),
            context,
        )
        .expect("mapped empty report")
        .is_empty());
    }

    fn context(timezone: &str) -> MappingContext {
        build_context(SourceKey::ClaudeCode, "20.0.19", 1, timezone).expect("mapping context")
    }

    fn build_context(
        source: SourceKey,
        collector_version: &str,
        profile_version: u16,
        timezone: &str,
    ) -> Result<MappingContext, MappingError> {
        MappingContext::new(
            source,
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
