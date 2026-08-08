//! Command Code usage mapping.
//!
//! Maps parsed transcript usage into Burnly daily and session candidates.
//! Token fields map directly (input/output/cache-read/cache-write; canonical
//! total is their sum), `costUsd` converts to integer micros deterministically,
//! and records dedupe by `(session id, message id)`.

use std::collections::{BTreeMap, BTreeSet};

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
    checked_add_u64, date_in_scope, local_date_from_millis, provenance, MappingIdentity,
};
use super::transcript_parser::{ParsedTranscript, TranscriptUsage};

const PROFILE_VERSION: u16 = 1;
const USD_MICROS_PER_DOLLAR: f64 = 1_000_000.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CommandCodeMappingContext {
    collector: CollectorKey,
    collector_version: String,
    collection_id: CollectionId,
    observed_at: DateTime<Utc>,
}

impl CommandCodeMappingContext {
    pub(crate) fn new(
        collector: CollectorKey,
        collector_version: String,
        collection_id: CollectionId,
        observed_at: DateTime<Utc>,
    ) -> Result<Self, CommandCodeMappingError> {
        if collector_version.trim().is_empty() {
            return Err(CommandCodeMappingError::EmptyCollectorVersion);
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
            source: SourceKey::CommandCode,
            collector: self.collector.clone(),
            collector_version: self.collector_version.clone(),
            profile_version: PROFILE_VERSION,
            collection_id: self.collection_id.clone(),
            observed_at: self.observed_at,
        })
    }
}

/// Map parsed transcripts into daily and session candidates for the scope.
pub(crate) fn map_daily(
    transcripts: Vec<ParsedTranscript>,
    timezone: &str,
    scope: &CollectionScope,
    context: &CommandCodeMappingContext,
) -> Result<Vec<DailyUsageCandidate>, CommandCodeMappingError> {
    let timezone = timezone
        .parse::<Tz>()
        .map_err(|_| CommandCodeMappingError::InvalidTimezone)?;
    let mut buckets = BTreeMap::<NaiveDate, CommandCodeDailyBucket>::new();

    for transcript in transcripts {
        for usage in dedupe_usages(&transcript) {
            let usage_date = local_date_from_millis(
                usage.timestamp.timestamp_millis(),
                timezone,
                CommandCodeMappingError::InvalidTimestamp,
            )?;
            if !date_in_scope(usage_date, scope) {
                continue;
            }
            buckets.entry(usage_date).or_default().add(usage)?;
        }
    }

    buckets
        .into_iter()
        .map(|(usage_date, bucket)| {
            let tokens = bucket.total.tokens()?;
            let aggregate_cost = cost(bucket.total.cost_micros, tokens.total_tokens());
            let model_breakdowns = bucket
                .models
                .into_iter()
                .map(|(model, usage)| {
                    let tokens = usage.tokens()?;
                    let cost = cost(usage.cost_micros, tokens.total_tokens());
                    Ok(ModelUsageCandidate {
                        raw_model_id: model,
                        tokens,
                        cost,
                    })
                })
                .collect::<Result<Vec<_>, CommandCodeMappingError>>()?;
            Ok(DailyUsageCandidate {
                provenance: context.provenance(),
                source_key: daily_source_key(SourceKey::CommandCode, usage_date, timezone.name())?,
                usage_date,
                aggregation_timezone: timezone.name().to_owned(),
                tokens,
                cost: aggregate_cost,
                model_breakdowns,
            })
        })
        .collect()
}

/// Map usage-bearing messages to session candidates grouped by
/// `(session id, model)`.
pub(crate) fn map_sessions(
    transcripts: Vec<ParsedTranscript>,
    context: &CommandCodeMappingContext,
) -> Result<Vec<SessionUsageCandidate>, CommandCodeMappingError> {
    let mut buckets = BTreeMap::<(String, String), CommandCodeSessionAccumulator>::new();

    for transcript in transcripts {
        for usage in dedupe_usages(&transcript) {
            let model = usage.model.clone().unwrap_or_else(|| "unknown".to_owned());
            buckets
                .entry((transcript.session_id.clone(), model.clone()))
                .or_insert_with(|| {
                    CommandCodeSessionAccumulator::new(
                        transcript.session_id.clone(),
                        model.clone(),
                        transcript.cwd.clone(),
                    )
                })
                .add(usage)?;
        }
    }

    buckets
        .into_values()
        .map(|bucket| bucket.candidate(context))
        .collect()
}

/// Dedupe usage records by `(session id, message id)`. Messages are
/// file-scoped, so the session id is part of the key.
fn dedupe_usages(transcript: &ParsedTranscript) -> Vec<&TranscriptUsage> {
    let mut seen = BTreeSet::new();
    transcript
        .usages
        .iter()
        .filter(|usage| seen.insert((transcript.session_id.as_str(), usage.message_id.as_str())))
        .collect()
}

#[derive(Debug, Default)]
struct CommandCodeDailyBucket {
    total: CommandCodeUsageAccumulator,
    models: BTreeMap<String, CommandCodeUsageAccumulator>,
}

impl CommandCodeDailyBucket {
    fn add(&mut self, usage: &TranscriptUsage) -> Result<(), CommandCodeMappingError> {
        self.total.add(usage)?;
        self.models
            .entry(usage.model.clone().unwrap_or_else(|| "unknown".to_owned()))
            .or_default()
            .add(usage)
    }
}

#[derive(Debug, Default)]
struct CommandCodeUsageAccumulator {
    input_tokens: u64,
    output_tokens: u64,
    cache_read_tokens: u64,
    cache_write_tokens: u64,
    cost_micros: u64,
}

impl CommandCodeUsageAccumulator {
    fn add(&mut self, usage: &TranscriptUsage) -> Result<(), CommandCodeMappingError> {
        self.input_tokens = checked_add(self.input_tokens, usage.tokens.input)?;
        self.output_tokens = checked_add(self.output_tokens, usage.tokens.output)?;
        self.cache_read_tokens = checked_add(self.cache_read_tokens, usage.tokens.cache_read)?;
        self.cache_write_tokens = checked_add(self.cache_write_tokens, usage.tokens.cache_write)?;
        if let Some(cost_usd) = usage.cost_usd {
            self.cost_micros = checked_add(
                self.cost_micros,
                cost_usd_to_micros(cost_usd, CommandCodeMappingError::InvalidCost)?,
            )?;
        }
        Ok(())
    }

    fn tokens(&self) -> Result<TokenUsage, CommandCodeMappingError> {
        // Command Code reports `cacheReadTokens` as a SUBSET of
        // `inputTokens` (the cache-hit portion of the prompt), not as
        // additional tokens. Summing them double-counts the cached portion.
        // Net input = input - cache_read; total = input + output + cache_write.
        let net_input = self
            .input_tokens
            .checked_sub(self.cache_read_tokens)
            .ok_or(CommandCodeMappingError::OverlappingCacheTokens)?;
        let total = checked_add(
            checked_add(self.input_tokens, self.output_tokens)?,
            self.cache_write_tokens,
        )?;
        TokenUsage::new(
            Some(net_input),
            Some(self.output_tokens),
            Some(self.cache_write_tokens),
            Some(self.cache_read_tokens),
            total,
        )
        .map_err(Into::into)
    }
}

struct CommandCodeSessionAccumulator {
    session_id: String,
    model_id: String,
    project_path: Option<String>,
    usage: CommandCodeUsageAccumulator,
    first_activity_at: Option<DateTime<Utc>>,
    last_activity_at: Option<DateTime<Utc>>,
}

impl CommandCodeSessionAccumulator {
    fn new(session_id: String, model_id: String, project_path: Option<String>) -> Self {
        Self {
            session_id,
            model_id,
            project_path,
            usage: CommandCodeUsageAccumulator::default(),
            first_activity_at: None,
            last_activity_at: None,
        }
    }

    fn add(&mut self, usage: &TranscriptUsage) -> Result<(), CommandCodeMappingError> {
        self.first_activity_at = Some(
            self.first_activity_at
                .map(|first| first.min(usage.timestamp))
                .unwrap_or(usage.timestamp),
        );
        self.last_activity_at = Some(
            self.last_activity_at
                .map(|last| last.max(usage.timestamp))
                .unwrap_or(usage.timestamp),
        );
        self.usage.add(usage)
    }

    fn candidate(
        self,
        context: &CommandCodeMappingContext,
    ) -> Result<SessionUsageCandidate, CommandCodeMappingError> {
        let tokens = self.usage.tokens()?;
        let cost = cost(self.usage.cost_micros, tokens.total_tokens());
        Ok(SessionUsageCandidate {
            provenance: context.provenance(),
            source_key: session_source_key(
                SourceKey::CommandCode,
                &format!("{}:{}", self.session_id, self.model_id),
            )?,
            source_session_id: self.session_id,
            project_path: self.project_path,
            first_activity_at: self.first_activity_at,
            last_activity_at: self.last_activity_at,
            tokens: tokens.clone(),
            cost: cost.clone(),
            model_breakdowns: vec![ModelUsageCandidate {
                raw_model_id: self.model_id,
                tokens,
                cost,
            }],
        })
    }
}

fn checked_add(left: u64, right: u64) -> Result<u64, CommandCodeMappingError> {
    checked_add_u64(left, right, CommandCodeMappingError::TokenOverflow)
}

/// Convert a USD float to integer micros deterministically (round half-up to 6
/// decimal places). Rejects negative and non-finite values.
fn cost_usd_to_micros(
    cost_usd: f64,
    error: CommandCodeMappingError,
) -> Result<u64, CommandCodeMappingError> {
    if !cost_usd.is_finite() || cost_usd < 0.0 {
        return Err(error);
    }
    // Round half-up: add 0.5 before truncating toward zero.
    let micros = (cost_usd * USD_MICROS_PER_DOLLAR + 0.5).floor();
    if micros > u64::MAX as f64 {
        return Err(error);
    }
    Ok(micros as u64)
}

fn cost(cost_micros: u64, total_tokens: u64) -> UsageCost {
    if cost_micros == 0 {
        return if total_tokens == 0 {
            UsageCost::NotApplicable {
                kind: CostKind::SourceReported,
            }
        } else {
            UsageCost::Unavailable {
                kind: CostKind::SourceReported,
            }
        };
    }
    UsageCost::Valued {
        amount_micros: cost_micros,
        currency: CurrencyCode::new("USD").expect("USD is a valid ISO-shaped currency"),
        kind: CostKind::SourceReported,
        status: ValuedCostStatus::Estimated,
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub(crate) enum CommandCodeMappingError {
    #[error("command-code mapping requires a collector version")]
    EmptyCollectorVersion,
    #[error("command-code mapping requires a valid timezone")]
    InvalidTimezone,
    #[error("command-code mapping received an invalid timestamp")]
    InvalidTimestamp,
    #[error("command-code token total overflowed")]
    TokenOverflow,
    #[error("command-code cache-read tokens exceed input tokens")]
    OverlappingCacheTokens,
    #[error("command-code cost value is invalid")]
    InvalidCost,
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error(transparent)]
    Usage(#[from] UsageValidationError),
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;
    use crate::application::collection::{CollectionId, CollectorKey};

    const VALID_TRANSCRIPT: &str = r#"{"type":"session","version":3,"id":"sess-1","timestamp":"2026-08-04T10:00:00Z","cwd":"/tmp/proj"}
{"type":"message","id":"m1","parentId":null,"timestamp":"2026-08-04T10:00:01Z","message":{"role":"user","content":[{"type":"text","text":"redacted"}]}}
{"type":"message","id":"m2","parentId":"m1","timestamp":"2026-08-04T10:00:02Z","message":{"role":"assistant","content":[{"type":"text","text":"redacted"}]},"usage":{"inputTokens":10,"outputTokens":2,"cacheReadTokens":3,"cacheWriteTokens":0,"costUsd":0.001},"model":"deepseek/deepseek-v4-flash","effort":"max"}"#;

    fn transcript_from(contents: &str) -> ParsedTranscript {
        let (_, parsed, _) = super::super::transcript_parser::parse_transcript(contents);
        parsed.expect("parsed transcript")
    }

    fn context() -> CommandCodeMappingContext {
        CommandCodeMappingContext::new(
            CollectorKey::new("command-code").expect("collector"),
            "local".to_owned(),
            CollectionId::new("command-code-test").expect("collection"),
            Utc.with_ymd_and_hms(2026, 8, 4, 1, 0, 0)
                .single()
                .expect("timestamp"),
        )
        .expect("context")
    }

    #[test]
    fn converts_cost_usd_to_micros_rounding_half_up() {
        assert_eq!(
            cost_usd_to_micros(0.001, CommandCodeMappingError::InvalidCost).expect("cost"),
            1000
        );
        assert_eq!(
            cost_usd_to_micros(0.0000005, CommandCodeMappingError::InvalidCost).expect("cost"),
            1
        );
        assert_eq!(
            cost_usd_to_micros(0.0, CommandCodeMappingError::InvalidCost).expect("cost"),
            0
        );
        assert_eq!(
            cost_usd_to_micros(1.0, CommandCodeMappingError::InvalidCost).expect("cost"),
            1_000_000
        );
    }

    #[test]
    fn rejects_negative_and_non_finite_cost() {
        assert_eq!(
            cost_usd_to_micros(-0.001, CommandCodeMappingError::InvalidCost).expect_err("cost"),
            CommandCodeMappingError::InvalidCost
        );
        assert_eq!(
            cost_usd_to_micros(f64::NAN, CommandCodeMappingError::InvalidCost).expect_err("cost"),
            CommandCodeMappingError::InvalidCost
        );
        assert_eq!(
            cost_usd_to_micros(f64::INFINITY, CommandCodeMappingError::InvalidCost)
                .expect_err("cost"),
            CommandCodeMappingError::InvalidCost
        );
    }

    #[test]
    fn maps_daily_candidate_from_transcript() {
        let transcripts = vec![transcript_from(VALID_TRANSCRIPT)];
        let context = context();

        let candidates = map_daily(
            transcripts,
            "Asia/Jakarta",
            &CollectionScope::Full,
            &context,
        )
        .expect("daily");

        assert_eq!(candidates.len(), 1);
        let candidate = &candidates[0];
        assert_eq!(
            candidate.source_key,
            "command-code:daily:v1:Asia/Jakarta:2026-08-04"
        );
        assert_eq!(candidate.tokens.input_tokens(), Some(7));
        assert_eq!(candidate.tokens.output_tokens(), Some(2));
        assert_eq!(candidate.tokens.cache_read_tokens(), Some(3));
        assert_eq!(candidate.tokens.cache_creation_tokens(), Some(0));
        // cache_read is a subset of input (10 = 7 net + 3 cached); total is
        // input + output = 12, NOT input + output + cache_read.
        assert_eq!(candidate.tokens.total_tokens(), 12);
        let cost = match &candidate.cost {
            UsageCost::Valued {
                amount_micros,
                kind,
                status,
                ..
            } => {
                assert_eq!(*amount_micros, 1000);
                assert_eq!(*kind, CostKind::SourceReported);
                assert_eq!(*status, ValuedCostStatus::Estimated);
                true
            }
            _ => false,
        };
        assert!(cost);
        assert_eq!(candidate.model_breakdowns.len(), 1);
        assert_eq!(
            candidate.model_breakdowns[0].raw_model_id,
            "deepseek/deepseek-v4-flash"
        );
    }

    #[test]
    fn cache_read_is_not_double_counted_on_top_of_input() {
        // Regression: Command Code reports cacheReadTokens as a subset of
        // inputTokens (the cache-hit portion of the prompt). The total must
        // not add cache_read on top of input.
        let transcript = transcript_from(VALID_TRANSCRIPT); // input 10, cache_read 3
        let context = context();

        let candidates =
            map_daily(vec![transcript], "UTC", &CollectionScope::Full, &context).expect("daily");

        assert_eq!(candidates[0].tokens.input_tokens(), Some(7));
        assert_eq!(candidates[0].tokens.cache_read_tokens(), Some(3));
        assert_eq!(candidates[0].tokens.total_tokens(), 12);
    }

    #[test]
    fn maps_session_candidate_from_transcript() {
        let transcripts = vec![transcript_from(VALID_TRANSCRIPT)];
        let context = context();

        let candidates = map_sessions(transcripts, &context).expect("sessions");

        assert_eq!(candidates.len(), 1);
        let candidate = &candidates[0];
        assert!(candidate
            .source_key
            .starts_with("command-code:session:v1:sess-1:"));
        assert_eq!(candidate.source_session_id, "sess-1");
        assert_eq!(candidate.project_path.as_deref(), Some("/tmp/proj"));
        assert!(candidate.first_activity_at.is_some());
        assert!(candidate.last_activity_at.is_some());
        assert_eq!(candidate.tokens.total_tokens(), 12);
    }

    #[test]
    fn dedupes_duplicate_message_ids_within_session() {
        let transcript = transcript_from(&format!(
            "{VALID_TRANSCRIPT}\n{}",
            VALID_TRANSCRIPT.lines().last().expect("last line")
        ));
        let context = context();

        let candidates = map_sessions(vec![transcript], &context).expect("sessions");

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].tokens.total_tokens(), 12);
    }

    #[test]
    fn aggregates_multiple_usage_records_per_day() {
        let multi = r#"{"type":"session","version":3,"id":"sess-2","timestamp":"2026-08-04T10:00:00Z","cwd":"/tmp/proj"}
{"type":"message","id":"m1","parentId":null,"timestamp":"2026-08-04T10:00:01Z","message":{"role":"assistant","content":[]},"usage":{"inputTokens":10,"outputTokens":2,"cacheReadTokens":0,"cacheWriteTokens":0,"costUsd":0.001},"model":"m","effort":"max"}
{"type":"message","id":"m2","parentId":"m1","timestamp":"2026-08-04T11:00:00Z","message":{"role":"assistant","content":[]},"usage":{"inputTokens":20,"outputTokens":4,"cacheReadTokens":0,"cacheWriteTokens":0,"costUsd":0.002},"model":"m","effort":"max"}"#;
        let context = context();

        let candidates = map_daily(
            vec![transcript_from(multi)],
            "UTC",
            &CollectionScope::Full,
            &context,
        )
        .expect("daily");

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].tokens.total_tokens(), 36);
        let cost = match &candidates[0].cost {
            UsageCost::Valued { amount_micros, .. } => *amount_micros,
            _ => panic!("expected valued cost"),
        };
        assert_eq!(cost, 3000);
    }

    #[test]
    fn zero_cost_with_usage_is_unavailable() {
        let no_cost = r#"{"type":"session","version":3,"id":"sess-3","timestamp":"2026-08-04T10:00:00Z","cwd":"/tmp/proj"}
{"type":"message","id":"m1","parentId":null,"timestamp":"2026-08-04T10:00:01Z","message":{"role":"assistant","content":[]},"usage":{"inputTokens":10,"outputTokens":2,"cacheReadTokens":0,"cacheWriteTokens":0,"costUsd":0},"model":"m","effort":"max"}"#;
        let context = context();

        let candidates = map_sessions(vec![transcript_from(no_cost)], &context).expect("sessions");

        assert!(matches!(
            candidates[0].cost,
            UsageCost::Unavailable {
                kind: CostKind::SourceReported
            }
        ));
    }

    #[test]
    fn respects_incremental_scope() {
        let context = context();
        let scope = CollectionScope::incremental(
            NaiveDate::from_ymd_opt(2026, 8, 3).expect("date"),
            NaiveDate::from_ymd_opt(2026, 8, 3).expect("date"),
        )
        .expect("scope");

        let candidates = map_daily(
            vec![transcript_from(VALID_TRANSCRIPT)],
            "UTC",
            &scope,
            &context,
        )
        .expect("daily");

        assert!(candidates.is_empty());
    }

    #[test]
    fn rejects_invalid_timezone() {
        let context = context();

        let error = map_daily(
            vec![transcript_from(VALID_TRANSCRIPT)],
            "not-a-timezone",
            &CollectionScope::Full,
            &context,
        )
        .expect_err("invalid timezone");

        assert_eq!(error, CommandCodeMappingError::InvalidTimezone);
    }
}
