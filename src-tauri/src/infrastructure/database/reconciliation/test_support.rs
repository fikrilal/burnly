use chrono::{NaiveDate, TimeZone, Utc};

use super::store::SqliteReconciliationStore;
use crate::application::collection::{
    CandidateProvenance, CollectionId, CollectionOutcome, CollectionProjection, CollectionScope,
    CollectorKey, DailyUsageCandidate, ModelUsageCandidate, SessionUsageCandidate,
};
use crate::application::ports::run_store::RunStore;
use crate::application::ports::usage_store::UsageStore;
use crate::application::reconciliation::{
    DailyReconciliationRequest, ImportCollector, ImportRunId, ImportRunSpec, JobKey, RefreshRunId,
    RefreshRunSpec, RefreshTrigger, SessionReconciliationRequest, SourceId,
};
use crate::domain::source::SourceKey;
use crate::domain::usage::{
    CostKind, CurrencyCode, DataQuality, TokenUsage, UsageCost, ValuedCostStatus,
};
use crate::infrastructure::database::Database;

pub(super) fn migrated_store() -> (tempfile::TempDir, SqliteReconciliationStore) {
    let directory = tempfile::TempDir::new().expect("create temporary directory");
    let database_path = directory.path().join("burnly.sqlite3");
    let mut database = Database::open(&database_path).expect("open database");
    database.migrate_to_latest().expect("migrate database");

    (directory, SqliteReconciliationStore::new(database))
}

pub(super) fn refresh_spec(job_key: &str) -> RefreshRunSpec {
    RefreshRunSpec::new(
        JobKey::new(job_key).expect("job key"),
        RefreshTrigger::Manual,
        "0.1.0",
    )
    .expect("refresh spec")
}

pub(super) fn daily_import_spec(
    refresh_run_id: RefreshRunId,
    source_id: SourceId,
) -> ImportRunSpec {
    ImportRunSpec::new(
        refresh_run_id,
        source_id,
        ImportCollector::new("ccusage", "20.0.11", 1).expect("collector"),
        CollectionProjection::Daily,
        CollectionScope::Full,
        Some("Asia/Jakarta".to_owned()),
    )
    .expect("import spec")
}

pub(super) fn session_import_spec(
    refresh_run_id: RefreshRunId,
    source_id: SourceId,
) -> ImportRunSpec {
    ImportRunSpec::new(
        refresh_run_id,
        source_id,
        ImportCollector::new("ccusage", "20.0.11", 1).expect("collector"),
        CollectionProjection::Session,
        CollectionScope::Full,
        None,
    )
    .expect("session import spec")
}

pub(super) fn daily_import_spec_with_scope(
    refresh_run_id: RefreshRunId,
    source_id: SourceId,
    scope: CollectionScope,
    aggregation_timezone: &str,
) -> ImportRunSpec {
    ImportRunSpec::new(
        refresh_run_id,
        source_id,
        ImportCollector::new("ccusage", "20.0.11", 1).expect("collector"),
        CollectionProjection::Daily,
        scope,
        Some(aggregation_timezone.to_owned()),
    )
    .expect("daily import spec")
}

pub(super) fn setup_import(
    store: &SqliteReconciliationStore,
    job_key: &str,
) -> (SourceId, ImportRunId) {
    let source_id = store
        .resolve_source(SourceKey::ClaudeCode, 100)
        .expect("resolve source");
    let refresh_run_id = store
        .begin_refresh_run(refresh_spec(job_key), 100)
        .expect("begin refresh run");
    let import_run_id = store
        .begin_import_run(daily_import_spec(refresh_run_id, source_id), 110)
        .expect("begin import run");

    (source_id, import_run_id)
}

pub(super) fn candidate(source_key: &str, total: u64, model: &str) -> DailyUsageCandidate {
    DailyUsageCandidate {
        provenance: provenance(),
        source_key: source_key.to_owned(),
        usage_date: NaiveDate::from_ymd_opt(2026, 6, 13).expect("date"),
        aggregation_timezone: "UTC".to_owned(),
        tokens: classified_tokens(total),
        cost: estimated_cost(total * 100),
        model_breakdowns: vec![ModelUsageCandidate {
            raw_model_id: model.to_owned(),
            tokens: classified_tokens(total),
            cost: estimated_cost(total * 100),
        }],
    }
}

pub(super) fn session_candidate(project_path: &str) -> SessionUsageCandidate {
    SessionUsageCandidate {
        provenance: provenance(),
        source_key: "claude-code:session:v1:session-1".to_owned(),
        source_session_id: "session-1".to_owned(),
        project_path: Some(project_path.to_owned()),
        first_activity_at: None,
        last_activity_at: None,
        tokens: classified_tokens(100),
        cost: estimated_cost(10_000),
        model_breakdowns: Vec::new(),
    }
}

pub(super) fn reconcile_session() -> SqliteReconciliationStore {
    let directory = tempfile::TempDir::new().expect("create temporary directory");
    let database_path = directory.keep().join("burnly.sqlite3");
    let mut database = Database::open(&database_path).expect("open database");
    database.migrate_to_latest().expect("migrate database");
    database.ensure_app_settings("UTC", 100).expect("settings");
    let store = SqliteReconciliationStore::new(database);
    let source_id = store
        .resolve_source(SourceKey::ClaudeCode, 100)
        .expect("resolve source");
    let refresh_run_id = store
        .begin_refresh_run(refresh_spec("session-refresh"), 100)
        .expect("begin refresh");
    let import_run_id = store
        .begin_import_run(session_import_spec(refresh_run_id, source_id), 110)
        .expect("begin import");
    store
        .reconcile_session(SessionReconciliationRequest::new(
            source_id,
            import_run_id,
            CollectionScope::Full,
            CollectionOutcome::Complete,
            120,
            vec![session_candidate("/home/dante/secret-project")],
        ))
        .expect("reconcile session");
    store
}

pub(super) fn request(
    source_id: SourceId,
    import_run_id: ImportRunId,
    candidates: Vec<DailyUsageCandidate>,
) -> DailyReconciliationRequest {
    DailyReconciliationRequest::new(
        source_id,
        import_run_id,
        CollectionScope::Full,
        CollectionOutcome::Complete,
        120,
        candidates,
    )
}

pub(super) fn source_and_refresh(store: &SqliteReconciliationStore) -> (SourceId, RefreshRunId) {
    let source_id = store
        .resolve_source(SourceKey::ClaudeCode, 100)
        .expect("resolve source");
    let refresh_run_id = store
        .begin_refresh_run(refresh_spec("refresh-1"), 100)
        .expect("begin refresh run");

    (source_id, refresh_run_id)
}

pub(super) fn next_import(
    store: &SqliteReconciliationStore,
    source_id: SourceId,
    refresh_run_id: RefreshRunId,
) -> ImportRunId {
    store
        .begin_import_run(daily_import_spec(refresh_run_id, source_id), 110)
        .expect("begin import run")
}

fn provenance() -> CandidateProvenance {
    CandidateProvenance {
        source: SourceKey::ClaudeCode,
        collector: CollectorKey::new("ccusage").expect("collector key"),
        collector_version: "20.0.11".to_owned(),
        profile_version: 1,
        collection_id: CollectionId::new("collection-1").expect("collection id"),
        observed_at: Utc
            .with_ymd_and_hms(2026, 6, 14, 12, 0, 0)
            .single()
            .expect("timestamp"),
        data_quality: DataQuality::Complete,
        warnings: Vec::new(),
    }
}

fn classified_tokens(total: u64) -> TokenUsage {
    TokenUsage::new(Some(total), Some(0), Some(0), Some(0), total).expect("tokens")
}

fn estimated_cost(amount_micros: u64) -> UsageCost {
    UsageCost::Valued {
        amount_micros,
        currency: CurrencyCode::new("USD").expect("currency"),
        kind: CostKind::CollectorCalculated,
        status: ValuedCostStatus::Estimated,
    }
}
