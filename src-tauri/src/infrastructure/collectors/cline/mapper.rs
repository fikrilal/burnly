use std::collections::BTreeMap;

use chrono::{DateTime, NaiveDate, Utc};
use chrono_tz::Tz;
use thiserror::Error;

use crate::{
    application::collection::{
        CandidateProvenance, CollectionId, CollectionScope, CollectorKey, DailyUsageCandidate,
        ModelUsageCandidate, SessionUsageCandidate,
    },
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
use super::{ClineMessageUsage, ClineSessionRow, ClineUsageMetrics};

const PROFILE_VERSION: u16 = 1;

type ActivityWindow = Option<(DateTime<Utc>, DateTime<Utc>)>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClineMappingContext {
    collector: CollectorKey,
    collector_version: String,
    collection_id: CollectionId,
    observed_at: DateTime<Utc>,
}

impl ClineMappingContext {
    pub(crate) fn new(
        collector: CollectorKey,
        collector_version: String,
        collection_id: CollectionId,
        observed_at: DateTime<Utc>,
    ) -> Result<Self, ClineMappingError> {
        if collector_version.trim().is_empty() {
            return Err(ClineMappingError::EmptyCollectorVersion);
        }
        Ok(Self {
            collector,
            collector_version,
            collection_id,
            observed_at,
        })
    }

    pub(crate) fn provenance(&self) -> CandidateProvenance {
        provenance(&MappingIdentity {
            source: SourceKey::Cline,
            collector: self.collector.clone(),
            collector_version: self.collector_version.clone(),
            profile_version: PROFILE_VERSION,
            collection_id: self.collection_id.clone(),
            observed_at: self.observed_at,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClineSessionMessages {
    pub session: ClineSessionRow,
    pub messages: Vec<ClineMessageUsage>,
}

pub(crate) fn map_daily(
    sessions: Vec<ClineSessionMessages>,
    timezone: &str,
    scope: &CollectionScope,
    context: &ClineMappingContext,
) -> Result<Vec<DailyUsageCandidate>, ClineMappingError> {
    let timezone = timezone
        .parse::<Tz>()
        .map_err(|_| ClineMappingError::InvalidTimezone)?;
    let mut buckets = BTreeMap::<(NaiveDate, String), ClineUsageAccumulator>::new();

    for session in sessions {
        for message in session.messages {
            let usage_date = local_date_from_millis(
                message.timestamp_ms,
                timezone,
                ClineMappingError::InvalidTimestamp,
            )?;
            if !date_in_scope(usage_date, scope) {
                continue;
            }
            buckets
                .entry((usage_date, session.session.model.clone()))
                .or_default()
                .add(message.metrics)?;
        }
    }

    buckets
        .into_iter()
        .map(|((usage_date, model), usage)| {
            let tokens = usage.tokens()?;
            let cost = cost_from_micros(usage.cost_micros, tokens.total_tokens())?;
            Ok(DailyUsageCandidate {
                provenance: context.provenance(),
                source_key: daily_source_key(SourceKey::Cline, usage_date, timezone.name())?,
                usage_date,
                aggregation_timezone: timezone.name().to_owned(),
                tokens: tokens.clone(),
                cost: cost.clone(),
                model_breakdowns: vec![ModelUsageCandidate {
                    raw_model_id: model,
                    tokens,
                    cost,
                }],
            })
        })
        .collect()
}

pub(crate) fn map_sessions(
    sessions: Vec<ClineSessionMessages>,
    context: &ClineMappingContext,
) -> Result<Vec<SessionUsageCandidate>, ClineMappingError> {
    sessions
        .into_iter()
        .map(|session| map_session(session, context))
        .collect()
}

fn map_session(
    session: ClineSessionMessages,
    context: &ClineMappingContext,
) -> Result<SessionUsageCandidate, ClineMappingError> {
    let usage = session
        .session
        .usage
        .ok_or(ClineMappingError::MissingSessionUsage)?;
    let tokens = tokens_from_metrics(usage)?;
    let cost = cost_from_micros(usage.cost_micros, tokens.total_tokens())?;
    let activity = activity_window(&session.messages)?;

    Ok(SessionUsageCandidate {
        provenance: context.provenance(),
        source_key: session_source_key(SourceKey::Cline, &session.session.session_id)?,
        source_session_id: session.session.session_id,
        project_path: None,
        first_activity_at: activity.map(|(first, _)| first),
        last_activity_at: activity.map(|(_, last)| last),
        tokens: tokens.clone(),
        cost: cost.clone(),
        model_breakdowns: vec![ModelUsageCandidate {
            raw_model_id: session.session.model,
            tokens,
            cost,
        }],
    })
}

fn activity_window(messages: &[ClineMessageUsage]) -> Result<ActivityWindow, ClineMappingError> {
    let Some(first) = messages.iter().map(|message| message.timestamp_ms).min() else {
        return Ok(None);
    };
    let last = messages
        .iter()
        .map(|message| message.timestamp_ms)
        .max()
        .expect("min implies max");

    Ok(Some((timestamp(first)?, timestamp(last)?)))
}

#[derive(Debug, Default)]
struct ClineUsageAccumulator {
    input_tokens: u64,
    output_tokens: u64,
    cache_read_tokens: u64,
    cache_write_tokens: u64,
    cost_micros: u64,
}

impl ClineUsageAccumulator {
    fn add(&mut self, metrics: ClineUsageMetrics) -> Result<(), ClineMappingError> {
        self.input_tokens = checked_add_u64(
            self.input_tokens,
            metrics.input_tokens,
            ClineMappingError::TokenOverflow,
        )?;
        self.output_tokens = checked_add_u64(
            self.output_tokens,
            metrics.output_tokens,
            ClineMappingError::TokenOverflow,
        )?;
        self.cache_read_tokens = checked_add_u64(
            self.cache_read_tokens,
            metrics.cache_read_tokens,
            ClineMappingError::TokenOverflow,
        )?;
        self.cache_write_tokens = checked_add_u64(
            self.cache_write_tokens,
            metrics.cache_write_tokens,
            ClineMappingError::TokenOverflow,
        )?;
        self.cost_micros = checked_add_u64(
            self.cost_micros,
            metrics.cost_micros,
            ClineMappingError::CostOutOfRange,
        )?;
        Ok(())
    }

    fn tokens(&self) -> Result<TokenUsage, ClineMappingError> {
        let total = checked_add_u64(
            checked_add_u64(
                checked_add_u64(
                    self.input_tokens,
                    self.output_tokens,
                    ClineMappingError::TokenOverflow,
                )?,
                self.cache_read_tokens,
                ClineMappingError::TokenOverflow,
            )?,
            self.cache_write_tokens,
            ClineMappingError::TokenOverflow,
        )?;
        TokenUsage::new(
            Some(self.input_tokens),
            Some(self.output_tokens),
            Some(self.cache_write_tokens),
            Some(self.cache_read_tokens),
            total,
        )
        .map_err(Into::into)
    }
}

fn tokens_from_metrics(metrics: ClineUsageMetrics) -> Result<TokenUsage, ClineMappingError> {
    let total = checked_add_u64(
        checked_add_u64(
            checked_add_u64(
                metrics.input_tokens,
                metrics.output_tokens,
                ClineMappingError::TokenOverflow,
            )?,
            metrics.cache_read_tokens,
            ClineMappingError::TokenOverflow,
        )?,
        metrics.cache_write_tokens,
        ClineMappingError::TokenOverflow,
    )?;
    TokenUsage::new(
        Some(metrics.input_tokens),
        Some(metrics.output_tokens),
        Some(metrics.cache_write_tokens),
        Some(metrics.cache_read_tokens),
        total,
    )
    .map_err(Into::into)
}

fn cost_from_micros(cost_micros: u64, total_tokens: u64) -> Result<UsageCost, ClineMappingError> {
    if cost_micros == 0 {
        return Ok(if total_tokens == 0 {
            UsageCost::NotApplicable {
                kind: CostKind::SourceReported,
            }
        } else {
            UsageCost::Unavailable {
                kind: CostKind::SourceReported,
            }
        });
    }

    Ok(UsageCost::Valued {
        amount_micros: cost_micros,
        currency: CurrencyCode::new("USD").expect("USD is a valid ISO-shaped currency"),
        kind: CostKind::SourceReported,
        status: ValuedCostStatus::Estimated,
    })
}

fn timestamp(timestamp_ms: i64) -> Result<DateTime<Utc>, ClineMappingError> {
    utc_from_millis(timestamp_ms, ClineMappingError::InvalidTimestamp)
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub(crate) enum ClineMappingError {
    #[error("cline mapping requires a collector version")]
    EmptyCollectorVersion,
    #[error("cline mapping requires a valid timezone")]
    InvalidTimezone,
    #[error("cline mapping received an invalid timestamp")]
    InvalidTimestamp,
    #[error("cline mapping received session without usage")]
    MissingSessionUsage,
    #[error("cline token total overflowed")]
    TokenOverflow,
    #[error("cline cost exceeded the supported micro-unit range")]
    CostOutOfRange,
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error(transparent)]
    Usage(#[from] UsageValidationError),
}
