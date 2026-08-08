//! Pure cost calculation from token usage and the pricing snapshot.

use crate::application::collection::{
    DailyUsageCandidate, ModelUsageCandidate, SessionUsageCandidate,
};
use crate::domain::usage::{CostKind, CurrencyCode, TokenUsage, UsageCost, ValuedCostStatus};

use super::snapshot::{PricingEntry, PricingSnapshot};

/// Deterministic round-half-up conversion of a USD float to integer micros.
fn usd_to_micros(usd: f64) -> Option<u64> {
    if !usd.is_finite() || usd < 0.0 {
        return None;
    }
    let micros = (usd * 1_000_000.0 + 0.5).floor();
    if micros > u64::MAX as f64 {
        return None;
    }
    Some(micros as u64)
}

/// The outcome of pricing one token usage record.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CostCalculation {
    pub(crate) cost: UsageCost,
}

/// Price `tokens` for `model` from the snapshot.
///
/// - zero tokens → `NotApplicable`
/// - model not in snapshot → `Unavailable`
/// - explicit $0 model → `NotApplicable`
/// - otherwise → `Valued` with `BurnlyCalculated` kind, `Estimated` status
pub(crate) fn calculate_cost(
    model: &str,
    tokens: &TokenUsage,
    snapshot: &PricingSnapshot,
) -> CostCalculation {
    let total = tokens.total_tokens();
    if total == 0 {
        return CostCalculation {
            cost: UsageCost::NotApplicable {
                kind: CostKind::BurnlyCalculated,
            },
        };
    }
    let Some(entry) = snapshot.find(model) else {
        return CostCalculation {
            cost: UsageCost::Unavailable {
                kind: CostKind::BurnlyCalculated,
            },
        };
    };
    let Some(micros) = price_tokens(tokens, entry) else {
        return CostCalculation {
            cost: UsageCost::Unavailable {
                kind: CostKind::BurnlyCalculated,
            },
        };
    };
    if micros == 0 {
        return CostCalculation {
            cost: UsageCost::NotApplicable {
                kind: CostKind::BurnlyCalculated,
            },
        };
    }
    CostCalculation {
        cost: UsageCost::Valued {
            amount_micros: micros,
            currency: CurrencyCode::new("USD").expect("USD is a valid ISO-shaped currency"),
            kind: CostKind::BurnlyCalculated,
            status: ValuedCostStatus::Estimated,
        },
    }
}

/// Gap-fill: when a candidate carries zero cost with positive tokens and the
/// snapshot prices the model, replace the cost with a Burnly-calculated one.
///
/// Explicitly free models (priced $0 in the snapshot) remain `NotApplicable`
/// and are not "filled" — there is nothing to fill.
pub(crate) fn gap_fill_cost(
    model: Option<&str>,
    tokens: &TokenUsage,
    snapshot: &PricingSnapshot,
    current: &UsageCost,
) -> UsageCost {
    let UsageCost::Valued { amount_micros, .. } = current else {
        return current.clone();
    };
    if *amount_micros != 0 || tokens.total_tokens() == 0 {
        return current.clone();
    }
    let Some(model) = model else {
        return current.clone();
    };
    let Some(entry) = snapshot.find(model) else {
        return current.clone();
    };
    let Some(micros) = price_tokens(tokens, entry) else {
        return current.clone();
    };
    if micros == 0 {
        return current.clone();
    }
    UsageCost::Valued {
        amount_micros: micros,
        currency: CurrencyCode::new("USD").expect("USD is a valid ISO-shaped currency"),
        kind: CostKind::BurnlyCalculated,
        status: ValuedCostStatus::Estimated,
    }
}

fn price_tokens(tokens: &TokenUsage, entry: PricingEntry) -> Option<u64> {
    let input = tokens.input_tokens().unwrap_or(0) as f64 * entry.input;
    let output = tokens.output_tokens().unwrap_or(0) as f64 * entry.output;
    let cache_read =
        tokens.cache_read_tokens().unwrap_or(0) as f64 * entry.cache_read.unwrap_or(0.0);
    let cache_write = tokens.cache_creation_tokens().unwrap_or(0) as f64
        * entry.cache_write.unwrap_or(entry.input);
    let total = input + output + cache_read + cache_write;
    usd_to_micros(total)
}

/// Apply gap-fill to the model breakdowns of a candidate, returning the new
/// aggregate cost (sum of per-model micros when any became valued).
fn gap_fill_model_breakdowns(
    breakdowns: &mut [ModelUsageCandidate],
    snapshot: &PricingSnapshot,
) -> UsageCost {
    let mut total_micros = 0_u64;
    let mut saw_valued = false;
    for model in breakdowns.iter_mut() {
        let filled = gap_fill_cost(
            Some(&model.raw_model_id),
            &model.tokens,
            snapshot,
            &model.cost,
        );
        model.cost = filled;
        if let UsageCost::Valued { amount_micros, .. } = model.cost {
            total_micros = total_micros.saturating_add(amount_micros);
            saw_valued = true;
        }
    }
    if saw_valued {
        UsageCost::Valued {
            amount_micros: total_micros,
            currency: CurrencyCode::new("USD").expect("USD is a valid ISO-shaped currency"),
            kind: CostKind::BurnlyCalculated,
            status: ValuedCostStatus::Estimated,
        }
    } else {
        UsageCost::Unavailable {
            kind: CostKind::BurnlyCalculated,
        }
    }
}

/// Gap-fill a daily candidate: fill each model breakdown's zero cost, then
/// set the aggregate cost to the sum of the filled breakdowns.
pub(crate) fn gap_fill_daily(candidate: &mut DailyUsageCandidate, snapshot: &PricingSnapshot) {
    candidate.cost = gap_fill_model_breakdowns(&mut candidate.model_breakdowns, snapshot);
}

/// Gap-fill a session candidate: fill the single model breakdown's zero cost
/// and propagate it to the session cost.
pub(crate) fn gap_fill_session(candidate: &mut SessionUsageCandidate, snapshot: &PricingSnapshot) {
    candidate.cost = gap_fill_model_breakdowns(&mut candidate.model_breakdowns, snapshot);
}

/// Calculator handle owned by collectors; loads the snapshot once.
#[derive(Debug, Clone)]
pub(crate) struct BurnlyCostCalculator {
    snapshot: PricingSnapshot,
}

impl BurnlyCostCalculator {
    pub(crate) fn new() -> Self {
        Self {
            snapshot: PricingSnapshot::load(),
        }
    }

    pub(crate) fn snapshot(&self) -> &PricingSnapshot {
        &self.snapshot
    }

    pub(crate) fn calculate(&self, model: &str, tokens: &TokenUsage) -> CostCalculation {
        calculate_cost(model, tokens, &self.snapshot)
    }
}

impl Default for BurnlyCostCalculator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(input: u64, output: u64, cache_read: u64, cache_write: u64) -> TokenUsage {
        TokenUsage::new(
            Some(input),
            Some(output),
            Some(cache_write),
            Some(cache_read),
            input + output + cache_read + cache_write,
        )
        .expect("tokens")
    }

    fn snapshot() -> PricingSnapshot {
        PricingSnapshot::load()
    }

    #[test]
    fn prices_tokens_into_micros() {
        let calc = calculate_cost(
            "deepseek-v4-flash",
            &tokens(1_000_000, 500_000, 0, 0),
            &snapshot(),
        );
        match calc.cost {
            UsageCost::Valued {
                amount_micros,
                kind,
                status,
                ..
            } => {
                // 1M input × $0.148/M + 0.5M output × $0.296/M = $0.296
                assert_eq!(amount_micros, 296_000);
                assert_eq!(kind, CostKind::BurnlyCalculated);
                assert_eq!(status, ValuedCostStatus::Estimated);
            }
            other => panic!("expected valued cost, got {other:?}"),
        }
    }

    #[test]
    fn zero_tokens_is_not_applicable() {
        let calc = calculate_cost("deepseek-v4-flash", &tokens(0, 0, 0, 0), &snapshot());
        assert!(matches!(
            calc.cost,
            UsageCost::NotApplicable {
                kind: CostKind::BurnlyCalculated
            }
        ));
    }

    #[test]
    fn unknown_model_is_unavailable() {
        let calc = calculate_cost("totally-unknown-model", &tokens(10, 10, 0, 0), &snapshot());
        assert!(matches!(
            calc.cost,
            UsageCost::Unavailable {
                kind: CostKind::BurnlyCalculated
            }
        ));
    }

    #[test]
    fn explicit_free_model_is_not_applicable() {
        let calc = calculate_cost("deepseek-v4-flash-free", &tokens(10, 10, 0, 0), &snapshot());
        assert!(matches!(
            calc.cost,
            UsageCost::NotApplicable {
                kind: CostKind::BurnlyCalculated
            }
        ));
    }

    #[test]
    fn gap_fill_replaces_zero_cost_with_positive_tokens() {
        let current = UsageCost::Valued {
            amount_micros: 0,
            currency: CurrencyCode::new("USD").expect("USD"),
            kind: CostKind::SourceReported,
            status: ValuedCostStatus::Estimated,
        };
        let filled = gap_fill_cost(
            Some("deepseek-v4-flash"),
            &tokens(1_000_000, 0, 0, 0),
            &snapshot(),
            &current,
        );
        match filled {
            UsageCost::Valued {
                amount_micros,
                kind,
                ..
            } => {
                assert_eq!(amount_micros, 148_000);
                assert_eq!(kind, CostKind::BurnlyCalculated);
            }
            other => panic!("expected filled cost, got {other:?}"),
        }
    }

    #[test]
    fn gap_fill_keeps_positive_cost() {
        let current = UsageCost::Valued {
            amount_micros: 5_000,
            currency: CurrencyCode::new("USD").expect("USD"),
            kind: CostKind::SourceReported,
            status: ValuedCostStatus::Estimated,
        };
        let filled = gap_fill_cost(
            Some("deepseek-v4-flash"),
            &tokens(1_000_000, 0, 0, 0),
            &snapshot(),
            &current,
        );
        assert_eq!(filled, current);
    }

    #[test]
    fn gap_fill_keeps_free_model_not_applicable() {
        let current = UsageCost::NotApplicable {
            kind: CostKind::SourceReported,
        };
        let filled = gap_fill_cost(
            Some("deepseek-v4-flash-free"),
            &tokens(1_000_000, 0, 0, 0),
            &snapshot(),
            &current,
        );
        assert_eq!(filled, current);
    }

    #[test]
    fn gap_fill_daily_fills_zero_cost_breakdowns() {
        use crate::application::collection::{
            CandidateProvenance, CollectionId, CollectorKey, DailyUsageCandidate,
        };
        use crate::domain::source::SourceKey;
        use chrono::NaiveDate;

        let token_usage = tokens(1_000_000, 0, 0, 0);
        let mut candidate = DailyUsageCandidate {
            provenance: CandidateProvenance {
                source: SourceKey::GrokBuild,
                collector: CollectorKey::new("grok-build").expect("collector"),
                collector_version: "local".to_owned(),
                profile_version: 1,
                collection_id: CollectionId::new("test").expect("collection"),
                observed_at: chrono::Utc::now(),
                data_quality: crate::domain::usage::DataQuality::Complete,
                warnings: Vec::new(),
            },
            source_key: "grok-build:daily:v1:UTC:2026-08-08".to_owned(),
            usage_date: NaiveDate::from_ymd_opt(2026, 8, 8).expect("date"),
            aggregation_timezone: "UTC".to_owned(),
            tokens: token_usage.clone(),
            cost: UsageCost::Valued {
                amount_micros: 0,
                currency: CurrencyCode::new("USD").expect("USD"),
                kind: CostKind::SourceReported,
                status: ValuedCostStatus::Estimated,
            },
            model_breakdowns: vec![ModelUsageCandidate {
                raw_model_id: "deepseek-v4-flash".to_owned(),
                tokens: token_usage,
                cost: UsageCost::Valued {
                    amount_micros: 0,
                    currency: CurrencyCode::new("USD").expect("USD"),
                    kind: CostKind::SourceReported,
                    status: ValuedCostStatus::Estimated,
                },
            }],
        };

        gap_fill_daily(&mut candidate, &snapshot());

        match &candidate.cost {
            UsageCost::Valued {
                amount_micros,
                kind,
                ..
            } => {
                assert_eq!(*amount_micros, 148_000);
                assert_eq!(*kind, CostKind::BurnlyCalculated);
            }
            other => panic!("expected filled aggregate cost, got {other:?}"),
        }
        match &candidate.model_breakdowns[0].cost {
            UsageCost::Valued {
                amount_micros,
                kind,
                ..
            } => {
                assert_eq!(*amount_micros, 148_000);
                assert_eq!(*kind, CostKind::BurnlyCalculated);
            }
            other => panic!("expected filled model cost, got {other:?}"),
        }
    }

    #[test]
    fn gap_fill_session_fills_zero_cost_breakdown() {
        use crate::application::collection::{
            CandidateProvenance, CollectionId, CollectorKey, SessionUsageCandidate,
        };
        use crate::domain::source::SourceKey;

        let token_usage = tokens(1_000_000, 0, 0, 0);
        let mut candidate = SessionUsageCandidate {
            provenance: CandidateProvenance {
                source: SourceKey::CommandCode,
                collector: CollectorKey::new("command-code").expect("collector"),
                collector_version: "local".to_owned(),
                profile_version: 1,
                collection_id: CollectionId::new("test").expect("collection"),
                observed_at: chrono::Utc::now(),
                data_quality: crate::domain::usage::DataQuality::Complete,
                warnings: Vec::new(),
            },
            source_key: "command-code:session:v1:sess-1:deepseek-v4-flash".to_owned(),
            source_session_id: "sess-1".to_owned(),
            project_path: None,
            first_activity_at: None,
            last_activity_at: None,
            tokens: token_usage.clone(),
            cost: UsageCost::Valued {
                amount_micros: 0,
                currency: CurrencyCode::new("USD").expect("USD"),
                kind: CostKind::SourceReported,
                status: ValuedCostStatus::Estimated,
            },
            model_breakdowns: vec![ModelUsageCandidate {
                raw_model_id: "deepseek-v4-flash".to_owned(),
                tokens: token_usage,
                cost: UsageCost::Valued {
                    amount_micros: 0,
                    currency: CurrencyCode::new("USD").expect("USD"),
                    kind: CostKind::SourceReported,
                    status: ValuedCostStatus::Estimated,
                },
            }],
        };

        gap_fill_session(&mut candidate, &snapshot());

        match &candidate.cost {
            UsageCost::Valued {
                amount_micros,
                kind,
                ..
            } => {
                assert_eq!(*amount_micros, 148_000);
                assert_eq!(*kind, CostKind::BurnlyCalculated);
            }
            other => panic!("expected filled session cost, got {other:?}"),
        }
    }

    #[test]
    fn gap_fill_daily_keeps_free_model_unfilled() {
        use crate::application::collection::{
            CandidateProvenance, CollectionId, CollectorKey, DailyUsageCandidate,
        };
        use crate::domain::source::SourceKey;
        use chrono::NaiveDate;

        let token_usage = tokens(1_000_000, 0, 0, 0);
        let not_applicable = UsageCost::NotApplicable {
            kind: CostKind::SourceReported,
        };
        let mut candidate = DailyUsageCandidate {
            provenance: CandidateProvenance {
                source: SourceKey::GrokBuild,
                collector: CollectorKey::new("grok-build").expect("collector"),
                collector_version: "local".to_owned(),
                profile_version: 1,
                collection_id: CollectionId::new("test").expect("collection"),
                observed_at: chrono::Utc::now(),
                data_quality: crate::domain::usage::DataQuality::Complete,
                warnings: Vec::new(),
            },
            source_key: "grok-build:daily:v1:UTC:2026-08-08".to_owned(),
            usage_date: NaiveDate::from_ymd_opt(2026, 8, 8).expect("date"),
            aggregation_timezone: "UTC".to_owned(),
            tokens: token_usage.clone(),
            cost: not_applicable.clone(),
            model_breakdowns: vec![ModelUsageCandidate {
                raw_model_id: "deepseek-v4-flash-free".to_owned(),
                tokens: token_usage,
                cost: not_applicable.clone(),
            }],
        };

        gap_fill_daily(&mut candidate, &snapshot());

        assert!(matches!(
            candidate.model_breakdowns[0].cost,
            UsageCost::NotApplicable { .. }
        ));
    }
}
