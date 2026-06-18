use thiserror::Error;

use crate::domain::usage::CurrencyCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct BudgetId(i64);

impl BudgetId {
    pub(crate) fn new(value: i64) -> Result<Self, BudgetValidationError> {
        if value <= 0 {
            return Err(BudgetValidationError::BudgetId);
        }
        Ok(Self(value))
    }

    pub(crate) const fn value(self) -> i64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BudgetPeriod {
    Daily,
    Weekly,
    Monthly,
}

impl BudgetPeriod {
    pub(crate) fn parse(value: &str) -> Result<Self, BudgetValidationError> {
        match value {
            "daily" => Ok(Self::Daily),
            "weekly" => Ok(Self::Weekly),
            "monthly" => Ok(Self::Monthly),
            _ => Err(BudgetValidationError::Period),
        }
    }

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Daily => "daily",
            Self::Weekly => "weekly",
            Self::Monthly => "monthly",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BudgetLimit {
    Tokens(u64),
    CostMicros {
        amount_micros: u64,
        currency: CurrencyCode,
    },
}

impl BudgetLimit {
    pub(crate) fn tokens(value: u64) -> Result<Self, BudgetValidationError> {
        positive(value).map(Self::Tokens)
    }

    pub(crate) fn cost_micros(
        amount_micros: u64,
        currency: CurrencyCode,
    ) -> Result<Self, BudgetValidationError> {
        Ok(Self::CostMicros {
            amount_micros: positive(amount_micros)?,
            currency,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BudgetScope {
    Global,
    Source(i64),
}

impl BudgetScope {
    pub(crate) fn source(source_id: i64) -> Result<Self, BudgetValidationError> {
        if source_id <= 0 {
            return Err(BudgetValidationError::SourceId);
        }
        Ok(Self::Source(source_id))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BudgetThreshold {
    basis_points: u32,
    enabled: bool,
}

impl BudgetThreshold {
    pub(crate) fn new(basis_points: u32, enabled: bool) -> Result<Self, BudgetValidationError> {
        if basis_points == 0 {
            return Err(BudgetValidationError::Threshold);
        }
        Ok(Self {
            basis_points,
            enabled,
        })
    }

    pub(crate) const fn basis_points(self) -> u32 {
        self.basis_points
    }

    pub(crate) const fn enabled(self) -> bool {
        self.enabled
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BudgetDefinition {
    name: String,
    limit: BudgetLimit,
    period: BudgetPeriod,
    scope: BudgetScope,
    enabled: bool,
    thresholds: Vec<BudgetThreshold>,
}

impl BudgetDefinition {
    pub(crate) fn new(
        name: impl Into<String>,
        limit: BudgetLimit,
        period: BudgetPeriod,
        scope: BudgetScope,
        enabled: bool,
        thresholds: Vec<BudgetThreshold>,
    ) -> Result<Self, BudgetValidationError> {
        let name = name.into().trim().to_owned();
        if name.is_empty() {
            return Err(BudgetValidationError::Name);
        }

        let mut thresholds = thresholds;
        thresholds.sort_unstable_by_key(|threshold| threshold.basis_points);
        if thresholds
            .windows(2)
            .any(|pair| pair[0].basis_points == pair[1].basis_points)
        {
            return Err(BudgetValidationError::DuplicateThreshold);
        }

        Ok(Self {
            name,
            limit,
            period,
            scope,
            enabled,
            thresholds,
        })
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) const fn limit(&self) -> &BudgetLimit {
        &self.limit
    }

    pub(crate) const fn period(&self) -> BudgetPeriod {
        self.period
    }

    pub(crate) const fn scope(&self) -> BudgetScope {
        self.scope
    }

    pub(crate) const fn enabled(&self) -> bool {
        self.enabled
    }

    pub(crate) fn thresholds(&self) -> &[BudgetThreshold] {
        &self.thresholds
    }

    #[cfg(test)]
    pub(crate) fn with_enabled(&self, enabled: bool) -> Self {
        let mut definition = self.clone();
        definition.enabled = enabled;
        definition
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Budget {
    id: BudgetId,
    revision: i64,
    definition: BudgetDefinition,
}

impl Budget {
    pub(crate) fn new(
        id: BudgetId,
        revision: i64,
        definition: BudgetDefinition,
    ) -> Result<Self, BudgetValidationError> {
        if revision <= 0 {
            return Err(BudgetValidationError::Revision);
        }
        Ok(Self {
            id,
            revision,
            definition,
        })
    }

    #[allow(
        dead_code,
        reason = "Phase 8D will expose stored budget documents through IPC"
    )]
    pub(crate) const fn id(&self) -> BudgetId {
        self.id
    }

    #[allow(
        dead_code,
        reason = "Phase 8D will expose stored budget documents through IPC"
    )]
    pub(crate) const fn revision(&self) -> i64 {
        self.revision
    }

    #[allow(
        dead_code,
        reason = "Phase 8D will expose stored budget documents through IPC"
    )]
    pub(crate) const fn definition(&self) -> &BudgetDefinition {
        &self.definition
    }
}

fn positive(value: u64) -> Result<u64, BudgetValidationError> {
    if value == 0 || value > i64::MAX as u64 {
        return Err(BudgetValidationError::Limit);
    }
    Ok(value)
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BudgetValidationError {
    #[error("budget ID must be positive")]
    BudgetId,
    #[error("budget name must not be empty")]
    Name,
    #[error("budget limit must be within the supported positive integer range")]
    Limit,
    #[error("budget period is invalid")]
    Period,
    #[error("source ID must be positive")]
    SourceId,
    #[error("budget threshold must be positive")]
    Threshold,
    #[error("budget thresholds must be unique")]
    DuplicateThreshold,
    #[error("budget revision must be positive")]
    Revision,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limit_variants_prevent_metric_currency_mismatches() {
        assert_eq!(BudgetLimit::tokens(0), Err(BudgetValidationError::Limit));
        assert_eq!(
            BudgetLimit::cost_micros(250_000, CurrencyCode::new("USD").expect("valid currency"))
                .expect("cost limit"),
            BudgetLimit::CostMicros {
                amount_micros: 250_000,
                currency: CurrencyCode::new("USD").expect("valid currency"),
            }
        );
    }

    #[test]
    fn definition_normalizes_threshold_order_and_rejects_duplicates() {
        let definition = BudgetDefinition::new(
            " Monthly tokens ",
            BudgetLimit::tokens(10_000).expect("limit"),
            BudgetPeriod::Monthly,
            BudgetScope::Global,
            true,
            vec![
                BudgetThreshold::new(10_000, true).expect("threshold"),
                BudgetThreshold::new(8_000, true).expect("threshold"),
            ],
        )
        .expect("definition");

        assert_eq!(definition.name(), "Monthly tokens");
        assert_eq!(
            definition
                .thresholds()
                .iter()
                .map(|threshold| threshold.basis_points())
                .collect::<Vec<_>>(),
            vec![8_000, 10_000]
        );
        assert_eq!(
            BudgetDefinition::new(
                "Tokens",
                BudgetLimit::tokens(1).expect("limit"),
                BudgetPeriod::Daily,
                BudgetScope::Global,
                true,
                vec![
                    BudgetThreshold::new(8_000, true).expect("threshold"),
                    BudgetThreshold::new(8_000, false).expect("threshold"),
                ],
            ),
            Err(BudgetValidationError::DuplicateThreshold)
        );
    }

    #[test]
    fn validates_identity_scope_threshold_and_revision_boundaries() {
        assert_eq!(BudgetId::new(0), Err(BudgetValidationError::BudgetId));
        assert_eq!(
            BudgetScope::source(-1),
            Err(BudgetValidationError::SourceId)
        );
        assert_eq!(
            BudgetThreshold::new(0, true),
            Err(BudgetValidationError::Threshold)
        );
        let definition = BudgetDefinition::new(
            "Tokens",
            BudgetLimit::tokens(1).expect("limit"),
            BudgetPeriod::Daily,
            BudgetScope::Global,
            true,
            vec![],
        )
        .expect("definition");
        assert_eq!(
            Budget::new(BudgetId::new(1).expect("id"), 0, definition),
            Err(BudgetValidationError::Revision)
        );
    }
}
