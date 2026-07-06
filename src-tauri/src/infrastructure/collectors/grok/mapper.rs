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
        usage::{CostKind, TokenUsage, UsageCost, UsageValidationError},
    },
};

use super::super::support::{
    checked_add_u64, date_in_scope, local_date_from_millis, provenance, MappingIdentity,
};
use super::session_index::GrokSessionSummary;
use super::unified_log_reader::GrokInferenceUsage;
use super::GrokModelResolver;

const PROFILE_VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GrokMappingContext {
    collector: CollectorKey,
    collector_version: String,
    collection_id: CollectionId,
    observed_at: DateTime<Utc>,
}

impl GrokMappingContext {
    pub(crate) fn new(
        collector: CollectorKey,
        collector_version: String,
        collection_id: CollectionId,
        observed_at: DateTime<Utc>,
    ) -> Result<Self, GrokMappingError> {
        if collector_version.trim().is_empty() {
            return Err(GrokMappingError::EmptyCollectorVersion);
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
            source: SourceKey::GrokBuild,
            collector: self.collector.clone(),
            collector_version: self.collector_version.clone(),
            profile_version: PROFILE_VERSION,
            collection_id: self.collection_id.clone(),
            observed_at: self.observed_at,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GrokMappedInference {
    pub(crate) inference: GrokInferenceUsage,
    pub(crate) model_id: String,
    pub(crate) project_path: Option<String>,
}

pub(crate) fn dedupe_inferences(rows: Vec<GrokInferenceUsage>) -> Vec<GrokInferenceUsage> {
    let mut seen = BTreeSet::new();
    let mut deduped = Vec::new();
    for row in rows {
        if seen.insert(dedupe_key(&row)) {
            deduped.push(row);
        }
    }
    deduped
}

pub(crate) fn map_inferences(
    rows: Vec<GrokInferenceUsage>,
    resolver: &GrokModelResolver,
    summaries: &[GrokSessionSummary],
) -> Vec<GrokMappedInference> {
    let sessions = summaries
        .iter()
        .map(|summary| (summary.session_id.clone(), summary.cwd.clone()))
        .collect::<BTreeMap<_, _>>();

    dedupe_inferences(rows)
        .into_iter()
        .map(|inference| GrokMappedInference {
            model_id: resolver.resolve(&inference),
            project_path: sessions.get(&inference.session_id).cloned(),
            inference,
        })
        .collect()
}

pub(crate) fn map_daily(
    rows: Vec<GrokMappedInference>,
    timezone: &str,
    scope: &CollectionScope,
    context: &GrokMappingContext,
) -> Result<Vec<DailyUsageCandidate>, GrokMappingError> {
    let timezone = timezone
        .parse::<Tz>()
        .map_err(|_| GrokMappingError::InvalidTimezone)?;
    let mut buckets = BTreeMap::<NaiveDate, GrokDailyBucket>::new();

    for row in rows {
        let usage_date = local_date_from_millis(
            row.inference.observed_at.timestamp_millis(),
            timezone,
            GrokMappingError::InvalidTimestamp,
        )?;
        if !date_in_scope(usage_date, scope) {
            continue;
        }
        buckets.entry(usage_date).or_default().add(&row)?;
    }

    buckets
        .into_iter()
        .map(|(usage_date, usage)| {
            let tokens = usage.total.tokens()?;
            let aggregate_cost = cost(tokens.total_tokens());
            let model_breakdowns = usage
                .models
                .into_iter()
                .map(|(model, usage)| {
                    let tokens = usage.tokens()?;
                    let cost = cost(tokens.total_tokens());
                    Ok(ModelUsageCandidate {
                        raw_model_id: model,
                        tokens,
                        cost,
                    })
                })
                .collect::<Result<Vec<_>, GrokMappingError>>()?;
            Ok(DailyUsageCandidate {
                provenance: context.provenance(),
                source_key: daily_source_key(SourceKey::GrokBuild, usage_date, timezone.name())?,
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
    rows: Vec<GrokMappedInference>,
    context: &GrokMappingContext,
) -> Result<Vec<SessionUsageCandidate>, GrokMappingError> {
    let mut buckets = BTreeMap::<(String, String), GrokSessionAccumulator>::new();

    for row in rows {
        buckets
            .entry((row.inference.session_id.clone(), row.model_id.clone()))
            .or_insert_with(|| {
                GrokSessionAccumulator::new(
                    row.inference.session_id.clone(),
                    row.model_id.clone(),
                    row.project_path.clone(),
                )
            })
            .add(&row)?;
    }

    buckets
        .into_values()
        .map(|usage| usage.candidate(context))
        .collect()
}

fn dedupe_key(row: &GrokInferenceUsage) -> (String, DateTime<Utc>, u32, u64, u64, u64) {
    (
        row.session_id.clone(),
        row.observed_at,
        row.loop_index,
        row.prompt_tokens,
        row.completion_tokens,
        row.pid,
    )
}

#[derive(Debug, Default)]
struct GrokDailyBucket {
    total: GrokUsageAccumulator,
    models: BTreeMap<String, GrokUsageAccumulator>,
}

impl GrokDailyBucket {
    fn add(&mut self, row: &GrokMappedInference) -> Result<(), GrokMappingError> {
        self.total.add(&row.inference)?;
        self.models
            .entry(row.model_id.clone())
            .or_default()
            .add(&row.inference)
    }
}

#[derive(Debug, Default)]
struct GrokUsageAccumulator {
    input_tokens: u64,
    output_tokens: u64,
    cache_read_tokens: u64,
    total_tokens: u64,
}

impl GrokUsageAccumulator {
    fn add(&mut self, row: &GrokInferenceUsage) -> Result<(), GrokMappingError> {
        let mapped = map_tokens(row)?;
        self.input_tokens = checked_add(self.input_tokens, mapped.input_tokens().unwrap_or(0))?;
        self.output_tokens = checked_add(self.output_tokens, mapped.output_tokens().unwrap_or(0))?;
        self.cache_read_tokens = checked_add(
            self.cache_read_tokens,
            mapped.cache_read_tokens().unwrap_or(0),
        )?;
        self.total_tokens = checked_add(self.total_tokens, mapped.total_tokens())?;
        Ok(())
    }

    fn tokens(&self) -> Result<TokenUsage, GrokMappingError> {
        TokenUsage::new(
            Some(self.input_tokens),
            Some(self.output_tokens),
            Some(0),
            Some(self.cache_read_tokens),
            self.total_tokens,
        )
        .map_err(Into::into)
    }
}

struct GrokSessionAccumulator {
    session_id: String,
    model_id: String,
    project_path: Option<String>,
    usage: GrokUsageAccumulator,
    first_activity_at: DateTime<Utc>,
    last_activity_at: DateTime<Utc>,
}

impl GrokSessionAccumulator {
    fn new(session_id: String, model_id: String, project_path: Option<String>) -> Self {
        Self {
            session_id,
            model_id,
            project_path,
            usage: GrokUsageAccumulator::default(),
            first_activity_at: DateTime::<Utc>::MIN_UTC,
            last_activity_at: DateTime::<Utc>::MIN_UTC,
        }
    }

    fn add(&mut self, row: &GrokMappedInference) -> Result<(), GrokMappingError> {
        if self.first_activity_at == DateTime::<Utc>::MIN_UTC {
            self.first_activity_at = row.inference.observed_at;
            self.last_activity_at = row.inference.observed_at;
        } else {
            self.first_activity_at = self.first_activity_at.min(row.inference.observed_at);
            self.last_activity_at = self.last_activity_at.max(row.inference.observed_at);
        }
        if self.project_path.is_none() {
            self.project_path = row.project_path.clone();
        }
        self.usage.add(&row.inference)
    }

    fn candidate(
        self,
        context: &GrokMappingContext,
    ) -> Result<SessionUsageCandidate, GrokMappingError> {
        let tokens = self.usage.tokens()?;
        let cost = cost(tokens.total_tokens());
        Ok(SessionUsageCandidate {
            provenance: context.provenance(),
            source_key: session_source_key(
                SourceKey::GrokBuild,
                &format!("{}:{}", self.session_id, self.model_id),
            )?,
            source_session_id: self.session_id,
            project_path: self.project_path,
            first_activity_at: Some(self.first_activity_at),
            last_activity_at: Some(self.last_activity_at),
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

fn map_tokens(row: &GrokInferenceUsage) -> Result<TokenUsage, GrokMappingError> {
    let input_tokens = row
        .prompt_tokens
        .checked_sub(row.cached_prompt_tokens)
        .ok_or(GrokMappingError::OverlappingCacheTokens)?;
    let output_tokens = checked_add(row.completion_tokens, row.reasoning_tokens)?;
    let total_tokens = checked_add(
        checked_add(row.prompt_tokens, row.completion_tokens)?,
        row.reasoning_tokens,
    )?;
    TokenUsage::new(
        Some(input_tokens),
        Some(output_tokens),
        Some(0),
        Some(row.cached_prompt_tokens),
        total_tokens,
    )
    .map_err(Into::into)
}

fn checked_add(left: u64, right: u64) -> Result<u64, GrokMappingError> {
    checked_add_u64(left, right, GrokMappingError::TokenOverflow)
}

fn cost(total_tokens: u64) -> UsageCost {
    if total_tokens == 0 {
        UsageCost::NotApplicable {
            kind: CostKind::SourceReported,
        }
    } else {
        UsageCost::Unavailable {
            kind: CostKind::SourceReported,
        }
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub(crate) enum GrokMappingError {
    #[error("grok mapping requires a collector version")]
    EmptyCollectorVersion,
    #[error("grok mapping requires a valid timezone")]
    InvalidTimezone,
    #[error("grok mapping received an invalid timestamp")]
    InvalidTimestamp,
    #[error("grok token total overflowed")]
    TokenOverflow,
    #[error("grok cache tokens exceed prompt tokens")]
    OverlappingCacheTokens,
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error(transparent)]
    Usage(#[from] UsageValidationError),
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use chrono::TimeZone;

    use super::*;
    use crate::application::collection::{CollectionId, CollectionScope, CollectorKey};

    #[test]
    fn dedupes_duplicate_inference_keys() {
        let rows = vec![
            inference_row(1, 12000, 8000, 240),
            inference_row(1, 12000, 8000, 240),
        ];

        let deduped = dedupe_inferences(rows);

        assert_eq!(deduped.len(), 1);
    }

    #[test]
    fn maps_inference_tokens_without_double_counting_cached_prompt_tokens() {
        let tokens = map_tokens(&inference_row(1, 12000, 8000, 240)).expect("tokens");

        assert_eq!(tokens.input_tokens(), Some(4000));
        assert_eq!(tokens.cache_read_tokens(), Some(8000));
        assert_eq!(tokens.output_tokens(), Some(240));
        assert_eq!(tokens.total_tokens(), 12240);
    }

    #[test]
    fn maps_daily_usage_from_fixture_rows() {
        let rows = mapped_rows_from_fixture("unified-log/single-session.jsonl");
        let context = context();

        let candidates =
            map_daily(rows, "Asia/Jakarta", &CollectionScope::Full, &context).expect("daily");

        assert_eq!(candidates.len(), 1);
        let candidate = &candidates[0];
        assert_eq!(
            candidate.source_key,
            "grok-build:daily:v1:Asia/Jakarta:2026-07-06"
        );
        assert_eq!(candidate.tokens.input_tokens(), Some(7000));
        assert_eq!(candidate.tokens.cache_read_tokens(), Some(20000));
        assert_eq!(candidate.tokens.output_tokens(), Some(420));
        assert_eq!(candidate.tokens.total_tokens(), 27420);
        assert_eq!(candidate.model_breakdowns.len(), 1);
        assert_eq!(
            candidate.model_breakdowns[0].raw_model_id,
            "grok-composer-2.5-fast"
        );
    }

    #[test]
    fn maps_session_usage_grouped_by_session_and_model() {
        let rows = mapped_rows_from_fixture("unified-log/multi-session.jsonl");
        let context = context();

        let candidates = map_sessions(rows, &context).expect("sessions");

        assert_eq!(candidates.len(), 2);
        assert!(candidates.iter().all(|candidate| {
            candidate
                .source_key
                .starts_with("grok-build:session:v1:019f0000-0000-7000-8000-00000000000")
        }));
        let first = candidates
            .iter()
            .find(|candidate| candidate.source_session_id.ends_with("0001"))
            .expect("first session");
        assert_eq!(
            first.project_path.as_deref(),
            Some("/tmp/grok-fixture-project")
        );
        assert_eq!(first.tokens.total_tokens(), 12240);
    }

    #[test]
    fn rejects_cache_tokens_that_exceed_prompt_tokens() {
        let mut row = inference_row(1, 1000, 2000, 10);

        let error = map_tokens(&row).expect_err("invalid overlap");
        assert_eq!(error, GrokMappingError::OverlappingCacheTokens);

        row.cached_prompt_tokens = 2000;
        let error = map_tokens(&row).expect_err("invalid overlap");
        assert_eq!(error, GrokMappingError::OverlappingCacheTokens);
    }

    fn mapped_rows_from_fixture(relative: &str) -> Vec<GrokMappedInference> {
        let fixture = MapperFixture::new(relative);
        let (rows, _) = super::super::unified_log_reader::UnifiedLogReader::read_from_path(
            &fixture.unified_log_path(),
        )
        .expect("read log");
        let summaries =
            super::super::session_index::GrokSessionIndex::from_grok_home(fixture.grok_home())
                .scan()
                .expect("scan summaries");
        let resolver =
            GrokModelResolver::from_grok_home(fixture.grok_home(), &summaries).expect("resolver");
        map_inferences(rows, &resolver, &summaries)
    }

    struct MapperFixture {
        _workspace: tempfile::TempDir,
        grok_home: std::path::PathBuf,
    }

    impl MapperFixture {
        fn new(log_relative: &str) -> Self {
            let workspace = tempfile::TempDir::new().expect("temp dir");
            let grok_home = workspace.path().to_path_buf();
            std::fs::create_dir_all(grok_home.join("logs")).expect("logs dir");
            std::fs::copy(
                fixture_path(log_relative),
                grok_home.join("logs/unified.jsonl"),
            )
            .expect("copy log");

            let session_dir = grok_home
                .join("sessions")
                .join("%2Ftmp%2Fgrok-fixture-project")
                .join("019f0000-0000-7000-8000-000000000001");
            std::fs::create_dir_all(&session_dir).expect("session dir");
            std::fs::copy(
                fixture_path("sessions/summary-valid.json"),
                session_dir.join("summary.json"),
            )
            .expect("copy summary");
            std::fs::copy(
                fixture_path("events/turn-started.jsonl"),
                session_dir.join("events.jsonl"),
            )
            .expect("copy events");

            Self {
                _workspace: workspace,
                grok_home,
            }
        }

        fn grok_home(&self) -> &std::path::Path {
            &self.grok_home
        }

        fn unified_log_path(&self) -> std::path::PathBuf {
            self.grok_home.join("logs/unified.jsonl")
        }
    }

    fn fixture_path(relative: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/collectors/grok")
            .join(relative)
    }

    fn context() -> GrokMappingContext {
        GrokMappingContext::new(
            CollectorKey::new("grok-build").expect("collector"),
            "local".to_owned(),
            CollectionId::new("grok-test").expect("collection"),
            Utc.with_ymd_and_hms(2026, 7, 6, 1, 0, 0)
                .single()
                .expect("timestamp"),
        )
        .expect("context")
    }

    fn inference_row(
        loop_index: u32,
        prompt_tokens: u64,
        cached_prompt_tokens: u64,
        completion_tokens: u64,
    ) -> GrokInferenceUsage {
        GrokInferenceUsage {
            session_id: "019f0000-0000-7000-8000-000000000001".to_owned(),
            observed_at: Utc
                .with_ymd_and_hms(2026, 7, 6, 10, 0, 0)
                .single()
                .expect("timestamp"),
            pid: 1001,
            loop_index,
            prompt_tokens,
            cached_prompt_tokens,
            completion_tokens,
            reasoning_tokens: 0,
        }
    }
}
