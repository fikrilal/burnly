//! Pure mapping from reconciled OpenCode ledger state to canonical candidates.

use std::collections::BTreeMap;

use chrono::{DateTime, NaiveDate, Utc};
use chrono_tz::Tz;
use thiserror::Error;

use crate::application::collection::{
    CandidateProvenance, CandidateWarning, CollectionId, CollectionScope, CollectorKey,
    DailyUsageCandidate, ModelUsageCandidate, SessionUsageCandidate,
};
use crate::application::cost::BurnlyCostCalculator;
use crate::application::ports::opencode_usage_ledger::{
    OpenCodeDataQuality, OpenCodeLedgerOrigin, OpenCodeLedgerReconcileResult, OpenCodeLedgerRecord,
    OpenCodeReconciliationState, OpenCodeTokenVector,
};
use crate::domain::identity::{daily_source_key, session_source_key, IdentityError};
use crate::domain::source::SourceKey;
use crate::domain::usage::{
    CostKind, CurrencyCode, DataQuality, TokenUsage, UsageCost, UsageValidationError,
    ValuedCostStatus,
};

use super::super::support::{
    date_in_scope, local_date_from_millis, provenance, utc_from_millis, MappingIdentity,
};

const COLLECTOR_KEY: &str = "opencode";
const PROFILE_VERSION: u16 = 2;
const USD_MICROS_PER_DOLLAR: f64 = 1_000_000.0;
const UNATTRIBUTED_MODEL: &str = "OpenCode unattributed";
const RECOVERY_WARNING_CODE: &str = "opencode.cumulative_recovery";
const DEFERRED_WARNING_CODE: &str = "opencode.live_write_deferred";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OpenCodeMappingContext {
    collector: CollectorKey,
    collector_version: String,
    collection_id: CollectionId,
    observed_at: DateTime<Utc>,
}

impl OpenCodeMappingContext {
    pub(crate) fn new(
        collector_version: String,
        collection_id: CollectionId,
        observed_at: DateTime<Utc>,
    ) -> Result<Self, OpenCodeMappingError> {
        if collector_version.trim().is_empty() {
            return Err(OpenCodeMappingError::EmptyCollectorVersion);
        }
        Ok(Self {
            collector: CollectorKey::new(COLLECTOR_KEY).expect("OpenCode collector key"),
            collector_version,
            collection_id,
            observed_at,
        })
    }

    fn provenance(&self, quality: MappingQuality) -> CandidateProvenance {
        let mut candidate = provenance(&MappingIdentity {
            source: SourceKey::OpenCode,
            collector: self.collector.clone(),
            collector_version: self.collector_version.clone(),
            profile_version: PROFILE_VERSION,
            collection_id: self.collection_id.clone(),
            observed_at: self.observed_at,
        });
        if quality.is_partial() {
            candidate.data_quality = DataQuality::Partial;
        }
        if quality.has_recovery {
            candidate.warnings.push(CandidateWarning {
                code: RECOVERY_WARNING_CODE.to_owned(),
                message: "Some OpenCode usage has only cumulative attribution.".to_owned(),
            });
        }
        if quality.has_deferred_live_write {
            candidate.warnings.push(CandidateWarning {
                code: DEFERRED_WARNING_CODE.to_owned(),
                message: "OpenCode cumulative recovery was deferred for an active response."
                    .to_owned(),
            });
        }
        candidate
    }
}

pub(crate) fn source_cost_usd_to_micros(
    cost_usd: Option<f64>,
) -> Result<Option<u64>, OpenCodeMappingError> {
    let Some(cost_usd) = cost_usd else {
        return Ok(None);
    };
    if !cost_usd.is_finite() || cost_usd < 0.0 {
        return Err(OpenCodeMappingError::InvalidCost);
    }
    let micros = (cost_usd * USD_MICROS_PER_DOLLAR + 0.5).floor();
    if micros > u64::MAX as f64 {
        return Err(OpenCodeMappingError::InvalidCost);
    }
    Ok(Some(micros as u64))
}

pub(crate) fn map_daily(
    reconciled_sessions: &[OpenCodeLedgerReconcileResult],
    timezone: &str,
    scope: &CollectionScope,
    context: &OpenCodeMappingContext,
    calculator: &BurnlyCostCalculator,
) -> Result<Vec<DailyUsageCandidate>, OpenCodeMappingError> {
    let timezone = timezone
        .parse::<Tz>()
        .map_err(|_| OpenCodeMappingError::InvalidTimezone)?;
    let mut days = BTreeMap::<NaiveDate, ProjectionBucket>::new();

    for reconciled in reconciled_sessions {
        validate_reconciled_session(reconciled)?;
        for record in &reconciled.records {
            let usage_date = local_date_from_millis(
                record.activity_at_ms,
                timezone,
                OpenCodeMappingError::InvalidTimestamp,
            )?;
            if !date_in_scope(usage_date, scope) {
                continue;
            }
            days.entry(usage_date)
                .or_default()
                .add(record, MappingQuality::from_record(reconciled, record))?;
        }
    }

    days.into_iter()
        .map(|(usage_date, bucket)| {
            let provenance = context.provenance(bucket.quality);
            let (tokens, cost, model_breakdowns) = bucket.finish(calculator)?;
            Ok(DailyUsageCandidate {
                provenance,
                source_key: daily_source_key(SourceKey::OpenCode, usage_date, timezone.name())?,
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
    reconciled_sessions: &[OpenCodeLedgerReconcileResult],
    context: &OpenCodeMappingContext,
    calculator: &BurnlyCostCalculator,
) -> Result<Vec<SessionUsageCandidate>, OpenCodeMappingError> {
    let mut candidates = Vec::with_capacity(reconciled_sessions.len());
    for reconciled in reconciled_sessions {
        validate_reconciled_session(reconciled)?;
        if reconciled.records.is_empty() {
            continue;
        }
        let mut bucket = ProjectionBucket::default();
        let quality = MappingQuality::from_reconciled(reconciled);
        for record in &reconciled.records {
            bucket.add(record, quality)?;
        }
        let first_activity_ms = reconciled
            .records
            .iter()
            .map(|record| record.activity_at_ms)
            .min()
            .expect("non-empty records have a first activity");
        let last_activity_ms = reconciled
            .records
            .iter()
            .map(|record| record.activity_at_ms)
            .max()
            .expect("non-empty records have a last activity");
        let provenance = context.provenance(bucket.quality);
        let (tokens, cost, model_breakdowns) = bucket.finish(calculator)?;
        candidates.push(SessionUsageCandidate {
            provenance,
            source_key: session_source_key(SourceKey::OpenCode, &reconciled.checkpoint.session_id)?,
            source_session_id: reconciled.checkpoint.session_id.clone(),
            project_path: None,
            first_activity_at: Some(timestamp(first_activity_ms)?),
            last_activity_at: Some(timestamp(last_activity_ms)?),
            tokens,
            cost,
            model_breakdowns,
        });
    }
    Ok(candidates)
}

#[derive(Debug, Clone, Copy, Default)]
struct MappingQuality {
    has_recovery: bool,
    has_deferred_live_write: bool,
}

impl MappingQuality {
    fn from_record(
        reconciled: &OpenCodeLedgerReconcileResult,
        record: &OpenCodeLedgerRecord,
    ) -> Self {
        Self {
            has_recovery: record.quality == OpenCodeDataQuality::Partial
                || record.origin == OpenCodeLedgerOrigin::CumulativeRecovery,
            has_deferred_live_write: reconciled.checkpoint.reconciliation_state
                == OpenCodeReconciliationState::DeferredLiveWrite,
        }
    }

    fn from_reconciled(reconciled: &OpenCodeLedgerReconcileResult) -> Self {
        Self {
            has_recovery: reconciled.records.iter().any(|record| {
                record.quality == OpenCodeDataQuality::Partial
                    || record.origin == OpenCodeLedgerOrigin::CumulativeRecovery
            }) || reconciled.checkpoint.reconciliation_state
                == OpenCodeReconciliationState::Partial,
            has_deferred_live_write: reconciled.checkpoint.reconciliation_state
                == OpenCodeReconciliationState::DeferredLiveWrite,
        }
    }

    const fn is_partial(self) -> bool {
        self.has_recovery || self.has_deferred_live_write
    }

    fn include(&mut self, other: Self) {
        self.has_recovery |= other.has_recovery;
        self.has_deferred_live_write |= other.has_deferred_live_write;
    }
}

#[derive(Debug, Default)]
struct ProjectionBucket {
    total: UsageAccumulator,
    models: BTreeMap<String, ModelBucket>,
    quality: MappingQuality,
}

impl ProjectionBucket {
    fn add(
        &mut self,
        record: &OpenCodeLedgerRecord,
        quality: MappingQuality,
    ) -> Result<(), OpenCodeMappingError> {
        let model = model_identity(record)?;
        let attribution = if record.origin == OpenCodeLedgerOrigin::CumulativeRecovery {
            ModelAttribution::Recovery
        } else {
            ModelAttribution::Exact
        };
        self.total.add(record.tokens, record.cost_micros)?;
        self.models
            .entry(model)
            .or_insert_with(|| ModelBucket::new(attribution))
            .add(record, attribution)?;
        self.quality.include(quality);
        Ok(())
    }

    fn finish(
        &self,
        calculator: &BurnlyCostCalculator,
    ) -> Result<(TokenUsage, UsageCost, Vec<ModelUsageCandidate>), OpenCodeMappingError> {
        let tokens = self.total.tokens()?;
        let mut model_breakdowns = Vec::with_capacity(self.models.len());
        for (model, usage) in &self.models {
            let model_tokens = usage.usage.tokens()?;
            let cost = model_cost(
                model,
                &model_tokens,
                usage.usage.cost_micros,
                usage.attribution,
                calculator,
            );
            model_breakdowns.push(ModelUsageCandidate {
                raw_model_id: model.clone(),
                tokens: model_tokens,
                cost,
            });
        }
        let cost = aggregate_cost(&model_breakdowns)?;
        Ok((tokens, cost, model_breakdowns))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModelAttribution {
    Exact,
    Recovery,
}

#[derive(Debug)]
struct ModelBucket {
    attribution: ModelAttribution,
    usage: UsageAccumulator,
}

impl ModelBucket {
    fn new(attribution: ModelAttribution) -> Self {
        Self {
            attribution,
            usage: UsageAccumulator::default(),
        }
    }

    fn add(
        &mut self,
        record: &OpenCodeLedgerRecord,
        attribution: ModelAttribution,
    ) -> Result<(), OpenCodeMappingError> {
        if self.attribution != attribution {
            return Err(OpenCodeMappingError::InconsistentLedger);
        }
        self.usage.add(record.tokens, record.cost_micros)
    }
}

#[derive(Debug, Clone, Copy)]
struct UsageAccumulator {
    tokens: OpenCodeTokenVector,
    cost_micros: Option<u64>,
}

impl Default for UsageAccumulator {
    fn default() -> Self {
        Self {
            tokens: OpenCodeTokenVector::default(),
            cost_micros: Some(0),
        }
    }
}

impl UsageAccumulator {
    fn add(
        &mut self,
        tokens: OpenCodeTokenVector,
        cost_micros: Option<u64>,
    ) -> Result<(), OpenCodeMappingError> {
        self.tokens = self
            .tokens
            .checked_add(tokens)
            .ok_or(OpenCodeMappingError::TokenOverflow)?;
        self.cost_micros = match (self.cost_micros, cost_micros) {
            (Some(total), Some(cost)) => Some(
                total
                    .checked_add(cost)
                    .ok_or(OpenCodeMappingError::CostOverflow)?,
            ),
            _ => None,
        };
        Ok(())
    }

    fn tokens(self) -> Result<TokenUsage, OpenCodeMappingError> {
        tokens(self.tokens)
    }
}

fn validate_reconciled_session(
    reconciled: &OpenCodeLedgerReconcileResult,
) -> Result<(), OpenCodeMappingError> {
    if reconciled.checkpoint.session_id.trim().is_empty()
        || reconciled
            .records
            .iter()
            .any(|record| record.session_id != reconciled.checkpoint.session_id)
    {
        return Err(OpenCodeMappingError::InconsistentLedger);
    }
    let mut total = UsageAccumulator::default();
    for record in &reconciled.records {
        total.add(record.tokens, record.cost_micros)?;
    }
    if total.tokens != reconciled.checkpoint.accepted_tokens
        || total.cost_micros != reconciled.checkpoint.accepted_cost_micros
    {
        return Err(OpenCodeMappingError::InconsistentLedger);
    }
    Ok(())
}

fn model_identity(record: &OpenCodeLedgerRecord) -> Result<String, OpenCodeMappingError> {
    if record.origin == OpenCodeLedgerOrigin::CumulativeRecovery {
        if record.provider_id.is_some() || record.raw_model_id != UNATTRIBUTED_MODEL {
            return Err(OpenCodeMappingError::InconsistentLedger);
        }
        return Ok(UNATTRIBUTED_MODEL.to_owned());
    }
    let provider = record
        .provider_id
        .as_deref()
        .filter(|provider| !provider.trim().is_empty())
        .ok_or(OpenCodeMappingError::InconsistentLedger)?;
    if record.raw_model_id.trim().is_empty() {
        return Err(OpenCodeMappingError::InconsistentLedger);
    }
    Ok(format!("{provider}/{}", record.raw_model_id))
}

fn tokens(vector: OpenCodeTokenVector) -> Result<TokenUsage, OpenCodeMappingError> {
    let total = vector
        .input
        .checked_add(vector.output)
        .and_then(|total| total.checked_add(vector.reasoning))
        .and_then(|total| total.checked_add(vector.cache_read))
        .and_then(|total| total.checked_add(vector.cache_write))
        .ok_or(OpenCodeMappingError::TokenOverflow)?;
    TokenUsage::new(
        Some(vector.input),
        Some(vector.output),
        Some(vector.cache_write),
        Some(vector.cache_read),
        total,
    )
    .map_err(Into::into)
}

fn model_cost(
    model: &str,
    tokens: &TokenUsage,
    cost_micros: Option<u64>,
    attribution: ModelAttribution,
    calculator: &BurnlyCostCalculator,
) -> UsageCost {
    let Some(cost_micros) = cost_micros else {
        return UsageCost::Unavailable {
            kind: CostKind::SourceReported,
        };
    };
    if cost_micros > 0 {
        return valued_cost(cost_micros, CostKind::SourceReported);
    }
    if tokens.total_tokens() == 0 {
        return UsageCost::NotApplicable {
            kind: CostKind::SourceReported,
        };
    }
    if attribution == ModelAttribution::Recovery {
        return UsageCost::Unavailable {
            kind: CostKind::SourceReported,
        };
    }
    match calculated_model_cost(calculator, model, tokens) {
        UsageCost::Valued { amount_micros, .. } if amount_micros > 0 => {
            valued_cost(amount_micros, CostKind::BurnlyCalculated)
        }
        UsageCost::NotApplicable { .. } => UsageCost::NotApplicable {
            kind: CostKind::BurnlyCalculated,
        },
        _ => UsageCost::Unavailable {
            kind: CostKind::SourceReported,
        },
    }
}

fn calculated_model_cost(
    calculator: &BurnlyCostCalculator,
    provider_qualified_model: &str,
    tokens: &TokenUsage,
) -> UsageCost {
    let qualified = calculator.calculate(provider_qualified_model, tokens).cost;
    if !matches!(qualified, UsageCost::Unavailable { .. }) {
        return qualified;
    }
    let Some((_, model)) = provider_qualified_model.split_once('/') else {
        return qualified;
    };
    calculator.calculate(model, tokens).cost
}

fn aggregate_cost(
    model_breakdowns: &[ModelUsageCandidate],
) -> Result<UsageCost, OpenCodeMappingError> {
    let kind = aggregate_cost_kind(model_breakdowns);
    if model_breakdowns
        .iter()
        .any(|model| matches!(model.cost, UsageCost::Unavailable { .. }))
    {
        return Ok(UsageCost::Unavailable { kind });
    }

    let mut total_micros = 0_u64;
    let mut saw_valued = false;
    for model in model_breakdowns {
        if let UsageCost::Valued { amount_micros, .. } = model.cost {
            total_micros = total_micros
                .checked_add(amount_micros)
                .ok_or(OpenCodeMappingError::CostOverflow)?;
            saw_valued = true;
        }
    }
    if saw_valued {
        Ok(valued_cost(total_micros, kind))
    } else {
        Ok(UsageCost::NotApplicable { kind })
    }
}

fn aggregate_cost_kind(model_breakdowns: &[ModelUsageCandidate]) -> CostKind {
    let mut source_reported = false;
    let mut burnly_calculated = false;
    for model in model_breakdowns {
        match cost_kind(&model.cost) {
            CostKind::SourceReported => source_reported = true,
            CostKind::BurnlyCalculated => burnly_calculated = true,
            _ => {}
        }
    }
    match (source_reported, burnly_calculated) {
        (true, true) => CostKind::CollectorMixed,
        (false, true) => CostKind::BurnlyCalculated,
        _ => CostKind::SourceReported,
    }
}

const fn cost_kind(cost: &UsageCost) -> CostKind {
    match cost {
        UsageCost::Valued { kind, .. }
        | UsageCost::NotApplicable { kind }
        | UsageCost::Unavailable { kind } => *kind,
    }
}

fn valued_cost(amount_micros: u64, kind: CostKind) -> UsageCost {
    UsageCost::Valued {
        amount_micros,
        currency: CurrencyCode::new("USD").expect("USD is a valid ISO-shaped currency"),
        kind,
        status: ValuedCostStatus::Estimated,
    }
}

fn timestamp(timestamp_ms: i64) -> Result<DateTime<Utc>, OpenCodeMappingError> {
    utc_from_millis(timestamp_ms, OpenCodeMappingError::InvalidTimestamp)
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub(crate) enum OpenCodeMappingError {
    #[error("OpenCode mapping requires a collector version")]
    EmptyCollectorVersion,
    #[error("OpenCode mapping requires a valid timezone")]
    InvalidTimezone,
    #[error("OpenCode mapping received an invalid timestamp")]
    InvalidTimestamp,
    #[error("OpenCode source cost is invalid")]
    InvalidCost,
    #[error("OpenCode token total overflowed")]
    TokenOverflow,
    #[error("OpenCode cost total overflowed")]
    CostOverflow,
    #[error("OpenCode reconciled ledger state is inconsistent")]
    InconsistentLedger,
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error(transparent)]
    Usage(#[from] UsageValidationError),
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;
    use crate::application::ports::opencode_usage_ledger::{
        OpenCodeSessionCheckpoint, OpenCodeTimestampOrigin,
    };

    #[test]
    fn converts_optional_source_cost_to_micros_rounding_half_up() {
        assert_eq!(source_cost_usd_to_micros(None), Ok(None));
        assert_eq!(source_cost_usd_to_micros(Some(0.0)), Ok(Some(0)));
        assert_eq!(source_cost_usd_to_micros(Some(0.000_000_5)), Ok(Some(1)));
        assert_eq!(
            source_cost_usd_to_micros(Some(1.234_567_4)),
            Ok(Some(1_234_567))
        );
        assert_eq!(
            source_cost_usd_to_micros(Some(-0.1)),
            Err(OpenCodeMappingError::InvalidCost)
        );
        assert_eq!(
            source_cost_usd_to_micros(Some(f64::NAN)),
            Err(OpenCodeMappingError::InvalidCost)
        );
        assert_eq!(
            source_cost_usd_to_micros(Some(f64::INFINITY)),
            Err(OpenCodeMappingError::InvalidCost)
        );
        assert_eq!(
            source_cost_usd_to_micros(Some(u64::MAX as f64)),
            Err(OpenCodeMappingError::InvalidCost)
        );
    }

    #[test]
    fn daily_mapping_separates_provider_collisions_and_preserves_reasoning() {
        let in_scope = timestamp_ms(2026, 8, 21, 18, 30);
        let out_of_scope = timestamp_ms(2026, 8, 22, 18, 30);
        let records = vec![
            exact_record(
                "session-daily",
                "message-one",
                "provider-a",
                "shared-model",
                in_scope,
                token_vector(10, 2, 3, 4, 5),
                Some(100),
            ),
            exact_record(
                "session-daily",
                "message-two",
                "provider-b",
                "shared-model",
                in_scope + 1,
                token_vector(10, 2, 3, 4, 5),
                Some(200),
            ),
            exact_record(
                "session-daily",
                "message-outside",
                "provider-a",
                "shared-model",
                out_of_scope,
                token_vector(1, 0, 0, 0, 0),
                Some(50),
            ),
        ];
        let reconciled = vec![reconciled(
            "session-daily",
            records,
            OpenCodeReconciliationState::Complete,
        )];
        let scope = CollectionScope::incremental(
            NaiveDate::from_ymd_opt(2026, 8, 22).expect("date"),
            NaiveDate::from_ymd_opt(2026, 8, 22).expect("date"),
        )
        .expect("scope");

        let candidates = map_daily(
            &reconciled,
            "Asia/Jakarta",
            &scope,
            &context(),
            &calculator(),
        )
        .expect("daily mapping");

        assert_eq!(candidates.len(), 1);
        let candidate = &candidates[0];
        assert_eq!(
            candidate.source_key,
            "opencode:daily:v1:Asia/Jakarta:2026-08-22"
        );
        assert_eq!(candidate.provenance.source, SourceKey::OpenCode);
        assert_eq!(candidate.provenance.profile_version, 2);
        assert_eq!(candidate.provenance.data_quality, DataQuality::Complete);
        assert_eq!(candidate.tokens.input_tokens(), Some(20));
        assert_eq!(candidate.tokens.output_tokens(), Some(4));
        assert_eq!(candidate.tokens.cache_read_tokens(), Some(8));
        assert_eq!(candidate.tokens.cache_creation_tokens(), Some(10));
        assert_eq!(candidate.tokens.unclassified_tokens(), Some(6));
        assert_eq!(candidate.tokens.total_tokens(), 48);
        assert_eq!(candidate.model_breakdowns.len(), 2);
        assert_eq!(
            candidate.model_breakdowns[0].raw_model_id,
            "provider-a/shared-model"
        );
        assert_eq!(
            candidate.model_breakdowns[1].raw_model_id,
            "provider-b/shared-model"
        );
        assert_eq!(valued_micros(&candidate.cost), Some(300));
        assert_eq!(
            candidate
                .model_breakdowns
                .iter()
                .map(|model| model.tokens.total_tokens())
                .sum::<u64>(),
            candidate.tokens.total_tokens()
        );
        assert_eq!(
            candidate
                .model_breakdowns
                .iter()
                .filter_map(|model| valued_micros(&model.cost))
                .sum::<u64>(),
            valued_micros(&candidate.cost).expect("aggregate valued cost")
        );
    }

    #[test]
    fn recovery_is_partial_unattributed_and_never_model_priced() {
        let records = vec![
            exact_record(
                "session-recovery",
                "message-exact",
                "openai",
                "gpt-5",
                timestamp_ms(2026, 8, 22, 1, 0),
                token_vector(1_000, 100, 0, 0, 0),
                Some(0),
            ),
            recovery_record(
                "session-recovery",
                0,
                timestamp_ms(2026, 8, 22, 2, 0),
                token_vector(500, 0, 0, 0, 0),
                Some(0),
            ),
        ];
        let reconciled = vec![reconciled(
            "session-recovery",
            records,
            OpenCodeReconciliationState::Partial,
        )];

        let candidates = map_daily(
            &reconciled,
            "UTC",
            &CollectionScope::Full,
            &context(),
            &calculator(),
        )
        .expect("daily mapping");

        let candidate = &candidates[0];
        assert_eq!(candidate.provenance.data_quality, DataQuality::Partial);
        assert_eq!(candidate.provenance.warnings[0].code, RECOVERY_WARNING_CODE);
        assert!(!candidate.provenance.warnings[0]
            .message
            .contains("session-recovery"));
        let exact = candidate
            .model_breakdowns
            .iter()
            .find(|model| model.raw_model_id == "openai/gpt-5")
            .expect("exact model");
        assert!(matches!(
            exact.cost,
            UsageCost::Valued {
                kind: CostKind::BurnlyCalculated,
                amount_micros,
                ..
            } if amount_micros > 0
        ));
        let recovery = candidate
            .model_breakdowns
            .iter()
            .find(|model| model.raw_model_id == UNATTRIBUTED_MODEL)
            .expect("recovery model");
        assert!(matches!(
            recovery.cost,
            UsageCost::Unavailable {
                kind: CostKind::SourceReported
            }
        ));
        assert!(matches!(
            candidate.cost,
            UsageCost::Unavailable {
                kind: CostKind::CollectorMixed
            }
        ));
    }

    #[test]
    fn recovery_marks_only_its_daily_bucket_partial() {
        let records = vec![
            exact_record(
                "session-cross-day",
                "message-exact",
                "provider",
                "model",
                timestamp_ms(2026, 8, 21, 1, 0),
                token_vector(10, 0, 0, 0, 0),
                Some(10),
            ),
            recovery_record(
                "session-cross-day",
                0,
                timestamp_ms(2026, 8, 22, 1, 0),
                token_vector(5, 0, 0, 0, 0),
                Some(5),
            ),
        ];
        let reconciled = vec![reconciled(
            "session-cross-day",
            records,
            OpenCodeReconciliationState::Partial,
        )];

        let candidates = map_daily(
            &reconciled,
            "UTC",
            &CollectionScope::Full,
            &context(),
            &calculator(),
        )
        .expect("daily mapping");

        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].provenance.data_quality, DataQuality::Complete);
        assert!(candidates[0].provenance.warnings.is_empty());
        assert_eq!(candidates[1].provenance.data_quality, DataQuality::Partial);
        assert_eq!(
            candidates[1].provenance.warnings[0].code,
            RECOVERY_WARNING_CODE
        );
    }

    #[test]
    fn session_mapping_uses_one_identity_models_and_activity_window() {
        let first = timestamp_ms(2026, 8, 22, 1, 0);
        let last = timestamp_ms(2026, 8, 22, 3, 0);
        let records = vec![
            exact_record(
                "session-window",
                "message-late",
                "provider-b",
                "model-b",
                last,
                token_vector(2, 3, 4, 5, 6),
                Some(200),
            ),
            exact_record(
                "session-window",
                "message-early",
                "provider-a",
                "model-a",
                first,
                token_vector(1, 2, 3, 4, 5),
                Some(100),
            ),
        ];
        let reconciled = vec![reconciled(
            "session-window",
            records,
            OpenCodeReconciliationState::Complete,
        )];

        let candidates =
            map_sessions(&reconciled, &context(), &calculator()).expect("session mapping");

        assert_eq!(candidates.len(), 1);
        let candidate = &candidates[0];
        assert_eq!(candidate.source_key, "opencode:session:v1:session-window");
        assert_eq!(candidate.source_session_id, "session-window");
        assert_eq!(candidate.project_path, None);
        assert_eq!(
            candidate
                .first_activity_at
                .expect("first")
                .timestamp_millis(),
            first
        );
        assert_eq!(
            candidate.last_activity_at.expect("last").timestamp_millis(),
            last
        );
        assert_eq!(candidate.model_breakdowns.len(), 2);
        assert_eq!(candidate.tokens.input_tokens(), Some(3));
        assert_eq!(candidate.tokens.unclassified_tokens(), Some(7));
        assert_eq!(valued_micros(&candidate.cost), Some(300));
    }

    #[test]
    fn deferred_session_is_partial_without_recovery_warning() {
        let reconciled = vec![reconciled(
            "session-live",
            vec![exact_record(
                "session-live",
                "message-durable",
                "provider",
                "model",
                timestamp_ms(2026, 8, 22, 1, 0),
                token_vector(5, 0, 0, 0, 0),
                Some(50),
            )],
            OpenCodeReconciliationState::DeferredLiveWrite,
        )];

        let candidates =
            map_sessions(&reconciled, &context(), &calculator()).expect("session mapping");
        let provenance = &candidates[0].provenance;
        assert_eq!(provenance.data_quality, DataQuality::Partial);
        assert_eq!(provenance.warnings.len(), 1);
        assert_eq!(provenance.warnings[0].code, DEFERRED_WARNING_CODE);
    }

    #[test]
    fn unknown_cost_remains_unavailable() {
        let reconciled = vec![reconciled(
            "session-unknown",
            vec![exact_record(
                "session-unknown",
                "message-unknown",
                "provider",
                "unknown-model",
                timestamp_ms(2026, 8, 22, 1, 0),
                token_vector(5, 0, 0, 0, 0),
                None,
            )],
            OpenCodeReconciliationState::Complete,
        )];

        let candidate = map_sessions(&reconciled, &context(), &calculator())
            .expect("mapping")
            .remove(0);
        assert!(matches!(
            candidate.cost,
            UsageCost::Unavailable {
                kind: CostKind::SourceReported
            }
        ));
    }

    #[test]
    fn rejects_checkpoint_mismatch_overflow_invalid_timestamp_and_timezone() {
        let mut mismatch = reconciled(
            "session-mismatch",
            vec![exact_record(
                "session-mismatch",
                "message",
                "provider",
                "model",
                timestamp_ms(2026, 8, 22, 1, 0),
                token_vector(5, 0, 0, 0, 0),
                Some(50),
            )],
            OpenCodeReconciliationState::Complete,
        );
        mismatch.checkpoint.accepted_tokens.input = 6;
        assert_eq!(
            map_sessions(&[mismatch], &context(), &calculator()).expect_err("checkpoint mismatch"),
            OpenCodeMappingError::InconsistentLedger
        );

        let overflow_records = vec![
            exact_record(
                "session-overflow",
                "message-one",
                "provider",
                "model",
                1,
                token_vector(u64::MAX, 0, 0, 0, 0),
                Some(0),
            ),
            exact_record(
                "session-overflow",
                "message-two",
                "provider",
                "model",
                2,
                token_vector(1, 0, 0, 0, 0),
                Some(0),
            ),
        ];
        let overflow = raw_reconciled(
            "session-overflow",
            overflow_records,
            token_vector(0, 0, 0, 0, 0),
            Some(0),
            OpenCodeReconciliationState::Complete,
        );
        assert_eq!(
            map_sessions(&[overflow], &context(), &calculator()).expect_err("overflow"),
            OpenCodeMappingError::TokenOverflow
        );

        let cost_overflow = raw_reconciled(
            "session-cost-overflow",
            vec![
                exact_record(
                    "session-cost-overflow",
                    "message-one",
                    "provider",
                    "model",
                    1,
                    token_vector(1, 0, 0, 0, 0),
                    Some(u64::MAX),
                ),
                exact_record(
                    "session-cost-overflow",
                    "message-two",
                    "provider",
                    "model",
                    2,
                    token_vector(1, 0, 0, 0, 0),
                    Some(1),
                ),
            ],
            token_vector(2, 0, 0, 0, 0),
            Some(0),
            OpenCodeReconciliationState::Complete,
        );
        assert_eq!(
            map_sessions(&[cost_overflow], &context(), &calculator()).expect_err("cost overflow"),
            OpenCodeMappingError::CostOverflow
        );

        let invalid_timestamp = reconciled(
            "session-timestamp",
            vec![exact_record(
                "session-timestamp",
                "message",
                "provider",
                "model",
                i64::MAX,
                token_vector(1, 0, 0, 0, 0),
                Some(1),
            )],
            OpenCodeReconciliationState::Complete,
        );
        assert_eq!(
            map_sessions(&[invalid_timestamp], &context(), &calculator())
                .expect_err("invalid timestamp"),
            OpenCodeMappingError::InvalidTimestamp
        );
        assert_eq!(
            map_daily(
                &[],
                "not/a-timezone",
                &CollectionScope::Full,
                &context(),
                &calculator(),
            )
            .expect_err("invalid timezone"),
            OpenCodeMappingError::InvalidTimezone
        );
    }

    fn context() -> OpenCodeMappingContext {
        OpenCodeMappingContext::new(
            "native-v2".to_owned(),
            CollectionId::new("opencode-mapping-test").expect("collection"),
            Utc.with_ymd_and_hms(2026, 8, 22, 4, 0, 0)
                .single()
                .expect("timestamp"),
        )
        .expect("context")
    }

    fn calculator() -> BurnlyCostCalculator {
        BurnlyCostCalculator::new()
    }

    fn reconciled(
        session_id: &str,
        records: Vec<OpenCodeLedgerRecord>,
        state: OpenCodeReconciliationState,
    ) -> OpenCodeLedgerReconcileResult {
        let mut tokens = OpenCodeTokenVector::default();
        let mut cost = Some(0_u64);
        for record in &records {
            tokens = tokens.checked_add(record.tokens).expect("fixture tokens");
            cost = match (cost, record.cost_micros) {
                (Some(total), Some(value)) => Some(total.checked_add(value).expect("fixture cost")),
                _ => None,
            };
        }
        raw_reconciled(session_id, records, tokens, cost, state)
    }

    fn raw_reconciled(
        session_id: &str,
        records: Vec<OpenCodeLedgerRecord>,
        accepted_tokens: OpenCodeTokenVector,
        accepted_cost_micros: Option<u64>,
        state: OpenCodeReconciliationState,
    ) -> OpenCodeLedgerReconcileResult {
        OpenCodeLedgerReconcileResult {
            records,
            checkpoint: OpenCodeSessionCheckpoint {
                session_id: session_id.to_owned(),
                accepted_tokens,
                accepted_cost_micros,
                observed_source_tokens: accepted_tokens,
                observed_source_cost_micros: accepted_cost_micros,
                source_updated_at_ms: 1,
                reconciliation_state: state,
                next_recovery_sequence: 1,
                first_observed_at_ms: 1,
                last_reconciled_at_ms: 2,
            },
            exact_records_accepted: 0,
            recovery_segments_created: 0,
            late_exact_reclassified: 0,
            late_exact_ignored: 0,
            counter_regressions: 0,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn exact_record(
        session_id: &str,
        message_id: &str,
        provider_id: &str,
        model_id: &str,
        activity_at_ms: i64,
        tokens: OpenCodeTokenVector,
        cost_micros: Option<u64>,
    ) -> OpenCodeLedgerRecord {
        OpenCodeLedgerRecord {
            source_message_id: Some(message_id.to_owned()),
            recovery_sequence: None,
            session_id: session_id.to_owned(),
            activity_at_ms,
            timestamp_origin: OpenCodeTimestampOrigin::SourceReported,
            provider_id: Some(provider_id.to_owned()),
            raw_model_id: model_id.to_owned(),
            tokens,
            cost_micros,
            origin: OpenCodeLedgerOrigin::V2Message,
            quality: OpenCodeDataQuality::Complete,
            first_seen_at_ms: 1,
            last_seen_at_ms: 2,
        }
    }

    fn recovery_record(
        session_id: &str,
        sequence: u64,
        activity_at_ms: i64,
        tokens: OpenCodeTokenVector,
        cost_micros: Option<u64>,
    ) -> OpenCodeLedgerRecord {
        OpenCodeLedgerRecord {
            source_message_id: None,
            recovery_sequence: Some(sequence),
            session_id: session_id.to_owned(),
            activity_at_ms,
            timestamp_origin: OpenCodeTimestampOrigin::SourceLifecycle,
            provider_id: None,
            raw_model_id: UNATTRIBUTED_MODEL.to_owned(),
            tokens,
            cost_micros,
            origin: OpenCodeLedgerOrigin::CumulativeRecovery,
            quality: OpenCodeDataQuality::Partial,
            first_seen_at_ms: 1,
            last_seen_at_ms: 1,
        }
    }

    const fn token_vector(
        input: u64,
        output: u64,
        reasoning: u64,
        cache_read: u64,
        cache_write: u64,
    ) -> OpenCodeTokenVector {
        OpenCodeTokenVector {
            input,
            output,
            reasoning,
            cache_read,
            cache_write,
        }
    }

    fn timestamp_ms(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> i64 {
        Utc.with_ymd_and_hms(year, month, day, hour, minute, 0)
            .single()
            .expect("timestamp")
            .timestamp_millis()
    }

    fn valued_micros(cost: &UsageCost) -> Option<u64> {
        match cost {
            UsageCost::Valued { amount_micros, .. } => Some(*amount_micros),
            _ => None,
        }
    }
}
