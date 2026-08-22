use chrono::NaiveDate;
use rusqlite::params;

use super::runs::{
    InterruptedRunRecovery, INTERRUPTED_IMPORT_ERROR_CODE, INTERRUPTED_REFRESH_ERROR_CODE,
};
use super::store::SqliteReconciliationStore;
use super::test_support::*;
use crate::application::collection::{CollectionOutcome, CollectionProjection, CollectionScope};
use crate::application::ports::run_store::{RunStore, RunStoreError};
use crate::application::ports::usage_store::{UsageStore, UsageStoreError};
use crate::application::reconciliation::{
    DailyReconciliationRequest, ImportCollector, ImportOutcome, ImportRunCompletion, ImportRunId,
    ImportRunLookup, ImportRunSpec, RefreshOutcome, RefreshRunCompletion, RefreshRunId, RunError,
};
use crate::domain::source::SourceKey;
use crate::domain::usage::TokenUsage;
use crate::infrastructure::database::Database;
use crate::infrastructure::project_identity::ProjectPathIdentity;

#[test]
fn resolves_source_get_or_create_is_idempotent() {
    let (_directory, store) = migrated_store();

    let first = store
        .resolve_source(SourceKey::ClaudeCode, 100)
        .expect("first resolve");
    let second = store
        .resolve_source(SourceKey::ClaudeCode, 200)
        .expect("second resolve");

    assert_eq!(first, second);
}

#[test]
fn refresh_run_lifecycle_reaches_a_terminal_status() {
    let (_directory, store) = migrated_store();

    let refresh_run_id = store
        .begin_refresh_run(refresh_spec("refresh-1"), 100)
        .expect("begin refresh run");

    store
        .complete_refresh_run(
            refresh_run_id,
            RefreshRunCompletion {
                outcome: RefreshOutcome::Succeeded,
                finished_at_ms: 200,
                error: None,
            },
        )
        .expect("complete refresh run");
}

#[test]
fn import_run_lifecycle_records_counts_and_redacted_error() {
    let (_directory, store) = migrated_store();

    let refresh_run_id = store
        .begin_refresh_run(refresh_spec("refresh-1"), 100)
        .expect("begin refresh run");
    let source_id = store
        .resolve_source(SourceKey::ClaudeCode, 100)
        .expect("resolve source");

    let import_run_id = store
        .begin_import_run(daily_import_spec(refresh_run_id, source_id), 110)
        .expect("begin import run");

    store
        .complete_import_run(
            import_run_id,
            ImportRunCompletion {
                outcome: ImportOutcome::Partial,
                records_seen: 12,
                records_rejected: 3,
                finished_at_ms: 180,
                error: Some(
                    RunError::new("collector.partial", "some records were rejected")
                        .expect("run error"),
                ),
            },
        )
        .expect("complete import run");
}

#[test]
fn incremental_import_run_persists_scope_dates() {
    let (_directory, store) = migrated_store();

    let refresh_run_id = store
        .begin_refresh_run(refresh_spec("refresh-1"), 100)
        .expect("begin refresh run");
    let source_id = store
        .resolve_source(SourceKey::ClaudeCode, 100)
        .expect("resolve source");
    let scope = CollectionScope::incremental(
        NaiveDate::from_ymd_opt(2026, 6, 1).expect("start"),
        NaiveDate::from_ymd_opt(2026, 6, 14).expect("end"),
    )
    .expect("incremental scope");
    let spec = ImportRunSpec::new(
        refresh_run_id,
        source_id,
        ImportCollector::new("ccusage", "20.0.11", 1).expect("collector"),
        CollectionProjection::Daily,
        scope,
        Some("UTC".to_owned()),
    )
    .expect("incremental import spec");

    store
        .begin_import_run(spec, 110)
        .expect("begin incremental import run");
}

#[test]
fn latest_successful_import_returns_matching_source_projection_and_timezone() {
    let (_directory, store) = migrated_store();
    let source_id = store
        .resolve_source(SourceKey::ClaudeCode, 100)
        .expect("resolve source");
    let utc_refresh_id = store
        .begin_refresh_run(refresh_spec("refresh-utc"), 100)
        .expect("begin utc refresh");
    let jakarta_refresh_id = store
        .begin_refresh_run(refresh_spec("refresh-jakarta"), 200)
        .expect("begin jakarta refresh");

    let utc_import_id = store
        .begin_import_run(
            daily_import_spec_with_scope(utc_refresh_id, source_id, CollectionScope::Full, "UTC"),
            110,
        )
        .expect("begin utc import");
    let jakarta_import_id = store
        .begin_import_run(
            daily_import_spec_with_scope(
                jakarta_refresh_id,
                source_id,
                CollectionScope::Full,
                "Asia/Jakarta",
            ),
            210,
        )
        .expect("begin jakarta import");

    store
        .complete_import_run(
            utc_import_id,
            ImportRunCompletion {
                outcome: ImportOutcome::Succeeded,
                records_seen: 1,
                records_rejected: 0,
                finished_at_ms: 150,
                error: None,
            },
        )
        .expect("complete utc import");
    store
        .complete_import_run(
            jakarta_import_id,
            ImportRunCompletion {
                outcome: ImportOutcome::Succeeded,
                records_seen: 1,
                records_rejected: 0,
                finished_at_ms: 250,
                error: None,
            },
        )
        .expect("complete jakarta import");

    let state = store
        .latest_successful_import(
            ImportRunLookup::new(
                SourceKey::ClaudeCode,
                CollectionProjection::Daily,
                Some("UTC".to_owned()),
            )
            .expect("lookup"),
        )
        .expect("latest import")
        .expect("matching import");

    assert_eq!(state.source(), SourceKey::ClaudeCode);
    assert_eq!(state.projection(), CollectionProjection::Daily);
    assert_eq!(state.scope(), &CollectionScope::Full);
    assert_eq!(state.finished_at_ms(), 150);
}

#[test]
fn latest_successful_import_ignores_failed_runs_and_returns_latest_scope() {
    let (_directory, store) = migrated_store();
    let source_id = store
        .resolve_source(SourceKey::ClaudeCode, 100)
        .expect("resolve source");
    let failed_refresh_id = store
        .begin_refresh_run(refresh_spec("refresh-failed"), 100)
        .expect("begin failed refresh");
    let latest_refresh_id = store
        .begin_refresh_run(refresh_spec("refresh-latest"), 200)
        .expect("begin latest refresh");
    let failed_scope = CollectionScope::incremental(
        NaiveDate::from_ymd_opt(2026, 6, 1).expect("start"),
        NaiveDate::from_ymd_opt(2026, 6, 14).expect("end"),
    )
    .expect("failed scope");
    let latest_scope = CollectionScope::incremental(
        NaiveDate::from_ymd_opt(2026, 6, 18).expect("start"),
        NaiveDate::from_ymd_opt(2026, 6, 20).expect("end"),
    )
    .expect("latest scope");

    let failed_import_id = store
        .begin_import_run(
            daily_import_spec_with_scope(failed_refresh_id, source_id, failed_scope, "UTC"),
            110,
        )
        .expect("begin failed import");
    let latest_import_id = store
        .begin_import_run(
            daily_import_spec_with_scope(latest_refresh_id, source_id, latest_scope.clone(), "UTC"),
            210,
        )
        .expect("begin latest import");

    store
        .complete_import_run(
            failed_import_id,
            ImportRunCompletion {
                outcome: ImportOutcome::Failed,
                records_seen: 0,
                records_rejected: 0,
                finished_at_ms: 400,
                error: Some(RunError::new("collector.failed", "failed").expect("run error")),
            },
        )
        .expect("complete failed import");
    store
        .complete_import_run(
            latest_import_id,
            ImportRunCompletion {
                outcome: ImportOutcome::Succeeded,
                records_seen: 1,
                records_rejected: 0,
                finished_at_ms: 300,
                error: None,
            },
        )
        .expect("complete latest import");

    let state = store
        .latest_successful_import(
            ImportRunLookup::new(
                SourceKey::ClaudeCode,
                CollectionProjection::Daily,
                Some("UTC".to_owned()),
            )
            .expect("lookup"),
        )
        .expect("latest import")
        .expect("successful import");

    assert_eq!(state.scope(), &latest_scope);
    assert_eq!(state.finished_at_ms(), 300);
}

#[test]
fn latest_successful_import_returns_none_when_identity_has_no_success() {
    let (_directory, store) = migrated_store();

    let result = store
        .latest_successful_import(
            ImportRunLookup::new(
                SourceKey::Codex,
                CollectionProjection::Daily,
                Some("UTC".to_owned()),
            )
            .expect("lookup"),
        )
        .expect("lookup succeeds");

    assert_eq!(result, None);
}

#[test]
fn latest_successful_import_requires_compatible_collector_profile() {
    let (_directory, store) = migrated_store();
    let source_id = store
        .resolve_source(SourceKey::Antigravity, 100)
        .expect("resolve source");
    let refresh_run_id = store
        .begin_refresh_run(refresh_spec("antigravity-profile-1"), 100)
        .expect("begin refresh");
    let import_id = store
        .begin_import_run(
            ImportRunSpec::new(
                refresh_run_id,
                source_id,
                ImportCollector::new("antigravity", "local-rpc", 1).expect("collector"),
                CollectionProjection::Daily,
                CollectionScope::Full,
                Some("UTC".to_owned()),
            )
            .expect("import spec"),
            110,
        )
        .expect("begin import");
    store
        .complete_import_run(
            import_id,
            ImportRunCompletion {
                outcome: ImportOutcome::Succeeded,
                records_seen: 1,
                records_rejected: 0,
                finished_at_ms: 150,
                error: None,
            },
        )
        .expect("complete import");

    let profile_2 = store
        .latest_successful_import(
            ImportRunLookup::compatible(
                SourceKey::Antigravity,
                CollectionProjection::Daily,
                Some("UTC".to_owned()),
                "antigravity",
                2,
            )
            .expect("lookup"),
        )
        .expect("lookup profile 2");
    let profile_1 = store
        .latest_successful_import(
            ImportRunLookup::compatible(
                SourceKey::Antigravity,
                CollectionProjection::Daily,
                Some("UTC".to_owned()),
                "antigravity",
                1,
            )
            .expect("lookup"),
        )
        .expect("lookup profile 1");

    assert_eq!(profile_2, None);
    assert!(profile_1.is_some());
}

#[test]
fn duplicate_job_key_is_rejected() {
    let (_directory, store) = migrated_store();

    store
        .begin_refresh_run(refresh_spec("refresh-1"), 100)
        .expect("first refresh run");
    let error = store
        .begin_refresh_run(refresh_spec("refresh-1"), 150)
        .expect_err("duplicate job key");

    assert_eq!(error, RunStoreError::DuplicateJobKey);
}

#[test]
fn completing_a_missing_run_reports_not_found() {
    let (_directory, store) = migrated_store();

    let error = store
        .complete_refresh_run(
            RefreshRunId::new(999),
            RefreshRunCompletion {
                outcome: RefreshOutcome::Failed,
                finished_at_ms: 200,
                error: None,
            },
        )
        .expect_err("missing refresh run");

    assert_eq!(error, RunStoreError::RunNotFound);
}

#[test]
fn interrupted_run_recovery_terminalizes_only_active_rows() {
    let (_directory, store) = migrated_store();
    let source_id = store
        .resolve_source(SourceKey::ClaudeCode, 100)
        .expect("resolve source");

    let running_refresh_id = store
        .begin_refresh_run(refresh_spec("refresh-running"), 100)
        .expect("begin running refresh");
    let running_import_id = store
        .begin_import_run(daily_import_spec(running_refresh_id, source_id), 110)
        .expect("begin running import");
    let queued_refresh_id = store
        .begin_refresh_run(refresh_spec("refresh-queued"), 120)
        .expect("begin queued refresh");
    let cancelling_refresh_id = store
        .begin_refresh_run(refresh_spec("refresh-cancelling"), 130)
        .expect("begin cancelling refresh");
    let succeeded_refresh_id = store
        .begin_refresh_run(refresh_spec("refresh-succeeded"), 140)
        .expect("begin succeeded refresh");
    let succeeded_import_id = store
        .begin_import_run(daily_import_spec(succeeded_refresh_id, source_id), 145)
        .expect("begin succeeded import");
    store
        .complete_import_run(
            succeeded_import_id,
            ImportRunCompletion {
                outcome: ImportOutcome::Succeeded,
                records_seen: 2,
                records_rejected: 0,
                finished_at_ms: 150,
                error: None,
            },
        )
        .expect("complete succeeded import");
    store
        .complete_refresh_run(
            succeeded_refresh_id,
            RefreshRunCompletion {
                outcome: RefreshOutcome::Succeeded,
                finished_at_ms: 160,
                error: None,
            },
        )
        .expect("complete succeeded refresh");

    {
        let database = store.database.lock().expect("store lock");
        database
            .connection()
            .execute(
                "UPDATE refresh_runs SET status = 'queued' WHERE id = ?1",
                [queued_refresh_id.value()],
            )
            .expect("mark queued");
        database
            .connection()
            .execute(
                "UPDATE refresh_runs SET status = 'cancelling' WHERE id = ?1",
                [cancelling_refresh_id.value()],
            )
            .expect("mark cancelling");
    }

    let recovery = store
        .recover_interrupted_runs(500)
        .expect("recover interrupted runs");

    assert_eq!(
        recovery,
        InterruptedRunRecovery {
            refresh_runs: 3,
            import_runs: 1,
        }
    );
    assert_eq!(
        refresh_status(&store, running_refresh_id),
        (
            "failed".to_owned(),
            Some(500),
            Some(INTERRUPTED_REFRESH_ERROR_CODE.to_owned())
        )
    );
    assert_eq!(
        refresh_status(&store, queued_refresh_id),
        (
            "failed".to_owned(),
            Some(500),
            Some(INTERRUPTED_REFRESH_ERROR_CODE.to_owned())
        )
    );
    assert_eq!(
        refresh_status(&store, cancelling_refresh_id),
        (
            "failed".to_owned(),
            Some(500),
            Some(INTERRUPTED_REFRESH_ERROR_CODE.to_owned())
        )
    );
    assert_eq!(
        import_status(&store, running_import_id),
        (
            "failed".to_owned(),
            Some(500),
            Some(INTERRUPTED_IMPORT_ERROR_CODE.to_owned())
        )
    );
    assert_eq!(
        refresh_status(&store, succeeded_refresh_id),
        ("succeeded".to_owned(), Some(160), None)
    );
    assert_eq!(
        import_status(&store, succeeded_import_id),
        ("succeeded".to_owned(), Some(150), None)
    );

    let second_recovery = store
        .recover_interrupted_runs(600)
        .expect("recover interrupted runs again");

    assert_eq!(
        second_recovery,
        InterruptedRunRecovery {
            refresh_runs: 0,
            import_runs: 0,
        }
    );
    assert_eq!(
        refresh_status(&store, running_refresh_id),
        (
            "failed".to_owned(),
            Some(500),
            Some(INTERRUPTED_REFRESH_ERROR_CODE.to_owned())
        )
    );
}

#[test]
fn interrupted_run_recovery_handles_future_started_rows() {
    let (_directory, store) = migrated_store();
    let source_id = store
        .resolve_source(SourceKey::ClaudeCode, 100)
        .expect("resolve source");
    let refresh_run_id = store
        .begin_refresh_run(refresh_spec("refresh-future"), 500)
        .expect("begin future refresh");
    let import_run_id = store
        .begin_import_run(daily_import_spec(refresh_run_id, source_id), 600)
        .expect("begin future import");

    let recovery = store
        .recover_interrupted_runs(100)
        .expect("recover future interrupted runs");

    assert_eq!(
        recovery,
        InterruptedRunRecovery {
            refresh_runs: 1,
            import_runs: 1,
        }
    );
    assert_eq!(
        refresh_status(&store, refresh_run_id),
        (
            "failed".to_owned(),
            Some(500),
            Some(INTERRUPTED_REFRESH_ERROR_CODE.to_owned())
        )
    );
    assert_eq!(
        import_status(&store, import_run_id),
        (
            "failed".to_owned(),
            Some(600),
            Some(INTERRUPTED_IMPORT_ERROR_CODE.to_owned())
        )
    );
}

fn refresh_status(
    store: &SqliteReconciliationStore,
    id: RefreshRunId,
) -> (String, Option<i64>, Option<String>) {
    let database = store.database.lock().expect("store lock");
    database
        .connection()
        .query_row(
            "SELECT status, finished_at_ms, error_code FROM refresh_runs WHERE id = ?1",
            [id.value()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("read refresh status")
}

fn import_status(
    store: &SqliteReconciliationStore,
    id: ImportRunId,
) -> (String, Option<i64>, Option<String>) {
    let database = store.database.lock().expect("store lock");
    database
        .connection()
        .query_row(
            "SELECT status, finished_at_ms, error_code FROM import_runs WHERE id = ?1",
            [id.value()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("read import status")
}

#[test]
fn session_reconciliation_persists_only_non_reversible_project_identity() {
    let store = reconcile_session();
    let database = store.database.lock().expect("store lock");
    let (identity_key, raw_path, fingerprint): (String, Option<String>, Vec<u8>) = database
        .connection()
        .query_row(
            "SELECT identity_key, raw_path, path_fingerprint FROM projects",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("read project");

    assert!(ProjectPathIdentity::is_key(&identity_key));
    assert!(!identity_key.contains("secret-project"));
    assert_eq!(raw_path, None);
    assert_eq!(fingerprint.len(), 32);
}

fn record_state(store: &SqliteReconciliationStore, source_key: &str) -> (String, i64) {
    let database = store.database.lock().expect("lock store");
    database
        .connection()
        .query_row(
            "SELECT record_state, absence_count FROM daily_usage WHERE source_key = ?1",
            params![source_key],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("record state query")
}

fn count(store: &SqliteReconciliationStore, table: &str) -> i64 {
    let database = store.database.lock().expect("lock store");
    database
        .connection()
        .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("count query")
}

fn daily_total(store: &SqliteReconciliationStore, source_key: &str) -> i64 {
    let database = store.database.lock().expect("lock store");
    database
        .connection()
        .query_row(
            "SELECT total_tokens FROM daily_usage WHERE source_key = ?1",
            params![source_key],
            |row| row.get(0),
        )
        .expect("total query")
}

#[test]
fn reconciles_daily_candidate_into_facts() {
    let (_directory, store) = migrated_store();
    let (source_id, import_run_id) = setup_import(&store, "refresh-1");

    let summary = store
        .reconcile_daily(request(
            source_id,
            import_run_id,
            vec![candidate(
                "claude-code:daily:v1:UTC:2026-06-13",
                100,
                "claude-sonnet-4",
            )],
        ))
        .expect("reconcile daily");

    assert_eq!(summary.upserted_days(), 1);
    assert_eq!(count(&store, "daily_usage"), 1);
    assert_eq!(count(&store, "daily_model_usage"), 1);
    assert_eq!(
        daily_total(&store, "claude-code:daily:v1:UTC:2026-06-13"),
        100
    );
}

#[test]
fn repeated_reconciliation_is_idempotent() {
    let (_directory, store) = migrated_store();
    let (source_id, import_run_id) = setup_import(&store, "refresh-1");
    let key = "claude-code:daily:v1:UTC:2026-06-13";

    for _ in 0..2 {
        store
            .reconcile_daily(request(
                source_id,
                import_run_id,
                vec![candidate(key, 100, "claude-sonnet-4")],
            ))
            .expect("reconcile daily");
    }

    assert_eq!(count(&store, "daily_usage"), 1);
    assert_eq!(count(&store, "daily_model_usage"), 1);
    assert_eq!(daily_total(&store, key), 100);
}

#[test]
fn changed_totals_replace_previous_values() {
    let (_directory, store) = migrated_store();
    let (source_id, import_run_id) = setup_import(&store, "refresh-1");
    let key = "claude-code:daily:v1:UTC:2026-06-13";

    store
        .reconcile_daily(request(
            source_id,
            import_run_id,
            vec![candidate(key, 100, "claude-sonnet-4")],
        ))
        .expect("first reconcile");
    store
        .reconcile_daily(request(
            source_id,
            import_run_id,
            vec![candidate(key, 250, "claude-sonnet-4")],
        ))
        .expect("second reconcile");

    assert_eq!(count(&store, "daily_usage"), 1);
    assert_eq!(daily_total(&store, key), 250);
}

#[test]
fn model_breakdowns_are_replaced_per_day() {
    let (_directory, store) = migrated_store();
    let (source_id, import_run_id) = setup_import(&store, "refresh-1");
    let key = "claude-code:daily:v1:UTC:2026-06-13";

    store
        .reconcile_daily(request(
            source_id,
            import_run_id,
            vec![candidate(key, 100, "model-a")],
        ))
        .expect("first reconcile");
    store
        .reconcile_daily(request(
            source_id,
            import_run_id,
            vec![candidate(key, 100, "model-b")],
        ))
        .expect("second reconcile");

    assert_eq!(count(&store, "daily_model_usage"), 1);

    let database = store.database.lock().expect("lock store");
    let referenced_model: String = database
        .connection()
        .query_row(
            "SELECT sm.raw_model_id
            FROM daily_model_usage dmu
            JOIN source_models sm ON sm.id = dmu.model_id
            JOIN daily_usage du ON du.id = dmu.daily_usage_id
            WHERE du.source_key = ?1",
            params![key],
            |row| row.get(0),
        )
        .expect("referenced model query");

    assert_eq!(referenced_model, "model-b");
}

#[test]
fn unknown_token_categories_persist_as_null() {
    let (_directory, store) = migrated_store();
    let (source_id, import_run_id) = setup_import(&store, "refresh-1");
    let key = "claude-code:daily:v1:UTC:2026-06-13";

    let mut partial = candidate(key, 100, "claude-sonnet-4");
    partial.tokens =
        TokenUsage::new(Some(100), Some(0), Some(0), None, 100).expect("partial tokens");

    store
        .reconcile_daily(request(source_id, import_run_id, vec![partial]))
        .expect("reconcile partial tokens");

    let database = store.database.lock().expect("lock store");
    let cache_read: Option<i64> = database
        .connection()
        .query_row(
            "SELECT cache_read_tokens FROM daily_usage WHERE source_key = ?1",
            params![key],
            |row| row.get(0),
        )
        .expect("cache read query");

    assert_eq!(cache_read, None);
}

#[test]
fn empty_reconciliation_preserves_existing_data() {
    let (_directory, store) = migrated_store();
    let (source_id, import_run_id) = setup_import(&store, "refresh-1");
    let key = "claude-code:daily:v1:UTC:2026-06-13";

    store
        .reconcile_daily(request(
            source_id,
            import_run_id,
            vec![candidate(key, 100, "claude-sonnet-4")],
        ))
        .expect("reconcile");

    let summary = store
        .reconcile_daily(request(source_id, import_run_id, Vec::new()))
        .expect("empty reconcile");

    assert_eq!(summary.upserted_days(), 0);
    assert_eq!(count(&store, "daily_usage"), 1);
}

#[test]
fn failed_write_rolls_back_without_partial_state() {
    let (_directory, store) = migrated_store();
    let source_id = store
        .resolve_source(SourceKey::ClaudeCode, 100)
        .expect("resolve source");

    let error = store
        .reconcile_daily(request(
            source_id,
            ImportRunId::new(999),
            vec![candidate(
                "claude-code:daily:v1:UTC:2026-06-13",
                100,
                "claude-sonnet-4",
            )],
        ))
        .expect_err("missing import run breaks the foreign key");

    assert_eq!(error, UsageStoreError::Backend);
    assert_eq!(count(&store, "daily_usage"), 0);
    assert_eq!(count(&store, "daily_model_usage"), 0);
}

#[test]
fn reconciled_usage_survives_database_reopen() {
    let directory = tempfile::TempDir::new().expect("create temporary directory");
    let database_path = directory.path().join("burnly.sqlite3");
    let key = "claude-code:daily:v1:UTC:2026-06-13";

    {
        let mut database = Database::open(&database_path).expect("open database");
        database.migrate_to_latest().expect("migrate database");
        let store = SqliteReconciliationStore::new(database);
        let (source_id, import_run_id) = setup_import(&store, "refresh-1");
        store
            .reconcile_daily(request(
                source_id,
                import_run_id,
                vec![candidate(key, 100, "claude-sonnet-4")],
            ))
            .expect("reconcile");
    }

    let reopened = Database::open(&database_path).expect("reopen database");
    let total: i64 = reopened
        .connection()
        .query_row(
            "SELECT total_tokens FROM daily_usage WHERE source_key = ?1",
            params![key],
            |row| row.get(0),
        )
        .expect("total query after reopen");

    assert_eq!(total, 100);
}

#[test]
fn absent_day_advances_active_to_missing_then_removed() {
    let (_directory, store) = migrated_store();
    let (source_id, refresh_run_id) = source_and_refresh(&store);
    let present = "claude-code:daily:v1:UTC:2026-06-12";
    let absent = "claude-code:daily:v1:UTC:2026-06-13";

    let first = next_import(&store, source_id, refresh_run_id);
    store
        .reconcile_daily(request(
            source_id,
            first,
            vec![
                candidate(present, 100, "claude-sonnet-4"),
                candidate(absent, 100, "claude-sonnet-4"),
            ],
        ))
        .expect("first import");
    assert_eq!(record_state(&store, absent), ("active".to_owned(), 0));

    let second = next_import(&store, source_id, refresh_run_id);
    store
        .reconcile_daily(request(
            source_id,
            second,
            vec![candidate(present, 100, "claude-sonnet-4")],
        ))
        .expect("second import");
    assert_eq!(record_state(&store, absent), ("missing".to_owned(), 1));

    let third = next_import(&store, source_id, refresh_run_id);
    store
        .reconcile_daily(request(
            source_id,
            third,
            vec![candidate(present, 100, "claude-sonnet-4")],
        ))
        .expect("third import");
    assert_eq!(record_state(&store, absent), ("removed".to_owned(), 2));
}

#[test]
fn reappearing_day_resets_to_active() {
    let (_directory, store) = migrated_store();
    let (source_id, refresh_run_id) = source_and_refresh(&store);
    let present = "claude-code:daily:v1:UTC:2026-06-12";
    let intermittent = "claude-code:daily:v1:UTC:2026-06-13";

    let first = next_import(&store, source_id, refresh_run_id);
    store
        .reconcile_daily(request(
            source_id,
            first,
            vec![
                candidate(present, 100, "claude-sonnet-4"),
                candidate(intermittent, 100, "claude-sonnet-4"),
            ],
        ))
        .expect("first import");

    let second = next_import(&store, source_id, refresh_run_id);
    store
        .reconcile_daily(request(
            source_id,
            second,
            vec![candidate(present, 100, "claude-sonnet-4")],
        ))
        .expect("second import");
    assert_eq!(
        record_state(&store, intermittent),
        ("missing".to_owned(), 1)
    );

    let third = next_import(&store, source_id, refresh_run_id);
    store
        .reconcile_daily(request(
            source_id,
            third,
            vec![
                candidate(present, 100, "claude-sonnet-4"),
                candidate(intermittent, 100, "claude-sonnet-4"),
            ],
        ))
        .expect("third import");
    assert_eq!(record_state(&store, intermittent), ("active".to_owned(), 0));
}

#[test]
fn partial_import_never_advances_absence() {
    let (_directory, store) = migrated_store();
    let (source_id, refresh_run_id) = source_and_refresh(&store);
    let present = "claude-code:daily:v1:UTC:2026-06-12";
    let absent = "claude-code:daily:v1:UTC:2026-06-13";

    let first = next_import(&store, source_id, refresh_run_id);
    store
        .reconcile_daily(request(
            source_id,
            first,
            vec![
                candidate(present, 100, "claude-sonnet-4"),
                candidate(absent, 100, "claude-sonnet-4"),
            ],
        ))
        .expect("first import");

    let second = next_import(&store, source_id, refresh_run_id);
    store
        .reconcile_daily(DailyReconciliationRequest::new(
            source_id,
            second,
            CollectionScope::Full,
            CollectionOutcome::Partial,
            120,
            vec![candidate(present, 100, "claude-sonnet-4")],
        ))
        .expect("partial import");

    assert_eq!(record_state(&store, absent), ("active".to_owned(), 0));
}

#[test]
fn incremental_import_never_advances_absence() {
    let (_directory, store) = migrated_store();
    let (source_id, refresh_run_id) = source_and_refresh(&store);
    let present = "claude-code:daily:v1:UTC:2026-06-12";
    let out_of_scope = "claude-code:daily:v1:UTC:2026-06-13";

    let first = next_import(&store, source_id, refresh_run_id);
    store
        .reconcile_daily(request(
            source_id,
            first,
            vec![
                candidate(present, 100, "claude-sonnet-4"),
                candidate(out_of_scope, 100, "claude-sonnet-4"),
            ],
        ))
        .expect("first import");

    let incremental = CollectionScope::incremental(
        NaiveDate::from_ymd_opt(2026, 6, 12).expect("start"),
        NaiveDate::from_ymd_opt(2026, 6, 12).expect("end"),
    )
    .expect("incremental scope");
    let second = next_import(&store, source_id, refresh_run_id);
    store
        .reconcile_daily(DailyReconciliationRequest::new(
            source_id,
            second,
            incremental,
            CollectionOutcome::Complete,
            120,
            vec![candidate(present, 100, "claude-sonnet-4")],
        ))
        .expect("incremental import");

    assert_eq!(record_state(&store, out_of_scope), ("active".to_owned(), 0));
}

#[test]
fn removed_days_are_excluded_from_active_queries() {
    let (_directory, store) = migrated_store();
    let (source_id, refresh_run_id) = source_and_refresh(&store);
    let present = "claude-code:daily:v1:UTC:2026-06-12";
    let absent = "claude-code:daily:v1:UTC:2026-06-13";

    let first = next_import(&store, source_id, refresh_run_id);
    store
        .reconcile_daily(request(
            source_id,
            first,
            vec![
                candidate(present, 100, "claude-sonnet-4"),
                candidate(absent, 100, "claude-sonnet-4"),
            ],
        ))
        .expect("first import");

    for _ in 0..2 {
        let import_run_id = next_import(&store, source_id, refresh_run_id);
        store
            .reconcile_daily(request(
                source_id,
                import_run_id,
                vec![candidate(present, 100, "claude-sonnet-4")],
            ))
            .expect("subsequent import");
    }

    assert_eq!(record_state(&store, absent), ("removed".to_owned(), 2));

    let database = store.database.lock().expect("lock store");
    let active_days: i64 = database
        .connection()
        .query_row(
            "SELECT count(*) FROM daily_usage WHERE record_state <> 'removed'",
            [],
            |row| row.get(0),
        )
        .expect("active count query");
    assert_eq!(active_days, 1);
}
