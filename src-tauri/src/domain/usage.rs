#![expect(
    dead_code,
    reason = "Phase 3A defines usage values consumed by later Phase 3 adapters"
)]

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TokenUsage {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cache_creation_tokens: Option<u64>,
    cache_read_tokens: Option<u64>,
    total_tokens: u64,
    unclassified_tokens: Option<u64>,
}

impl TokenUsage {
    pub(crate) fn new(
        input_tokens: Option<u64>,
        output_tokens: Option<u64>,
        cache_creation_tokens: Option<u64>,
        cache_read_tokens: Option<u64>,
        total_tokens: u64,
    ) -> Result<Self, UsageValidationError> {
        let classified_tokens = [
            input_tokens,
            output_tokens,
            cache_creation_tokens,
            cache_read_tokens,
        ]
        .into_iter()
        .flatten()
        .try_fold(0_u64, u64::checked_add)
        .ok_or(UsageValidationError::TokenOverflow)?;

        if classified_tokens > total_tokens {
            return Err(UsageValidationError::ClassifiedTokensExceedTotal {
                classified_tokens,
                total_tokens,
            });
        }

        let all_categories_known = input_tokens.is_some()
            && output_tokens.is_some()
            && cache_creation_tokens.is_some()
            && cache_read_tokens.is_some();

        Ok(Self {
            input_tokens,
            output_tokens,
            cache_creation_tokens,
            cache_read_tokens,
            total_tokens,
            unclassified_tokens: all_categories_known.then_some(total_tokens - classified_tokens),
        })
    }

    pub(crate) const fn input_tokens(&self) -> Option<u64> {
        self.input_tokens
    }

    pub(crate) const fn output_tokens(&self) -> Option<u64> {
        self.output_tokens
    }

    pub(crate) const fn cache_creation_tokens(&self) -> Option<u64> {
        self.cache_creation_tokens
    }

    pub(crate) const fn cache_read_tokens(&self) -> Option<u64> {
        self.cache_read_tokens
    }

    pub(crate) const fn total_tokens(&self) -> u64 {
        self.total_tokens
    }

    pub(crate) const fn unclassified_tokens(&self) -> Option<u64> {
        self.unclassified_tokens
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UsageCost {
    Valued {
        amount_micros: u64,
        currency: CurrencyCode,
        kind: CostKind,
        status: ValuedCostStatus,
    },
    NotApplicable {
        kind: CostKind,
    },
    Unavailable {
        kind: CostKind,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CostKind {
    SourceReported,
    CollectorCalculated,
    CollectorMixed,
    BurnlyCalculated,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValuedCostStatus {
    Available,
    Estimated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CurrencyCode(String);

impl CurrencyCode {
    pub(crate) fn new(value: impl Into<String>) -> Result<Self, UsageValidationError> {
        let value = value.into();
        let valid = value.len() == 3
            && value
                .bytes()
                .all(|character| character.is_ascii_uppercase());

        if !valid {
            return Err(UsageValidationError::InvalidCurrencyCode);
        }

        Ok(Self(value))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DataQuality {
    Complete,
    Partial,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UsageSession {
    pub session_id: i64,
    pub source_id: i64,
    pub source_session_id: String,
    pub project_id: Option<i64>,
    pub project_path: Option<String>,
    pub first_activity_at_ms: Option<i64>,
    pub last_activity_at_ms: Option<i64>,
    pub tokens: TokenUsage,
    pub cost: UsageCost,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionDetail {
    pub session: UsageSession,
    pub model_breakdowns: Vec<SessionModelUsage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionModelUsage {
    pub raw_model_id: Option<String>,
    pub tokens: TokenUsage,
    pub cost: UsageCost,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub(crate) enum UsageValidationError {
    #[error("classified token total overflowed")]
    TokenOverflow,

    #[error("classified tokens {classified_tokens} exceed authoritative total {total_tokens}")]
    ClassifiedTokensExceedTotal {
        classified_tokens: u64,
        total_tokens: u64,
    },

    #[error("currency code must be three uppercase ASCII letters")]
    InvalidCurrencyCode,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_unclassified_tokens_only_when_all_categories_are_known() {
        let complete = TokenUsage::new(Some(10), Some(20), Some(5), Some(5), 50)
            .expect("valid complete usage");
        let partial =
            TokenUsage::new(Some(10), Some(20), None, Some(5), 50).expect("valid partial usage");

        assert_eq!(complete.unclassified_tokens(), Some(10));
        assert_eq!(partial.unclassified_tokens(), None);
    }

    #[test]
    fn rejects_classified_tokens_above_authoritative_total() {
        let error = TokenUsage::new(Some(30), Some(30), Some(0), Some(0), 50)
            .expect_err("invalid component total");

        assert_eq!(
            error,
            UsageValidationError::ClassifiedTokensExceedTotal {
                classified_tokens: 60,
                total_tokens: 50,
            }
        );
    }

    #[test]
    fn currency_code_requires_iso_shaped_value() {
        assert_eq!(
            CurrencyCode::new("USD").expect("valid currency").as_str(),
            "USD"
        );
        assert_eq!(
            CurrencyCode::new("usd").expect_err("lowercase currency"),
            UsageValidationError::InvalidCurrencyCode
        );
    }
}
