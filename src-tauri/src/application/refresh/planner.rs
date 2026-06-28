//! Refresh policy planner.
//!
//! This module decides the collection scope for a refresh target from explicit
//! policy inputs. It does not execute collectors or persist runs.

#![allow(
    dead_code,
    reason = "planner is introduced before coordinator wiring in the refresh-policy implementation series"
)]

use chrono::{Days, NaiveDate};

use crate::application::collection::{CollectionProjection, CollectionScope};
use crate::application::reconciliation::SuccessfulImportState;
use crate::domain::source::SourceKey;

const CATCH_UP_LOOKBACK_DAYS: u64 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RefreshPlanMode {
    CatchUp,
    Freshness,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RefreshPlanTarget {
    source: SourceKey,
    projection: CollectionProjection,
    aggregation_timezone: Option<String>,
}

impl RefreshPlanTarget {
    pub(crate) fn daily(source: SourceKey, aggregation_timezone: impl Into<String>) -> Self {
        Self {
            source,
            projection: CollectionProjection::Daily,
            aggregation_timezone: Some(aggregation_timezone.into()),
        }
    }

    pub(crate) const fn session(source: SourceKey) -> Self {
        Self {
            source,
            projection: CollectionProjection::Session,
            aggregation_timezone: None,
        }
    }

    pub(crate) const fn source(&self) -> SourceKey {
        self.source
    }

    pub(crate) const fn projection(&self) -> CollectionProjection {
        self.projection
    }

    pub(crate) fn aggregation_timezone(&self) -> Option<&str> {
        self.aggregation_timezone.as_deref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RefreshPlanRequest {
    target: RefreshPlanTarget,
    mode: RefreshPlanMode,
    today: NaiveDate,
    previous_import: Option<SuccessfulImportState>,
}

impl RefreshPlanRequest {
    pub(crate) const fn new(
        target: RefreshPlanTarget,
        mode: RefreshPlanMode,
        today: NaiveDate,
        previous_import: Option<SuccessfulImportState>,
    ) -> Self {
        Self {
            target,
            mode,
            today,
            previous_import,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RefreshPlan {
    target: RefreshPlanTarget,
    scope: CollectionScope,
}

impl RefreshPlan {
    pub(crate) const fn target(&self) -> &RefreshPlanTarget {
        &self.target
    }

    pub(crate) const fn scope(&self) -> &CollectionScope {
        &self.scope
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct RefreshPolicyPlanner;

impl RefreshPolicyPlanner {
    pub(crate) const fn new() -> Self {
        Self
    }

    pub(crate) fn plan(&self, request: RefreshPlanRequest) -> RefreshPlan {
        let scope = match request.previous_import.as_ref() {
            None => CollectionScope::Full,
            Some(previous_import) if import_matches_target(&request.target, previous_import) => {
                match request.mode {
                    RefreshPlanMode::CatchUp => catch_up_scope(request.today, previous_import),
                    RefreshPlanMode::Freshness => today_scope(request.today),
                }
            }
            Some(_) => CollectionScope::Full,
        };

        RefreshPlan {
            target: request.target,
            scope,
        }
    }
}

fn import_matches_target(
    target: &RefreshPlanTarget,
    previous_import: &SuccessfulImportState,
) -> bool {
    previous_import.source() == target.source()
        && previous_import.projection() == target.projection()
}

fn catch_up_scope(today: NaiveDate, previous_import: &SuccessfulImportState) -> CollectionScope {
    let base_date = previous_import.scope_end_date().unwrap_or(today);
    let lookback_start = base_date
        .checked_sub_days(Days::new(CATCH_UP_LOOKBACK_DAYS))
        .unwrap_or(base_date);
    let start = lookback_start.min(today);

    CollectionScope::incremental(start, today).expect("planner builds a valid catch-up scope")
}

fn today_scope(today: NaiveDate) -> CollectionScope {
    CollectionScope::incremental(today, today).expect("planner builds a valid freshness scope")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("date")
    }

    fn planner() -> RefreshPolicyPlanner {
        RefreshPolicyPlanner::new()
    }

    fn daily_target() -> RefreshPlanTarget {
        RefreshPlanTarget::daily(SourceKey::ClaudeCode, "Asia/Jakarta")
    }

    fn successful_incremental(end_date: NaiveDate) -> SuccessfulImportState {
        SuccessfulImportState::new(
            SourceKey::ClaudeCode,
            CollectionProjection::Daily,
            CollectionScope::incremental(end_date, end_date).expect("scope"),
            100,
        )
    }

    fn successful_full() -> SuccessfulImportState {
        SuccessfulImportState::new(
            SourceKey::ClaudeCode,
            CollectionProjection::Daily,
            CollectionScope::Full,
            100,
        )
    }

    #[test]
    fn missing_baseline_plans_full_refresh() {
        let plan = planner().plan(RefreshPlanRequest::new(
            daily_target(),
            RefreshPlanMode::CatchUp,
            date(2026, 6, 28),
            None,
        ));

        assert_eq!(plan.scope(), &CollectionScope::Full);
        assert_eq!(plan.target().source(), SourceKey::ClaudeCode);
        assert_eq!(plan.target().projection(), CollectionProjection::Daily);
        assert_eq!(plan.target().aggregation_timezone(), Some("Asia/Jakarta"));
    }

    #[test]
    fn catch_up_uses_two_day_lookback_from_previous_scope_end() {
        let plan = planner().plan(RefreshPlanRequest::new(
            daily_target(),
            RefreshPlanMode::CatchUp,
            date(2026, 6, 28),
            Some(successful_incremental(date(2026, 6, 20))),
        ));

        assert_eq!(
            plan.scope(),
            &CollectionScope::incremental(date(2026, 6, 18), date(2026, 6, 28)).expect("scope")
        );
    }

    #[test]
    fn catch_up_never_starts_after_today() {
        let plan = planner().plan(RefreshPlanRequest::new(
            daily_target(),
            RefreshPlanMode::CatchUp,
            date(2026, 6, 28),
            Some(successful_incremental(date(2026, 7, 3))),
        ));

        assert_eq!(
            plan.scope(),
            &CollectionScope::incremental(date(2026, 6, 28), date(2026, 6, 28)).expect("scope")
        );
    }

    #[test]
    fn full_baseline_catch_up_uses_today_with_lookback() {
        let plan = planner().plan(RefreshPlanRequest::new(
            daily_target(),
            RefreshPlanMode::CatchUp,
            date(2026, 6, 28),
            Some(successful_full()),
        ));

        assert_eq!(
            plan.scope(),
            &CollectionScope::incremental(date(2026, 6, 26), date(2026, 6, 28)).expect("scope")
        );
    }

    #[test]
    fn freshness_uses_today_only_after_baseline_exists() {
        let plan = planner().plan(RefreshPlanRequest::new(
            daily_target(),
            RefreshPlanMode::Freshness,
            date(2026, 6, 28),
            Some(successful_incremental(date(2026, 6, 20))),
        ));

        assert_eq!(
            plan.scope(),
            &CollectionScope::incremental(date(2026, 6, 28), date(2026, 6, 28)).expect("scope")
        );
    }

    #[test]
    fn freshness_without_baseline_still_plans_full() {
        let plan = planner().plan(RefreshPlanRequest::new(
            RefreshPlanTarget::session(SourceKey::ClaudeCode),
            RefreshPlanMode::Freshness,
            date(2026, 6, 28),
            None,
        ));

        assert_eq!(plan.scope(), &CollectionScope::Full);
    }

    #[test]
    fn mismatched_previous_import_does_not_count_as_baseline() {
        let plan = planner().plan(RefreshPlanRequest::new(
            RefreshPlanTarget::daily(SourceKey::Codex, "Asia/Jakarta"),
            RefreshPlanMode::CatchUp,
            date(2026, 6, 28),
            Some(successful_incremental(date(2026, 6, 20))),
        ));

        assert_eq!(plan.scope(), &CollectionScope::Full);
    }
}
