//! Refresh request planning and collection request construction.
//!
//! This module translates a refresh target plus scope policy into a concrete
//! `CollectionRequest`. It keeps source/projection request details separate from
//! the coordinator's worker lifecycle and persistence flow.

use chrono::{DateTime, Utc};

use crate::application::collection::{CollectionId, CollectionProjection, CollectionRequest};
use crate::application::ports::run_store::RunStore;
use crate::application::refresh::planner::{
    RefreshPlanMode, RefreshPlanRequest, RefreshPolicyPlanner,
};

use super::target::{local_date, projection_label, RefreshTarget};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RefreshScopePolicy {
    Full,
    CatchUp,
    Freshness,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RequestPlanError {
    InvalidRequest,
    InvalidTimezone,
    InvalidImportState,
    ImportStateUnavailable,
}

impl RequestPlanError {
    pub(super) const fn code(self) -> &'static str {
        match self {
            Self::InvalidRequest => "refresh.request",
            Self::InvalidTimezone => "refresh.timezone",
            Self::InvalidImportState | Self::ImportStateUnavailable => "refresh.import_state",
        }
    }

    pub(super) const fn summary(self) -> &'static str {
        match self {
            Self::InvalidRequest => "Refresh request is invalid.",
            Self::InvalidTimezone => "Refresh timezone is invalid.",
            Self::InvalidImportState => "Refresh import state is invalid.",
            Self::ImportStateUnavailable => "Could not read the latest successful import state.",
        }
    }
}

pub(super) fn planned_collection_request(
    run_store: &dyn RunStore,
    job_id: &str,
    target: RefreshTarget,
    requested_at: DateTime<Utc>,
    aggregation_timezone: &str,
    scope_policy: RefreshScopePolicy,
) -> Result<CollectionRequest, RequestPlanError> {
    let scope = match scope_policy {
        RefreshScopePolicy::Full => crate::application::collection::CollectionScope::Full,
        RefreshScopePolicy::CatchUp | RefreshScopePolicy::Freshness => {
            let today = local_date(requested_at, aggregation_timezone)
                .map_err(|_| RequestPlanError::InvalidTimezone)?;
            let lookup = target
                .import_lookup(aggregation_timezone)
                .map_err(|_| RequestPlanError::InvalidImportState)?;
            let previous_import = run_store
                .latest_successful_import(lookup)
                .map_err(|_| RequestPlanError::ImportStateUnavailable)?;
            let mode = match scope_policy {
                RefreshScopePolicy::CatchUp => RefreshPlanMode::CatchUp,
                RefreshScopePolicy::Freshness => RefreshPlanMode::Freshness,
                RefreshScopePolicy::Full => unreachable!("full scope returned earlier"),
            };
            let plan = RefreshPolicyPlanner::new().plan(RefreshPlanRequest::new(
                target.plan_target(aggregation_timezone),
                mode,
                today,
                previous_import,
            ));
            plan.scope().clone()
        }
    };

    collection_request(job_id, target, scope, requested_at, aggregation_timezone)
}

fn collection_request(
    job_id: &str,
    target: RefreshTarget,
    scope: crate::application::collection::CollectionScope,
    requested_at: DateTime<Utc>,
    aggregation_timezone: &str,
) -> Result<CollectionRequest, RequestPlanError> {
    let collection_id = CollectionId::new(format!(
        "{job_id}:{}:{}",
        target.source.as_str(),
        projection_label(target.projection)
    ))
    .map_err(|_| RequestPlanError::InvalidRequest)?;

    match target.projection {
        CollectionProjection::Daily => CollectionRequest::daily(
            collection_id,
            target.source,
            scope,
            aggregation_timezone.to_owned(),
            requested_at,
        )
        .map_err(|_| RequestPlanError::InvalidRequest),
        CollectionProjection::Session => Ok(CollectionRequest::session(
            collection_id,
            target.source,
            scope,
            requested_at,
        )),
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;
    use crate::application::collection::CollectionScope;
    use crate::application::ports::run_store::RunStoreError;
    use crate::application::reconciliation::{
        ImportRunId, ImportRunLookup, ImportRunSpec, RefreshRunCompletion, RefreshRunId,
        RefreshRunSpec, SourceId, SuccessfulImportState,
    };
    use crate::domain::source::SourceKey;

    #[derive(Default)]
    struct FakeRunStore {
        latest_import: Option<SuccessfulImportState>,
        latest_import_error: bool,
    }

    impl RunStore for FakeRunStore {
        fn resolve_source(
            &self,
            _source: SourceKey,
            _now_ms: i64,
        ) -> Result<SourceId, RunStoreError> {
            unimplemented!("request planning does not resolve sources")
        }

        fn begin_refresh_run(
            &self,
            _spec: RefreshRunSpec,
            _now_ms: i64,
        ) -> Result<RefreshRunId, RunStoreError> {
            unimplemented!("request planning does not begin refresh runs")
        }

        fn complete_refresh_run(
            &self,
            _id: RefreshRunId,
            _completion: RefreshRunCompletion,
        ) -> Result<(), RunStoreError> {
            unimplemented!("request planning does not complete refresh runs")
        }

        fn begin_import_run(
            &self,
            _spec: ImportRunSpec,
            _started_at_ms: i64,
        ) -> Result<ImportRunId, RunStoreError> {
            unimplemented!("request planning does not begin import runs")
        }

        fn complete_import_run(
            &self,
            _id: ImportRunId,
            _completion: crate::application::reconciliation::ImportRunCompletion,
        ) -> Result<(), RunStoreError> {
            unimplemented!("request planning does not complete import runs")
        }

        fn latest_successful_import(
            &self,
            _lookup: ImportRunLookup,
        ) -> Result<Option<SuccessfulImportState>, RunStoreError> {
            if self.latest_import_error {
                return Err(RunStoreError::Backend);
            }
            Ok(self.latest_import.clone())
        }
    }

    fn requested_at() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 7, 4, 7, 0, 0)
            .single()
            .expect("timestamp")
    }

    fn target(projection: CollectionProjection) -> RefreshTarget {
        RefreshTarget {
            source: SourceKey::Codex,
            projection,
        }
    }

    #[test]
    fn full_daily_request_uses_full_scope_and_timezone() {
        let request = planned_collection_request(
            &FakeRunStore::default(),
            "refresh-1",
            target(CollectionProjection::Daily),
            requested_at(),
            "Asia/Jakarta",
            RefreshScopePolicy::Full,
        )
        .expect("planned request");

        assert_eq!(request.collection_id().as_str(), "refresh-1:codex:daily");
        assert_eq!(request.scope(), &CollectionScope::Full);
        assert_eq!(request.aggregation_timezone(), Some("Asia/Jakarta"));
    }

    #[test]
    fn full_session_request_uses_full_scope_without_timezone() {
        let request = planned_collection_request(
            &FakeRunStore::default(),
            "refresh-1",
            target(CollectionProjection::Session),
            requested_at(),
            "Asia/Jakarta",
            RefreshScopePolicy::Full,
        )
        .expect("planned request");

        assert_eq!(request.collection_id().as_str(), "refresh-1:codex:session");
        assert_eq!(request.scope(), &CollectionScope::Full);
        assert_eq!(request.aggregation_timezone(), None);
    }

    #[test]
    fn freshness_with_baseline_uses_incremental_today_scope() {
        let previous = SuccessfulImportState::new(
            SourceKey::Codex,
            CollectionProjection::Daily,
            CollectionScope::Full,
            100,
        );
        let run_store = FakeRunStore {
            latest_import: Some(previous),
            latest_import_error: false,
        };

        let request = planned_collection_request(
            &run_store,
            "refresh-1",
            target(CollectionProjection::Daily),
            requested_at(),
            "UTC",
            RefreshScopePolicy::Freshness,
        )
        .expect("planned request");

        assert_eq!(
            request.scope(),
            &CollectionScope::incremental(
                chrono::NaiveDate::from_ymd_opt(2026, 7, 4).expect("start date"),
                chrono::NaiveDate::from_ymd_opt(2026, 7, 4).expect("end date"),
            )
            .expect("incremental scope")
        );
    }

    #[test]
    fn catch_up_without_baseline_uses_full_scope() {
        let request = planned_collection_request(
            &FakeRunStore::default(),
            "refresh-1",
            target(CollectionProjection::Daily),
            requested_at(),
            "UTC",
            RefreshScopePolicy::CatchUp,
        )
        .expect("planned request");

        assert_eq!(request.scope(), &CollectionScope::Full);
    }

    #[test]
    fn invalid_timezone_returns_stable_error() {
        let error = planned_collection_request(
            &FakeRunStore::default(),
            "refresh-1",
            target(CollectionProjection::Daily),
            requested_at(),
            "not-a-timezone",
            RefreshScopePolicy::Freshness,
        )
        .expect_err("invalid timezone should fail");

        assert_eq!(error.code(), "refresh.timezone");
        assert_eq!(error.summary(), "Refresh timezone is invalid.");
    }

    #[test]
    fn import_state_read_failure_returns_stable_error() {
        let run_store = FakeRunStore {
            latest_import: None,
            latest_import_error: true,
        };

        let error = planned_collection_request(
            &run_store,
            "refresh-1",
            target(CollectionProjection::Daily),
            requested_at(),
            "UTC",
            RefreshScopePolicy::Freshness,
        )
        .expect_err("import state read should fail");

        assert_eq!(error.code(), "refresh.import_state");
        assert_eq!(
            error.summary(),
            "Could not read the latest successful import state."
        );
    }
}
