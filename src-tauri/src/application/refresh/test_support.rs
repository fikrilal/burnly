use std::sync::Arc;
use std::thread;
use std::time::Duration;

use super::{
    collection_for_request, empty_collection_for_request, FakeClock, FakeRunStore, FakeUsageStore,
    ScriptedCollector,
};
use crate::application::collection::{CollectionOutcome, CollectionProjection, CollectionScope};
use crate::application::ports::collector::Collector;
use crate::application::reconciliation::{ImportOutcome, RefreshOutcome, SuccessfulImportState};
use crate::application::refresh::coordinator::RefreshCoordinator;
use crate::application::refresh::state::RefreshSnapshot;
use crate::application::refresh::target::refresh_targets;
use crate::domain::source::SourceKey;

pub(super) fn expected_refresh_targets() -> Vec<(SourceKey, CollectionProjection)> {
    refresh_targets()
        .iter()
        .map(|target| (target.source, target.projection))
        .collect()
}

pub(super) fn expected_refresh_projections() -> Vec<CollectionProjection> {
    refresh_targets()
        .iter()
        .map(|target| target.projection)
        .collect()
}

pub(super) fn repeated_import_outcomes(outcome: ImportOutcome) -> Vec<ImportOutcome> {
    vec![outcome; refresh_targets().len()]
}

pub(super) fn repeated_collection_outcomes(outcome: CollectionOutcome) -> Vec<CollectionOutcome> {
    vec![outcome; refresh_targets().len()]
}

pub(super) fn repeated_refresh_outcomes(outcome: RefreshOutcome) -> Vec<RefreshOutcome> {
    vec![outcome]
}

pub(super) fn repeated_scope(scope: CollectionScope) -> Vec<CollectionScope> {
    vec![scope; refresh_targets().len()]
}

pub(super) fn date(year: i32, month: u32, day: u32) -> chrono::NaiveDate {
    chrono::NaiveDate::from_ymd_opt(year, month, day).expect("date")
}

pub(super) fn seed_successful_imports(run_store: &FakeRunStore, scope: CollectionScope) {
    for target in refresh_targets() {
        run_store.seed_successful_import(SuccessfulImportState::new(
            target.source,
            target.projection,
            scope.clone(),
            1,
        ));
    }
}

pub(super) fn successful_collector() -> Arc<ScriptedCollector> {
    Arc::new(ScriptedCollector::new(|request| {
        Ok(collection_for_request(&request, Vec::new()))
    }))
}

pub(super) fn empty_collector() -> Arc<ScriptedCollector> {
    Arc::new(ScriptedCollector::new(|request| {
        Ok(empty_collection_for_request(&request))
    }))
}

pub(super) fn coordinator_with(
    collector: Arc<dyn Collector>,
) -> (RefreshCoordinator, Arc<FakeRunStore>, Arc<FakeUsageStore>) {
    let run_store = Arc::new(FakeRunStore::new());
    let usage_store = Arc::new(FakeUsageStore::new());
    let clock = Arc::new(FakeClock { now_ms: 1_000 });
    let coordinator = RefreshCoordinator::new(
        collector,
        run_store.clone(),
        usage_store.clone(),
        clock,
        "0.1.0",
        "UTC",
    );

    (coordinator, run_store, usage_store)
}

pub(super) fn await_terminal(coordinator: &RefreshCoordinator) -> RefreshSnapshot {
    for _ in 0..1_000 {
        let snapshot = coordinator.snapshot();
        if !snapshot.status.is_active() {
            return snapshot;
        }
        thread::sleep(Duration::from_millis(1));
    }
    panic!("refresh did not reach a terminal state");
}
