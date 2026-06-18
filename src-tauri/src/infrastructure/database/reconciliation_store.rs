//! SQLite implementation of the reconciliation store.
//!
//! Owns the SQL for source resolution, refresh/import run records, and the
//! transactional reconciliation of canonical daily facts. Run rows store only
//! stable codes and bounded summaries; raw collector output, paths, and session
//! identifiers never reach these tables.

#![allow(
    dead_code,
    reason = "The SQLite reconciliation store is constructed by the Phase 4E refresh coordinator wiring"
)]

use std::sync::Mutex;

use rusqlite::{params, Connection, Transaction};

use crate::application::collection::{
    CollectionOutcome, CollectionProjection, CollectionScope, DailyUsageCandidate,
    ModelUsageCandidate, SessionUsageCandidate,
};
use crate::application::ports::run_store::{RunStore, RunStoreError};
use crate::application::ports::usage_store::{UsageStore, UsageStoreError};
use crate::application::reconciliation::{
    DailyReconciliationRequest, DailyReconciliationSummary, ImportOutcome, ImportRunCompletion,
    ImportRunId, ImportRunSpec, RefreshOutcome, RefreshRunCompletion, RefreshRunId, RefreshRunSpec,
    RefreshTrigger, RunError, SessionReconciliationRequest, SessionReconciliationSummary, SourceId,
};
use crate::domain::identity::DAILY_IDENTITY_VERSION;
use crate::domain::source::SourceKey;
use crate::domain::usage::{CostKind, DataQuality, TokenUsage, UsageCost, ValuedCostStatus};

use super::Database;
use crate::infrastructure::project_identity::ProjectPathIdentity;

pub(crate) struct SqliteReconciliationStore {
    database: Mutex<Database>,
}

impl SqliteReconciliationStore {
    pub(crate) fn new(database: Database) -> Self {
        Self {
            database: Mutex::new(database),
        }
    }

    fn with_connection<T>(
        &self,
        operation: impl FnOnce(&Connection) -> Result<T, RunStoreError>,
    ) -> Result<T, RunStoreError> {
        let database = self.database.lock().map_err(|_| RunStoreError::Backend)?;
        operation(database.connection())
    }
}

impl RunStore for SqliteReconciliationStore {
    fn resolve_source(&self, source: SourceKey, now_ms: i64) -> Result<SourceId, RunStoreError> {
        self.with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO sources (
                        source_key, display_name, enabled, detection_state,
                        created_at_ms, updated_at_ms
                    ) VALUES (?1, ?2, 1, 'unknown', ?3, ?3)
                    ON CONFLICT(source_key) DO NOTHING",
                    params![source.as_str(), source.as_str(), now_ms],
                )
                .map_err(|_| RunStoreError::Backend)?;

            let id: i64 = connection
                .query_row(
                    "SELECT id FROM sources WHERE source_key = ?1",
                    params![source.as_str()],
                    |row| row.get(0),
                )
                .map_err(|_| RunStoreError::Backend)?;

            Ok(SourceId::new(id))
        })
    }

    fn begin_refresh_run(
        &self,
        spec: RefreshRunSpec,
        now_ms: i64,
    ) -> Result<RefreshRunId, RunStoreError> {
        self.with_connection(|connection| {
            let result = connection.execute(
                "INSERT INTO refresh_runs (
                    job_key, trigger, status, started_at_ms,
                    requested_by_app_version, created_at_ms
                ) VALUES (?1, ?2, 'running', ?3, ?4, ?3)",
                params![
                    spec.job_key().as_str(),
                    refresh_trigger_value(spec.trigger()),
                    now_ms,
                    spec.requested_by_app_version(),
                ],
            );

            match result {
                Ok(_) => Ok(RefreshRunId::new(connection.last_insert_rowid())),
                Err(error) if is_unique_violation(&error) => Err(RunStoreError::DuplicateJobKey),
                Err(_) => Err(RunStoreError::Backend),
            }
        })
    }

    fn complete_refresh_run(
        &self,
        id: RefreshRunId,
        completion: RefreshRunCompletion,
    ) -> Result<(), RunStoreError> {
        let (error_code, error_summary) = error_fields(completion.error.as_ref());

        self.with_connection(|connection| {
            let changed = connection
                .execute(
                    "UPDATE refresh_runs
                    SET status = ?2, finished_at_ms = ?3, error_code = ?4, error_summary = ?5
                    WHERE id = ?1",
                    params![
                        id.value(),
                        refresh_outcome_value(completion.outcome),
                        completion.finished_at_ms,
                        error_code,
                        error_summary,
                    ],
                )
                .map_err(|_| RunStoreError::Backend)?;

            run_found(changed)
        })
    }

    fn begin_import_run(
        &self,
        spec: ImportRunSpec,
        started_at_ms: i64,
    ) -> Result<ImportRunId, RunStoreError> {
        let (scope_kind, scope_start, scope_end) = scope_fields(spec.scope());

        self.with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO import_runs (
                        refresh_run_id, source_id, collector_key, collector_version,
                        profile_version, projection, scope_kind, scope_start_date,
                        scope_end_date, aggregation_timezone, status,
                        records_seen, records_rejected, started_at_ms
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'running', 0, 0, ?11)",
                    params![
                        spec.refresh_run_id().value(),
                        spec.source_id().value(),
                        spec.collector_key(),
                        spec.collector_version(),
                        i64::from(spec.profile_version()),
                        projection_value(spec.projection()),
                        scope_kind,
                        scope_start,
                        scope_end,
                        spec.aggregation_timezone(),
                        started_at_ms,
                    ],
                )
                .map_err(|_| RunStoreError::Backend)?;

            Ok(ImportRunId::new(connection.last_insert_rowid()))
        })
    }

    fn complete_import_run(
        &self,
        id: ImportRunId,
        completion: ImportRunCompletion,
    ) -> Result<(), RunStoreError> {
        let (error_code, error_detail) = error_fields(completion.error.as_ref());

        self.with_connection(|connection| {
            let changed = connection
                .execute(
                    "UPDATE import_runs
                    SET status = ?2, records_seen = ?3, records_rejected = ?4,
                        finished_at_ms = ?5, error_code = ?6, error_detail = ?7
                    WHERE id = ?1",
                    params![
                        id.value(),
                        import_outcome_value(completion.outcome),
                        i64::from(completion.records_seen),
                        i64::from(completion.records_rejected),
                        completion.finished_at_ms,
                        error_code,
                        error_detail,
                    ],
                )
                .map_err(|_| RunStoreError::Backend)?;

            run_found(changed)
        })
    }
}

impl UsageStore for SqliteReconciliationStore {
    fn reconcile_daily(
        &self,
        request: DailyReconciliationRequest,
    ) -> Result<DailyReconciliationSummary, UsageStoreError> {
        let mut database = self.database.lock().map_err(|_| UsageStoreError::Backend)?;
        let transaction = database
            .connection_mut()
            .transaction()
            .map_err(|_| UsageStoreError::Backend)?;

        let summary = reconcile_daily_in_transaction(&transaction, &request)?;

        transaction.commit().map_err(|_| UsageStoreError::Backend)?;
        Ok(summary)
    }

    fn reconcile_session(
        &self,
        request: SessionReconciliationRequest,
    ) -> Result<SessionReconciliationSummary, UsageStoreError> {
        let mut database = self.database.lock().map_err(|_| UsageStoreError::Backend)?;
        let transaction = database
            .connection_mut()
            .transaction()
            .map_err(|_| UsageStoreError::Backend)?;

        let summary = reconcile_session_in_transaction(&transaction, &request)?;

        transaction.commit().map_err(|_| UsageStoreError::Backend)?;
        Ok(summary)
    }
}

fn reconcile_daily_in_transaction(
    transaction: &Transaction<'_>,
    request: &DailyReconciliationRequest,
) -> Result<DailyReconciliationSummary, UsageStoreError> {
    let source_id = request.source_id();
    let import_run_id = request.import_run_id();
    let observed_at_ms = request.observed_at_ms();

    let mut observed_source_keys = Vec::with_capacity(request.candidates().len());

    for candidate in request.candidates() {
        let daily_usage_id = upsert_daily_usage(
            transaction,
            source_id,
            import_run_id,
            observed_at_ms,
            candidate,
        )?;
        replace_model_breakdowns(
            transaction,
            source_id,
            import_run_id,
            observed_at_ms,
            daily_usage_id,
            candidate,
        )?;
        observed_source_keys.push(candidate.source_key.clone());
    }

    if should_evaluate_absence(request.scope(), request.outcome()) {
        advance_absences(transaction, source_id, import_run_id, observed_at_ms)?;
    }

    let upserted_days =
        u32::try_from(observed_source_keys.len()).map_err(|_| UsageStoreError::ValueOutOfRange)?;
    Ok(DailyReconciliationSummary::new(
        upserted_days,
        observed_source_keys,
    ))
}

fn reconcile_session_in_transaction(
    transaction: &Transaction<'_>,
    request: &SessionReconciliationRequest,
) -> Result<SessionReconciliationSummary, UsageStoreError> {
    let source_id = request.source_id();
    let import_run_id = request.import_run_id();
    let observed_at_ms = request.observed_at_ms();
    let retain_project_paths: bool = transaction
        .query_row(
            "SELECT store_project_paths FROM app_settings WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .map_err(|_| UsageStoreError::Backend)?;

    let mut observed_source_keys = Vec::with_capacity(request.candidates().len());

    for candidate in request.candidates() {
        let project_id = match &candidate.project_path {
            Some(path) => Some(resolve_project(
                transaction,
                source_id,
                observed_at_ms,
                path,
                retain_project_paths,
            )?),
            None => None,
        };

        let session_id = upsert_session(
            transaction,
            source_id,
            import_run_id,
            observed_at_ms,
            project_id,
            candidate,
        )?;
        replace_session_model_breakdowns(
            transaction,
            source_id,
            import_run_id,
            observed_at_ms,
            session_id,
            candidate,
        )?;
        observed_source_keys.push(candidate.source_key.clone());
    }

    if should_evaluate_absence(request.scope(), request.outcome()) {
        advance_absences_for_session(transaction, source_id, import_run_id, observed_at_ms)?;
    }

    let upserted_sessions =
        u32::try_from(observed_source_keys.len()).map_err(|_| UsageStoreError::ValueOutOfRange)?;
    Ok(SessionReconciliationSummary::new(
        upserted_sessions,
        observed_source_keys,
    ))
}

/// Absence advances only on a successful full-scope import. Partial imports may
/// be missing records for transient reasons, and incremental imports do not
/// describe the full set of days, so neither may remove records.
fn should_evaluate_absence(scope: &CollectionScope, outcome: CollectionOutcome) -> bool {
    matches!(scope, CollectionScope::Full) && !matches!(outcome, CollectionOutcome::Partial)
}

/// Advances the absence state of rows not touched by the current import.
///
/// Rows upserted in this transaction carry the current `latest_import_id`; any
/// active or missing row of this source still carrying an older import id was
/// absent from the result. Each absent row advances exactly one step per import:
/// `missing -> removed` is applied before `active -> missing` so a freshly missing
/// row is not removed in the same pass.
fn advance_absences(
    transaction: &Transaction<'_>,
    source_id: SourceId,
    import_run_id: ImportRunId,
    now_ms: i64,
) -> Result<(), UsageStoreError> {
    transaction
        .execute(
            "UPDATE daily_usage
            SET record_state = 'removed',
                absence_count = absence_count + 1,
                removed_at_ms = ?3
            WHERE source_id = ?1
                AND latest_import_id != ?2
                AND record_state = 'missing'",
            params![source_id.value(), import_run_id.value(), now_ms],
        )
        .map_err(|_| UsageStoreError::Backend)?;

    transaction
        .execute(
            "UPDATE daily_usage
            SET record_state = 'missing',
                absence_count = 1
            WHERE source_id = ?1
                AND latest_import_id != ?2
                AND record_state = 'active'",
            params![source_id.value(), import_run_id.value()],
        )
        .map_err(|_| UsageStoreError::Backend)?;

    Ok(())
}

fn advance_absences_for_session(
    transaction: &Transaction<'_>,
    source_id: SourceId,
    import_run_id: ImportRunId,
    now_ms: i64,
) -> Result<(), UsageStoreError> {
    transaction
        .execute(
            "UPDATE sessions
            SET record_state = 'removed',
                absence_count = absence_count + 1,
                removed_at_ms = ?3
            WHERE source_id = ?1
                AND latest_import_id != ?2
                AND record_state = 'missing'",
            params![source_id.value(), import_run_id.value(), now_ms],
        )
        .map_err(|_| UsageStoreError::Backend)?;

    transaction
        .execute(
            "UPDATE sessions
            SET record_state = 'missing',
                absence_count = 1
            WHERE source_id = ?1
                AND latest_import_id != ?2
                AND record_state = 'active'",
            params![source_id.value(), import_run_id.value()],
        )
        .map_err(|_| UsageStoreError::Backend)?;

    Ok(())
}

fn upsert_daily_usage(
    transaction: &Transaction<'_>,
    source_id: SourceId,
    import_run_id: ImportRunId,
    observed_at_ms: i64,
    candidate: &DailyUsageCandidate,
) -> Result<i64, UsageStoreError> {
    let tokens = token_columns(&candidate.tokens)?;
    let cost = daily_cost_columns(&candidate.cost)?;

    transaction
        .query_row(
            "INSERT INTO daily_usage (
                source_id, source_key, identity_version, usage_date, aggregation_timezone,
                input_tokens, output_tokens, cache_creation_tokens, cache_read_tokens,
                total_tokens, unclassified_tokens,
                cost_amount_micros, cost_currency, cost_kind, cost_status,
                data_quality, record_state, absence_count,
                first_seen_at_ms, last_seen_at_ms, latest_import_id
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5,
                ?6, ?7, ?8, ?9,
                ?10, ?11,
                ?12, ?13, ?14, ?15,
                ?16, 'active', 0,
                ?17, ?17, ?18
            )
            ON CONFLICT(source_id, source_key) DO UPDATE SET
                identity_version = excluded.identity_version,
                usage_date = excluded.usage_date,
                aggregation_timezone = excluded.aggregation_timezone,
                input_tokens = excluded.input_tokens,
                output_tokens = excluded.output_tokens,
                cache_creation_tokens = excluded.cache_creation_tokens,
                cache_read_tokens = excluded.cache_read_tokens,
                total_tokens = excluded.total_tokens,
                unclassified_tokens = excluded.unclassified_tokens,
                cost_amount_micros = excluded.cost_amount_micros,
                cost_currency = excluded.cost_currency,
                cost_kind = excluded.cost_kind,
                cost_status = excluded.cost_status,
                data_quality = excluded.data_quality,
                record_state = 'active',
                absence_count = 0,
                removed_at_ms = NULL,
                last_seen_at_ms = excluded.last_seen_at_ms,
                latest_import_id = excluded.latest_import_id
            RETURNING id",
            params![
                source_id.value(),
                candidate.source_key,
                i64::from(DAILY_IDENTITY_VERSION),
                candidate.usage_date.to_string(),
                candidate.aggregation_timezone,
                tokens.input,
                tokens.output,
                tokens.cache_creation,
                tokens.cache_read,
                tokens.total,
                tokens.unclassified,
                cost.amount_micros,
                cost.currency,
                cost.kind,
                cost.status,
                data_quality_value(candidate.provenance.data_quality),
                observed_at_ms,
                import_run_id.value(),
            ],
            |row| row.get(0),
        )
        .map_err(|_| UsageStoreError::Backend)
}

fn upsert_session(
    transaction: &Transaction<'_>,
    source_id: SourceId,
    import_run_id: ImportRunId,
    observed_at_ms: i64,
    project_id: Option<i64>,
    candidate: &SessionUsageCandidate,
) -> Result<i64, UsageStoreError> {
    let tokens = token_columns(&candidate.tokens)?;
    let cost = daily_cost_columns(&candidate.cost)?;

    transaction
        .query_row(
            "INSERT INTO sessions (
                source_id, source_key, identity_version, source_session_id, project_id,
                first_activity_at_ms, last_activity_at_ms,
                input_tokens, output_tokens, cache_creation_tokens, cache_read_tokens,
                total_tokens, unclassified_tokens,
                cost_amount_micros, cost_currency, cost_kind, cost_status,
                data_quality, record_state, absence_count,
                first_seen_at_ms, last_seen_at_ms, latest_import_id
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5,
                ?6, ?7,
                ?8, ?9, ?10, ?11,
                ?12, ?13,
                ?14, ?15, ?16, ?17,
                ?18, 'active', 0,
                ?19, ?19, ?20
            )
            ON CONFLICT(source_id, source_session_id) DO UPDATE SET
                identity_version = excluded.identity_version,
                source_key = excluded.source_key,
                project_id = excluded.project_id,
                first_activity_at_ms = excluded.first_activity_at_ms,
                last_activity_at_ms = excluded.last_activity_at_ms,
                input_tokens = excluded.input_tokens,
                output_tokens = excluded.output_tokens,
                cache_creation_tokens = excluded.cache_creation_tokens,
                cache_read_tokens = excluded.cache_read_tokens,
                total_tokens = excluded.total_tokens,
                unclassified_tokens = excluded.unclassified_tokens,
                cost_amount_micros = excluded.cost_amount_micros,
                cost_currency = excluded.cost_currency,
                cost_kind = excluded.cost_kind,
                cost_status = excluded.cost_status,
                data_quality = excluded.data_quality,
                record_state = 'active',
                absence_count = 0,
                removed_at_ms = NULL,
                last_seen_at_ms = excluded.last_seen_at_ms,
                latest_import_id = excluded.latest_import_id
            RETURNING id",
            params![
                source_id.value(),
                candidate.source_key,
                i64::from(DAILY_IDENTITY_VERSION),
                candidate.source_session_id,
                project_id,
                candidate.first_activity_at.map(|t| t.timestamp_millis()),
                candidate.last_activity_at.map(|t| t.timestamp_millis()),
                tokens.input,
                tokens.output,
                tokens.cache_creation,
                tokens.cache_read,
                tokens.total,
                tokens.unclassified,
                cost.amount_micros,
                cost.currency,
                cost.kind,
                cost.status,
                data_quality_value(candidate.provenance.data_quality),
                observed_at_ms,
                import_run_id.value(),
            ],
            |row| row.get(0),
        )
        .map_err(|_| UsageStoreError::Backend)
}

fn replace_model_breakdowns(
    transaction: &Transaction<'_>,
    source_id: SourceId,
    import_run_id: ImportRunId,
    observed_at_ms: i64,
    daily_usage_id: i64,
    candidate: &DailyUsageCandidate,
) -> Result<(), UsageStoreError> {
    transaction
        .execute(
            "DELETE FROM daily_model_usage WHERE daily_usage_id = ?1",
            params![daily_usage_id],
        )
        .map_err(|_| UsageStoreError::Backend)?;

    for model in &candidate.model_breakdowns {
        let model_id = resolve_model(transaction, source_id, observed_at_ms, model)?;
        insert_daily_model_usage(
            transaction,
            source_id,
            import_run_id,
            daily_usage_id,
            model_id,
            model,
        )?;
    }

    Ok(())
}

fn replace_session_model_breakdowns(
    transaction: &Transaction<'_>,
    source_id: SourceId,
    import_run_id: ImportRunId,
    observed_at_ms: i64,
    session_id: i64,
    candidate: &SessionUsageCandidate,
) -> Result<(), UsageStoreError> {
    transaction
        .execute(
            "DELETE FROM session_model_usage WHERE session_id = ?1",
            params![session_id],
        )
        .map_err(|_| UsageStoreError::Backend)?;

    for model in &candidate.model_breakdowns {
        let model_id = resolve_model(transaction, source_id, observed_at_ms, model)?;
        insert_session_model_usage(
            transaction,
            source_id,
            import_run_id,
            session_id,
            model_id,
            model,
        )?;
    }

    Ok(())
}

fn resolve_model(
    transaction: &Transaction<'_>,
    source_id: SourceId,
    observed_at_ms: i64,
    model: &ModelUsageCandidate,
) -> Result<i64, UsageStoreError> {
    transaction
        .query_row(
            "INSERT INTO source_models (source_id, raw_model_id, first_seen_at_ms, last_seen_at_ms)
            VALUES (?1, ?2, ?3, ?3)
            ON CONFLICT(source_id, raw_model_id)
                DO UPDATE SET last_seen_at_ms = excluded.last_seen_at_ms
            RETURNING id",
            params![source_id.value(), model.raw_model_id, observed_at_ms],
            |row| row.get(0),
        )
        .map_err(|_| UsageStoreError::Backend)
}

fn resolve_project(
    transaction: &Transaction<'_>,
    source_id: SourceId,
    observed_at_ms: i64,
    project_path: &str,
    retain_path: bool,
) -> Result<i64, UsageStoreError> {
    let identity = ProjectPathIdentity::from_path(project_path);
    let raw_path = retain_path.then_some(project_path);
    transaction
        .query_row(
            "INSERT INTO projects (
                source_id, identity_key, identity_kind, raw_path,
                path_fingerprint, first_seen_at_ms, last_seen_at_ms
            )
            VALUES (?1, ?2, 'path', ?3, ?4, ?5, ?5)
            ON CONFLICT(source_id, identity_key)
                DO UPDATE SET
                    raw_path = excluded.raw_path,
                    path_fingerprint = excluded.path_fingerprint,
                    last_seen_at_ms = excluded.last_seen_at_ms
            RETURNING id",
            params![
                source_id.value(),
                identity.key(),
                raw_path,
                identity.fingerprint().as_slice(),
                observed_at_ms
            ],
            |row| row.get(0),
        )
        .map_err(|_| UsageStoreError::Backend)
}

fn insert_daily_model_usage(
    transaction: &Transaction<'_>,
    source_id: SourceId,
    import_run_id: ImportRunId,
    daily_usage_id: i64,
    model_id: i64,
    model: &ModelUsageCandidate,
) -> Result<(), UsageStoreError> {
    let tokens = token_columns(&model.tokens)?;
    let cost = model_cost_columns(&model.cost)?;

    transaction
        .execute(
            "INSERT INTO daily_model_usage (
                daily_usage_id, source_id, model_id,
                input_tokens, output_tokens, cache_creation_tokens, cache_read_tokens,
                total_tokens, unclassified_tokens,
                cost_amount_micros, cost_currency, cost_status, latest_import_id
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                daily_usage_id,
                source_id.value(),
                model_id,
                tokens.input,
                tokens.output,
                tokens.cache_creation,
                tokens.cache_read,
                tokens.total,
                tokens.unclassified,
                cost.amount_micros,
                cost.currency,
                cost.status,
                import_run_id.value(),
            ],
        )
        .map_err(|_| UsageStoreError::Backend)?;

    Ok(())
}

fn insert_session_model_usage(
    transaction: &Transaction<'_>,
    source_id: SourceId,
    import_run_id: ImportRunId,
    session_id: i64,
    model_id: i64,
    model: &ModelUsageCandidate,
) -> Result<(), UsageStoreError> {
    let tokens = token_columns(&model.tokens)?;
    let cost = model_cost_columns(&model.cost)?;

    transaction
        .execute(
            "INSERT INTO session_model_usage (
                session_id, source_id, model_id,
                input_tokens, output_tokens, cache_creation_tokens, cache_read_tokens,
                total_tokens, unclassified_tokens,
                cost_amount_micros, cost_currency, cost_status, latest_import_id
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                session_id,
                source_id.value(),
                model_id,
                tokens.input,
                tokens.output,
                tokens.cache_creation,
                tokens.cache_read,
                tokens.total,
                tokens.unclassified,
                cost.amount_micros,
                cost.currency,
                cost.status,
                import_run_id.value(),
            ],
        )
        .map_err(|_| UsageStoreError::Backend)?;

    Ok(())
}

struct TokenColumns {
    input: Option<i64>,
    output: Option<i64>,
    cache_creation: Option<i64>,
    cache_read: Option<i64>,
    total: i64,
    unclassified: Option<i64>,
}

fn token_columns(tokens: &TokenUsage) -> Result<TokenColumns, UsageStoreError> {
    Ok(TokenColumns {
        input: optional_token(tokens.input_tokens())?,
        output: optional_token(tokens.output_tokens())?,
        cache_creation: optional_token(tokens.cache_creation_tokens())?,
        cache_read: optional_token(tokens.cache_read_tokens())?,
        total: token_value(tokens.total_tokens())?,
        unclassified: optional_token(tokens.unclassified_tokens())?,
    })
}

fn token_value(value: u64) -> Result<i64, UsageStoreError> {
    i64::try_from(value).map_err(|_| UsageStoreError::ValueOutOfRange)
}

fn optional_token(value: Option<u64>) -> Result<Option<i64>, UsageStoreError> {
    value.map(token_value).transpose()
}

struct DailyCostColumns {
    amount_micros: Option<i64>,
    currency: Option<String>,
    kind: &'static str,
    status: &'static str,
}

fn daily_cost_columns(cost: &UsageCost) -> Result<DailyCostColumns, UsageStoreError> {
    Ok(match cost {
        UsageCost::Valued {
            amount_micros,
            currency,
            kind,
            status,
        } => DailyCostColumns {
            amount_micros: Some(token_value(*amount_micros)?),
            currency: Some(currency.as_str().to_owned()),
            kind: cost_kind_value(*kind),
            status: valued_status_value(*status),
        },
        UsageCost::NotApplicable { kind } => DailyCostColumns {
            amount_micros: None,
            currency: None,
            kind: cost_kind_value(*kind),
            status: "not_applicable",
        },
        UsageCost::Unavailable { kind } => DailyCostColumns {
            amount_micros: None,
            currency: None,
            kind: cost_kind_value(*kind),
            status: "unavailable",
        },
    })
}

struct ModelCostColumns {
    amount_micros: Option<i64>,
    currency: Option<String>,
    status: &'static str,
}

fn model_cost_columns(cost: &UsageCost) -> Result<ModelCostColumns, UsageStoreError> {
    Ok(match cost {
        UsageCost::Valued {
            amount_micros,
            currency,
            ..
        } => ModelCostColumns {
            amount_micros: Some(token_value(*amount_micros)?),
            currency: Some(currency.as_str().to_owned()),
            status: "estimated",
        },
        UsageCost::NotApplicable { .. } | UsageCost::Unavailable { .. } => ModelCostColumns {
            amount_micros: None,
            currency: None,
            status: "unavailable",
        },
    })
}

const fn cost_kind_value(kind: CostKind) -> &'static str {
    match kind {
        CostKind::SourceReported => "source_reported",
        CostKind::CollectorCalculated => "collector_calculated",
        CostKind::CollectorMixed => "collector_mixed",
        CostKind::BurnlyCalculated => "burnly_calculated",
        CostKind::Unknown => "unknown",
    }
}

const fn valued_status_value(status: ValuedCostStatus) -> &'static str {
    match status {
        ValuedCostStatus::Available => "available",
        ValuedCostStatus::Estimated => "estimated",
    }
}

const fn data_quality_value(quality: DataQuality) -> &'static str {
    match quality {
        DataQuality::Complete => "complete",
        DataQuality::Partial => "partial",
    }
}

fn run_found(changed: usize) -> Result<(), RunStoreError> {
    if changed == 0 {
        Err(RunStoreError::RunNotFound)
    } else {
        Ok(())
    }
}

fn error_fields(error: Option<&RunError>) -> (Option<&str>, Option<&str>) {
    match error {
        Some(error) => (Some(error.code()), Some(error.summary())),
        None => (None, None),
    }
}

fn scope_fields(scope: &CollectionScope) -> (&'static str, Option<String>, Option<String>) {
    match scope {
        CollectionScope::Full => ("full", None, None),
        CollectionScope::Incremental(incremental) => (
            "incremental",
            Some(incremental.start_date().to_string()),
            Some(incremental.end_date().to_string()),
        ),
    }
}

const fn refresh_trigger_value(trigger: RefreshTrigger) -> &'static str {
    match trigger {
        RefreshTrigger::Launch => "launch",
        RefreshTrigger::Manual => "manual",
        RefreshTrigger::Scheduled => "scheduled",
        RefreshTrigger::FileChange => "file_change",
        RefreshTrigger::Resume => "resume",
        RefreshTrigger::Reconcile => "reconcile",
    }
}

const fn refresh_outcome_value(outcome: RefreshOutcome) -> &'static str {
    match outcome {
        RefreshOutcome::Succeeded => "succeeded",
        RefreshOutcome::Partial => "partial",
        RefreshOutcome::Failed => "failed",
        RefreshOutcome::Cancelled => "cancelled",
    }
}

const fn import_outcome_value(outcome: ImportOutcome) -> &'static str {
    match outcome {
        ImportOutcome::Succeeded => "succeeded",
        ImportOutcome::Partial => "partial",
        ImportOutcome::Failed => "failed",
        ImportOutcome::Cancelled => "cancelled",
    }
}

const fn projection_value(projection: CollectionProjection) -> &'static str {
    match projection {
        CollectionProjection::Daily => "daily",
        CollectionProjection::Session => "session",
    }
}

fn is_unique_violation(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(failure, _)
            if failure.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE
    )
}

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, TimeZone, Utc};

    use super::*;
    use crate::application::collection::{CandidateProvenance, CollectionId, CollectorKey};
    use crate::application::reconciliation::{ImportCollector, JobKey};
    use crate::domain::usage::CurrencyCode;

    fn migrated_store() -> (tempfile::TempDir, SqliteReconciliationStore) {
        let directory = tempfile::TempDir::new().expect("create temporary directory");
        let database_path = directory.path().join("burnly.sqlite3");
        let mut database = Database::open(&database_path).expect("open database");
        database.migrate_to_latest().expect("migrate database");

        (directory, SqliteReconciliationStore::new(database))
    }

    fn refresh_spec(job_key: &str) -> RefreshRunSpec {
        RefreshRunSpec::new(
            JobKey::new(job_key).expect("job key"),
            RefreshTrigger::Manual,
            "0.1.0",
        )
        .expect("refresh spec")
    }

    fn daily_import_spec(refresh_run_id: RefreshRunId, source_id: SourceId) -> ImportRunSpec {
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

    fn session_import_spec(refresh_run_id: RefreshRunId, source_id: SourceId) -> ImportRunSpec {
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

    fn setup_import(store: &SqliteReconciliationStore, job_key: &str) -> (SourceId, ImportRunId) {
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

    fn candidate(source_key: &str, total: u64, model: &str) -> DailyUsageCandidate {
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

    fn session_candidate(project_path: &str) -> SessionUsageCandidate {
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

    fn reconcile_session_with_policy(retain_paths: bool) -> SqliteReconciliationStore {
        let directory = tempfile::TempDir::new().expect("create temporary directory");
        let database_path = directory.keep().join("burnly.sqlite3");
        let mut database = Database::open(&database_path).expect("open database");
        database.migrate_to_latest().expect("migrate database");
        database.ensure_app_settings("UTC", 100).expect("settings");
        database
            .connection()
            .execute(
                "UPDATE app_settings SET store_project_paths = ?1",
                [retain_paths],
            )
            .expect("set privacy policy");
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

    #[test]
    fn disabled_retention_persists_only_non_reversible_project_identity() {
        let store = reconcile_session_with_policy(false);
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

    #[test]
    fn enabled_retention_keeps_raw_path_separate_from_project_identity() {
        let store = reconcile_session_with_policy(true);
        let database = store.database.lock().expect("store lock");
        let (identity_key, raw_path): (String, Option<String>) = database
            .connection()
            .query_row("SELECT identity_key, raw_path FROM projects", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .expect("read project");

        assert!(ProjectPathIdentity::is_key(&identity_key));
        assert_eq!(raw_path.as_deref(), Some("/home/dante/secret-project"));
    }

    fn request(
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

    fn source_and_refresh(store: &SqliteReconciliationStore) -> (SourceId, RefreshRunId) {
        let source_id = store
            .resolve_source(SourceKey::ClaudeCode, 100)
            .expect("resolve source");
        let refresh_run_id = store
            .begin_refresh_run(refresh_spec("refresh-1"), 100)
            .expect("begin refresh run");

        (source_id, refresh_run_id)
    }

    fn next_import(
        store: &SqliteReconciliationStore,
        source_id: SourceId,
        refresh_run_id: RefreshRunId,
    ) -> ImportRunId {
        store
            .begin_import_run(daily_import_spec(refresh_run_id, source_id), 110)
            .expect("begin import run")
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
}
