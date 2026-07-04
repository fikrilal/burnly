//! Daily usage reconciliation transaction and daily-specific SQL.

use rusqlite::{params, Transaction};

use crate::application::collection::{DailyUsageCandidate, ModelUsageCandidate};
use crate::application::ports::usage_store::UsageStoreError;
use crate::application::reconciliation::{
    DailyReconciliationRequest, DailyReconciliationSummary, ImportRunId, SourceId,
};
use crate::domain::identity::DAILY_IDENTITY_VERSION;

use super::identity::resolve_model;
use super::mapping::{
    daily_cost_columns, data_quality_value, model_cost_columns, should_evaluate_absence,
    token_columns,
};

pub(super) fn reconcile_daily_in_transaction(
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

/// Advances the absence state of daily rows not touched by the current import.
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
