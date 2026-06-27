use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use chrono::{DateTime, Datelike, Days, NaiveDate, Utc};
use chrono_tz::Tz;
use thiserror::Error;

use crate::application::ports::clock::Clock;
use crate::application::ports::tray_summary_store::{TraySummaryStore, TraySummaryStoreError};
use crate::domain::source::SourceKey;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PersistedRefreshStatus {
    Cancelled,
    Succeeded,
    Partial,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OverviewDataStatus {
    Current,
    Stale,
    Partial,
    Empty,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TraySummaryScope {
    today: NaiveDate,
    yesterday: NaiveDate,
    week_start: NaiveDate,
    week_end: NaiveDate,
    month_start: NaiveDate,
    month_end: NaiveDate,
    aggregation_timezone: String,
}

impl TraySummaryScope {
    #[cfg(test)]
    pub(crate) fn new(
        today: NaiveDate,
        aggregation_timezone: impl Into<String>,
    ) -> Result<Self, TraySummaryQueryError> {
        let aggregation_timezone = aggregation_timezone.into();
        if aggregation_timezone.trim().is_empty() {
            return Err(TraySummaryQueryError::EmptyAggregationTimezone);
        }
        Tz::from_str(&aggregation_timezone)
            .map_err(|_| TraySummaryQueryError::InvalidAggregationTimezone)?;
        scope_for_date(today, aggregation_timezone)
    }

    pub(crate) const fn today(&self) -> NaiveDate {
        self.today
    }

    pub(crate) const fn yesterday(&self) -> NaiveDate {
        self.yesterday
    }

    pub(crate) const fn week_start(&self) -> NaiveDate {
        self.week_start
    }

    pub(crate) const fn week_end(&self) -> NaiveDate {
        self.week_end
    }

    pub(crate) const fn month_start(&self) -> NaiveDate {
        self.month_start
    }

    pub(crate) const fn month_end(&self) -> NaiveDate {
        self.month_end
    }

    pub(crate) fn aggregation_timezone(&self) -> &str {
        &self.aggregation_timezone
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TraySummaryPeriodMetric {
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub total_tokens: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TraySummaryTrendDirection {
    Increased,
    Decreased,
    Flat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TraySummaryTrend {
    pub direction: TraySummaryTrendDirection,
    pub basis_points: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TraySummaryModelRow {
    pub model_name: String,
    pub agent_label: String,
    pub total_tokens: u64,
    pub trend: Option<TraySummaryTrend>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TraySummaryReadModel {
    pub today: TraySummaryPeriodMetric,
    pub week: TraySummaryPeriodMetric,
    pub month: TraySummaryPeriodMetric,
    pub models: Vec<TraySummaryModelRow>,
    pub as_of_ms: i64,
    pub last_successful_refresh_at_ms: Option<i64>,
    pub data_status: OverviewDataStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TraySummaryStoreResult {
    pub today_total_tokens: u64,
    pub week_total_tokens: u64,
    pub month_total_tokens: u64,
    pub today_models: Vec<TraySummaryStoreModelUsage>,
    pub yesterday_models: Vec<TraySummaryStoreModelUsage>,
    pub has_partial_data: bool,
    pub latest_refresh_status: Option<PersistedRefreshStatus>,
    pub last_successful_refresh_at_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TraySummaryStoreModelUsage {
    pub model_name: String,
    pub source_keys: Vec<SourceKey>,
    pub total_tokens: u64,
}

#[derive(Clone)]
pub(crate) struct TraySummaryQuery {
    store: Arc<dyn TraySummaryStore>,
    clock: Arc<dyn Clock>,
}

impl TraySummaryQuery {
    pub(crate) fn new(store: Arc<dyn TraySummaryStore>, clock: Arc<dyn Clock>) -> Self {
        Self { store, clock }
    }

    pub(crate) fn get(
        &self,
        reporting_timezone: impl Into<String>,
    ) -> Result<TraySummaryReadModel, TraySummaryQueryError> {
        let reporting_timezone = reporting_timezone.into();
        let scope = scope_for_instant(self.clock.now_epoch_ms(), reporting_timezone)?;
        let result = self.store.read_tray_summary(&scope)?;
        Ok(read_model(scope, result, self.clock.now_epoch_ms()))
    }
}

fn scope_for_instant(
    now_epoch_ms: i64,
    aggregation_timezone: String,
) -> Result<TraySummaryScope, TraySummaryQueryError> {
    if aggregation_timezone.trim().is_empty() {
        return Err(TraySummaryQueryError::EmptyAggregationTimezone);
    }
    let timezone = Tz::from_str(&aggregation_timezone)
        .map_err(|_| TraySummaryQueryError::InvalidAggregationTimezone)?;
    let instant = DateTime::<Utc>::from_timestamp_millis(now_epoch_ms)
        .ok_or(TraySummaryQueryError::InvalidTimestamp)?;
    scope_for_date(
        instant.with_timezone(&timezone).date_naive(),
        aggregation_timezone,
    )
}

fn scope_for_date(
    today: NaiveDate,
    aggregation_timezone: String,
) -> Result<TraySummaryScope, TraySummaryQueryError> {
    let yesterday = today
        .checked_sub_days(Days::new(1))
        .ok_or(TraySummaryQueryError::InvalidTimestamp)?;
    let week_start = today
        .checked_sub_days(Days::new(u64::from(today.weekday().num_days_from_monday())))
        .ok_or(TraySummaryQueryError::InvalidTimestamp)?;
    let week_end = week_start
        .checked_add_days(Days::new(6))
        .ok_or(TraySummaryQueryError::InvalidTimestamp)?;
    let month_start = NaiveDate::from_ymd_opt(today.year(), today.month(), 1)
        .ok_or(TraySummaryQueryError::InvalidTimestamp)?;
    let next_month = if today.month() == 12 {
        NaiveDate::from_ymd_opt(today.year() + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(today.year(), today.month() + 1, 1)
    }
    .ok_or(TraySummaryQueryError::InvalidTimestamp)?;
    let month_end = next_month
        .checked_sub_days(Days::new(1))
        .ok_or(TraySummaryQueryError::InvalidTimestamp)?;

    Ok(TraySummaryScope {
        today,
        yesterday,
        week_start,
        week_end,
        month_start,
        month_end,
        aggregation_timezone,
    })
}

fn read_model(
    scope: TraySummaryScope,
    result: TraySummaryStoreResult,
    as_of_ms: i64,
) -> TraySummaryReadModel {
    let yesterday_models = result
        .yesterday_models
        .into_iter()
        .map(|model| (model.model_name, model.total_tokens))
        .collect::<HashMap<_, _>>();
    let has_partial_data = result.has_partial_data;
    let latest_refresh_status = result.latest_refresh_status;
    let models = top_model_rows(result.today_models, &yesterday_models);
    let data_status = data_status(
        result.today_total_tokens,
        has_partial_data,
        latest_refresh_status,
        &models,
    );

    TraySummaryReadModel {
        today: TraySummaryPeriodMetric {
            start_date: scope.today,
            end_date: scope.today,
            total_tokens: result.today_total_tokens,
        },
        week: TraySummaryPeriodMetric {
            start_date: scope.week_start,
            end_date: scope.week_end,
            total_tokens: result.week_total_tokens,
        },
        month: TraySummaryPeriodMetric {
            start_date: scope.month_start,
            end_date: scope.month_end,
            total_tokens: result.month_total_tokens,
        },
        models,
        as_of_ms,
        last_successful_refresh_at_ms: result.last_successful_refresh_at_ms,
        data_status,
    }
}

fn top_model_rows(
    mut today_models: Vec<TraySummaryStoreModelUsage>,
    yesterday_models: &HashMap<String, u64>,
) -> Vec<TraySummaryModelRow> {
    today_models.sort_by(|left, right| {
        right
            .total_tokens
            .cmp(&left.total_tokens)
            .then_with(|| left.model_name.cmp(&right.model_name))
    });

    let mut rows = today_models
        .iter()
        .take(3)
        .map(|model| model_row(model, yesterday_models))
        .collect::<Vec<_>>();
    if today_models.len() > 3 {
        let other_total = today_models[3..].iter().fold(0_u64, |total, model| {
            total.saturating_add(model.total_tokens)
        });
        let other_yesterday = today_models[3..]
            .iter()
            .filter_map(|model| yesterday_models.get(&model.model_name).copied())
            .fold(0_u64, |total, tokens| total.saturating_add(tokens));
        rows.push(TraySummaryModelRow {
            model_name: "Other".to_owned(),
            agent_label: "Multiple agents".to_owned(),
            total_tokens: other_total,
            trend: trend(other_total, other_yesterday),
        });
    }
    rows
}

fn model_row(
    model: &TraySummaryStoreModelUsage,
    yesterday_models: &HashMap<String, u64>,
) -> TraySummaryModelRow {
    TraySummaryModelRow {
        model_name: model.model_name.clone(),
        agent_label: agent_label(&model.source_keys),
        total_tokens: model.total_tokens,
        trend: yesterday_models
            .get(&model.model_name)
            .copied()
            .and_then(|yesterday| trend(model.total_tokens, yesterday)),
    }
}

fn agent_label(source_keys: &[SourceKey]) -> String {
    match source_keys {
        [source] => source_label(*source).to_owned(),
        [] => "Unknown agent".to_owned(),
        _ => "Multiple agents".to_owned(),
    }
}

fn source_label(source: SourceKey) -> &'static str {
    match source {
        SourceKey::ClaudeCode => "Claude Code",
        SourceKey::Codex => "Codex",
        SourceKey::OpenCode => "OpenCode",
        #[cfg(test)]
        SourceKey::TestUnsupported => "Unsupported",
    }
}

fn trend(today: u64, yesterday: u64) -> Option<TraySummaryTrend> {
    if yesterday == 0 {
        return (today == 0).then_some(TraySummaryTrend {
            direction: TraySummaryTrendDirection::Flat,
            basis_points: 0,
        });
    }
    let direction = match today.cmp(&yesterday) {
        std::cmp::Ordering::Greater => TraySummaryTrendDirection::Increased,
        std::cmp::Ordering::Less => TraySummaryTrendDirection::Decreased,
        std::cmp::Ordering::Equal => TraySummaryTrendDirection::Flat,
    };
    let difference = today.abs_diff(yesterday);
    let basis_points = difference
        .saturating_mul(10_000)
        .checked_div(yesterday)
        .unwrap_or(0)
        .min(u64::from(u32::MAX)) as u32;
    Some(TraySummaryTrend {
        direction,
        basis_points,
    })
}

fn data_status(
    today_total_tokens: u64,
    has_partial_data: bool,
    latest_refresh_status: Option<PersistedRefreshStatus>,
    models: &[TraySummaryModelRow],
) -> OverviewDataStatus {
    if today_total_tokens == 0 && models.is_empty() {
        return OverviewDataStatus::Empty;
    }
    if has_partial_data || matches!(latest_refresh_status, Some(PersistedRefreshStatus::Partial)) {
        return OverviewDataStatus::Partial;
    }
    if matches!(
        latest_refresh_status,
        Some(PersistedRefreshStatus::Failed | PersistedRefreshStatus::Cancelled)
    ) {
        return OverviewDataStatus::Stale;
    }
    OverviewDataStatus::Current
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub(crate) enum TraySummaryQueryError {
    #[error("tray summary aggregation timezone must not be empty")]
    EmptyAggregationTimezone,
    #[error("tray summary aggregation timezone must be a valid IANA timezone")]
    InvalidAggregationTimezone,
    #[error("tray summary timestamp is invalid")]
    InvalidTimestamp,
    #[error("tray summary storage failed")]
    Storage(#[from] TraySummaryStoreError),
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    struct FixedClock(i64);

    impl Clock for FixedClock {
        fn now_epoch_ms(&self) -> i64 {
            self.0
        }
    }

    struct FakeStore(Mutex<TraySummaryStoreResult>);

    impl TraySummaryStore for FakeStore {
        fn read_tray_summary(
            &self,
            _scope: &TraySummaryScope,
        ) -> Result<TraySummaryStoreResult, TraySummaryStoreError> {
            Ok(self.0.lock().expect("store lock").clone())
        }
    }

    #[test]
    fn builds_reporting_timezone_windows_from_clock() {
        let scope = scope_for_instant(1_782_375_600_000, "Asia/Jakarta".to_owned()).expect("scope");

        assert_eq!(scope.today(), date(2026, 6, 25));
        assert_eq!(scope.yesterday(), date(2026, 6, 24));
        assert_eq!(scope.week_start(), date(2026, 6, 22));
        assert_eq!(scope.week_end(), date(2026, 6, 28));
        assert_eq!(scope.month_start(), date(2026, 6, 1));
        assert_eq!(scope.month_end(), date(2026, 6, 30));
        assert_eq!(scope.aggregation_timezone(), "Asia/Jakarta");
    }

    #[test]
    fn maps_top_models_other_agent_labels_and_trends() {
        let query = TraySummaryQuery::new(
            Arc::new(FakeStore(Mutex::new(TraySummaryStoreResult {
                today_total_tokens: 1_000,
                week_total_tokens: 2_000,
                month_total_tokens: 3_000,
                today_models: vec![
                    usage("gpt-5.1", &[SourceKey::Codex], 500),
                    usage("claude-sonnet", &[SourceKey::ClaudeCode], 300),
                    usage("mimo", &[SourceKey::OpenCode], 100),
                    usage("shared", &[SourceKey::Codex, SourceKey::OpenCode], 80),
                    usage("small", &[SourceKey::Codex], 20),
                ],
                yesterday_models: vec![
                    usage("gpt-5.1", &[SourceKey::Codex], 250),
                    usage("claude-sonnet", &[SourceKey::ClaudeCode], 600),
                    usage("mimo", &[SourceKey::OpenCode], 100),
                    usage("shared", &[SourceKey::Codex, SourceKey::OpenCode], 40),
                    usage("small", &[SourceKey::Codex], 10),
                ],
                has_partial_data: false,
                latest_refresh_status: Some(PersistedRefreshStatus::Succeeded),
                last_successful_refresh_at_ms: Some(1_000),
            }))),
            Arc::new(FixedClock(1_782_375_600_000)),
        );

        let model = query.get("Asia/Jakarta").expect("summary");

        assert_eq!(model.today.total_tokens, 1_000);
        assert_eq!(model.week.total_tokens, 2_000);
        assert_eq!(model.month.total_tokens, 3_000);
        assert_eq!(model.models.len(), 4);
        assert_eq!(model.models[0].model_name, "gpt-5.1");
        assert_eq!(model.models[0].agent_label, "Codex");
        assert_eq!(
            model.models[0].trend,
            Some(TraySummaryTrend {
                direction: TraySummaryTrendDirection::Increased,
                basis_points: 10_000,
            })
        );
        assert_eq!(model.models[1].agent_label, "Claude Code");
        assert_eq!(
            model.models[1].trend.map(|trend| trend.direction),
            Some(TraySummaryTrendDirection::Decreased)
        );
        assert_eq!(model.models[2].agent_label, "OpenCode");
        assert_eq!(
            model.models[2].trend.map(|trend| trend.direction),
            Some(TraySummaryTrendDirection::Flat)
        );
        assert_eq!(model.models[3].model_name, "Other");
        assert_eq!(model.models[3].agent_label, "Multiple agents");
        assert_eq!(model.models[3].total_tokens, 100);
    }

    #[test]
    fn missing_yesterday_model_has_no_trend() {
        assert_eq!(trend(100, 0), None);
    }

    #[test]
    fn validates_timezone() {
        assert_eq!(
            scope_for_instant(1_782_375_600_000, " ".to_owned()),
            Err(TraySummaryQueryError::EmptyAggregationTimezone)
        );
        assert_eq!(
            scope_for_instant(1_782_375_600_000, "Mars/Base".to_owned()),
            Err(TraySummaryQueryError::InvalidAggregationTimezone)
        );
    }

    fn usage(
        model_name: &str,
        source_keys: &[SourceKey],
        total_tokens: u64,
    ) -> TraySummaryStoreModelUsage {
        TraySummaryStoreModelUsage {
            model_name: model_name.to_owned(),
            source_keys: source_keys.to_vec(),
            total_tokens,
        }
    }

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
    }
}
