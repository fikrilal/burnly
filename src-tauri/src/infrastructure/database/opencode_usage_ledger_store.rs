#![allow(
    dead_code,
    reason = "chunk 2 establishes the ledger store wired into the native adapter in chunk 4"
)]

use std::collections::BTreeMap;
use std::sync::Mutex;

use rusqlite::{params, Connection, OptionalExtension, Transaction};

use crate::application::ports::opencode_usage_ledger::{
    OpenCodeDataQuality, OpenCodeExactOrigin, OpenCodeExactUsage, OpenCodeLedgerOrigin,
    OpenCodeLedgerReconcileResult, OpenCodeLedgerRecord, OpenCodeReconciliationState,
    OpenCodeRecoveryDisposition, OpenCodeSessionCheckpoint, OpenCodeSessionLedgerSnapshot,
    OpenCodeTimestampOrigin, OpenCodeTokenVector, OpenCodeUsageLedger, OpenCodeUsageLedgerError,
};

use super::Database;

const UNATTRIBUTED_MODEL: &str = "OpenCode unattributed";

pub(crate) struct SqliteOpenCodeUsageLedgerStore {
    database: Mutex<Database>,
}

impl SqliteOpenCodeUsageLedgerStore {
    pub(crate) fn new(database: Database) -> Self {
        Self {
            database: Mutex::new(database),
        }
    }
}

impl OpenCodeUsageLedger for SqliteOpenCodeUsageLedgerStore {
    fn reconcile_session(
        &self,
        snapshot: &OpenCodeSessionLedgerSnapshot,
    ) -> Result<OpenCodeLedgerReconcileResult, OpenCodeUsageLedgerError> {
        let exact_usage = validate_and_deduplicate(snapshot)?;
        let mut database = self
            .database
            .lock()
            .map_err(|_| OpenCodeUsageLedgerError::Storage)?;
        let transaction = database
            .connection_mut()
            .transaction()
            .map_err(|_| OpenCodeUsageLedgerError::Storage)?;

        let previous_checkpoint = select_checkpoint(&transaction, &snapshot.session_id)?;
        ensure_checkpoint_parent(&transaction, snapshot, previous_checkpoint.as_ref())?;
        let existing_records = select_records(&transaction, &snapshot.session_id)?;
        let existing_totals = totals(&existing_records)?;
        let existing_cost_tolerance = cost_rounding_tolerance(&existing_records)?;
        let regressed = previous_checkpoint.as_ref().is_some_and(|checkpoint| {
            snapshot
                .cumulative_tokens
                .checked_sub(checkpoint.accepted_tokens)
                .is_none()
                || cost_is_lower_with_tolerance(
                    snapshot.cumulative_cost_micros,
                    checkpoint.accepted_cost_micros,
                    existing_cost_tolerance,
                )
        }) || snapshot
            .cumulative_tokens
            .checked_sub(existing_totals.tokens)
            .is_none()
            || cost_is_lower_with_tolerance(
                snapshot.cumulative_cost_micros,
                existing_totals.cost,
                existing_cost_tolerance,
            );

        let mut counters = ReconcileCounters::default();
        let (records, next_recovery_sequence) = if regressed {
            counters.counter_regressions = 1;
            rebuild_session(
                &transaction,
                snapshot,
                &exact_usage,
                previous_checkpoint
                    .as_ref()
                    .map_or(0, |checkpoint| checkpoint.next_recovery_sequence),
                &mut counters,
            )?
        } else {
            reconcile_existing(
                &transaction,
                snapshot,
                &exact_usage,
                existing_records,
                previous_checkpoint
                    .as_ref()
                    .map_or(0, |checkpoint| checkpoint.next_recovery_sequence),
                &mut counters,
            )?
        };

        let final_totals = totals(&records)?;
        let reconciliation_state = reconciliation_state(snapshot, &records);
        let checkpoint = persist_checkpoint(
            &transaction,
            snapshot,
            previous_checkpoint.as_ref(),
            final_totals,
            reconciliation_state,
            next_recovery_sequence,
        )?;
        transaction
            .commit()
            .map_err(|_| OpenCodeUsageLedgerError::Storage)?;

        Ok(OpenCodeLedgerReconcileResult {
            records,
            checkpoint,
            exact_records_accepted: counters.exact_records_accepted,
            recovery_segments_created: counters.recovery_segments_created,
            late_exact_reclassified: counters.late_exact_reclassified,
            late_exact_ignored: counters.late_exact_ignored,
            counter_regressions: counters.counter_regressions,
        })
    }

    fn read_checkpoint(
        &self,
        session_id: &str,
    ) -> Result<Option<OpenCodeSessionCheckpoint>, OpenCodeUsageLedgerError> {
        validate_identity(session_id)?;
        let database = self
            .database
            .lock()
            .map_err(|_| OpenCodeUsageLedgerError::Storage)?;
        select_checkpoint(database.connection(), session_id)
    }

    fn read_session_records(
        &self,
        session_id: &str,
    ) -> Result<Vec<OpenCodeLedgerRecord>, OpenCodeUsageLedgerError> {
        validate_identity(session_id)?;
        let database = self
            .database
            .lock()
            .map_err(|_| OpenCodeUsageLedgerError::Storage)?;
        select_records(database.connection(), session_id)
    }
}

#[derive(Default)]
struct ReconcileCounters {
    exact_records_accepted: u32,
    recovery_segments_created: u32,
    late_exact_reclassified: u32,
    late_exact_ignored: u32,
    counter_regressions: u32,
}

#[derive(Clone, Copy)]
struct LedgerTotals {
    tokens: OpenCodeTokenVector,
    cost: Option<u64>,
}

fn validate_and_deduplicate(
    snapshot: &OpenCodeSessionLedgerSnapshot,
) -> Result<Vec<OpenCodeExactUsage>, OpenCodeUsageLedgerError> {
    validate_identity(&snapshot.session_id)?;
    validate_timestamp(snapshot.source_updated_at_ms)?;
    validate_timestamp(snapshot.observed_at_ms)?;
    if let Some(timestamp) = snapshot.recovery_activity_at_ms {
        validate_timestamp(timestamp)?;
    }
    validate_vector(snapshot.cumulative_tokens)?;
    validate_optional_u64(snapshot.cumulative_cost_micros)?;

    let mut records = BTreeMap::<String, OpenCodeExactUsage>::new();
    for record in &snapshot.exact_usage {
        validate_identity(&record.message_id)?;
        validate_identity(&record.provider_id)?;
        validate_identity(&record.raw_model_id)?;
        validate_timestamp(record.activity_at_ms)?;
        validate_vector(record.tokens)?;
        validate_optional_u64(record.cost_micros)?;

        if let Some(existing) = records.get(&record.message_id) {
            if existing.tokens != record.tokens || existing.cost_micros != record.cost_micros {
                return Err(OpenCodeUsageLedgerError::IncompatibleSnapshot);
            }
            if record.origin > existing.origin {
                records.insert(record.message_id.clone(), record.clone());
            }
        } else {
            records.insert(record.message_id.clone(), record.clone());
        }
    }
    Ok(records.into_values().collect())
}

fn ensure_checkpoint_parent(
    transaction: &Transaction<'_>,
    snapshot: &OpenCodeSessionLedgerSnapshot,
    previous: Option<&OpenCodeSessionCheckpoint>,
) -> Result<(), OpenCodeUsageLedgerError> {
    if previous.is_some() {
        return Ok(());
    }
    let zero = OpenCodeTokenVector::default();
    transaction
        .execute(
            "INSERT INTO opencode_session_checkpoint (
                session_id,
                accepted_input_tokens, accepted_output_tokens,
                accepted_reasoning_tokens, accepted_cache_read_tokens,
                accepted_cache_write_tokens, accepted_cost_micros,
                observed_input_tokens, observed_output_tokens,
                observed_reasoning_tokens, observed_cache_read_tokens,
                observed_cache_write_tokens, observed_cost_micros,
                source_updated_at_ms, reconciliation_state,
                next_recovery_sequence, first_observed_at_ms, last_reconciled_at_ms
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7, ?8, ?9, ?10, ?11, ?12,
                ?13, 'deferred_live_write', 0, ?14, ?14
             )",
            params![
                snapshot.session_id,
                sql_u64(zero.input)?,
                sql_u64(zero.output)?,
                sql_u64(zero.reasoning)?,
                sql_u64(zero.cache_read)?,
                sql_u64(zero.cache_write)?,
                sql_u64(snapshot.cumulative_tokens.input)?,
                sql_u64(snapshot.cumulative_tokens.output)?,
                sql_u64(snapshot.cumulative_tokens.reasoning)?,
                sql_u64(snapshot.cumulative_tokens.cache_read)?,
                sql_u64(snapshot.cumulative_tokens.cache_write)?,
                sql_optional_u64(snapshot.cumulative_cost_micros)?,
                snapshot.source_updated_at_ms,
                snapshot.observed_at_ms,
            ],
        )
        .map_err(|_| OpenCodeUsageLedgerError::Storage)?;
    Ok(())
}

fn rebuild_session(
    transaction: &Transaction<'_>,
    snapshot: &OpenCodeSessionLedgerSnapshot,
    exact_usage: &[OpenCodeExactUsage],
    next_recovery_sequence: u64,
    counters: &mut ReconcileCounters,
) -> Result<(Vec<OpenCodeLedgerRecord>, u64), OpenCodeUsageLedgerError> {
    let exact_totals = exact_totals(exact_usage)?;
    ensure_explained_by_cumulative(
        snapshot,
        exact_totals,
        exact_cost_rounding_tolerance(exact_usage)?,
    )?;
    transaction
        .execute(
            "DELETE FROM opencode_usage_ledger WHERE session_id = ?1",
            [&snapshot.session_id],
        )
        .map_err(|_| OpenCodeUsageLedgerError::Storage)?;

    let mut records = Vec::with_capacity(exact_usage.len() + 1);
    for exact in exact_usage {
        insert_exact(
            transaction,
            &snapshot.session_id,
            exact,
            snapshot.observed_at_ms,
        )?;
        records.push(exact_record(
            &snapshot.session_id,
            exact,
            snapshot.observed_at_ms,
        ));
        counters.exact_records_accepted = counters.exact_records_accepted.saturating_add(1);
    }
    append_recovery_if_ready(
        transaction,
        snapshot,
        &mut records,
        next_recovery_sequence,
        counters,
    )
}

fn reconcile_existing(
    transaction: &Transaction<'_>,
    snapshot: &OpenCodeSessionLedgerSnapshot,
    exact_usage: &[OpenCodeExactUsage],
    mut records: Vec<OpenCodeLedgerRecord>,
    next_recovery_sequence: u64,
    counters: &mut ReconcileCounters,
) -> Result<(Vec<OpenCodeLedgerRecord>, u64), OpenCodeUsageLedgerError> {
    for exact in exact_usage {
        if let Some(index) = records
            .iter()
            .position(|record| record.source_message_id.as_deref() == Some(&exact.message_id))
        {
            reconcile_known_exact(transaction, snapshot, exact, &mut records[index], counters)?;
            continue;
        }

        if source_message_belongs_to_other_session(transaction, snapshot, exact)? {
            return Err(OpenCodeUsageLedgerError::IncompatibleSnapshot);
        }

        let current = totals(&records)?;
        let candidate_tokens = current
            .tokens
            .checked_add(exact.tokens)
            .ok_or(OpenCodeUsageLedgerError::IncompatibleSnapshot)?;
        if snapshot
            .cumulative_tokens
            .checked_sub(candidate_tokens)
            .is_some()
        {
            insert_exact(
                transaction,
                &snapshot.session_id,
                exact,
                snapshot.observed_at_ms,
            )?;
            records.push(exact_record(
                &snapshot.session_id,
                exact,
                snapshot.observed_at_ms,
            ));
            counters.exact_records_accepted = counters.exact_records_accepted.saturating_add(1);
            continue;
        }

        let matches = records
            .iter()
            .enumerate()
            .filter(|(_, record)| {
                record.origin == OpenCodeLedgerOrigin::CumulativeRecovery
                    && record.tokens == exact.tokens
                    && record.cost_micros == exact.cost_micros
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        if matches.len() == 1 {
            let recovery = records.remove(matches[0]);
            delete_recovery(
                transaction,
                &snapshot.session_id,
                recovery.recovery_sequence,
            )?;
            insert_exact(
                transaction,
                &snapshot.session_id,
                exact,
                snapshot.observed_at_ms,
            )?;
            records.push(exact_record(
                &snapshot.session_id,
                exact,
                snapshot.observed_at_ms,
            ));
            counters.late_exact_reclassified = counters.late_exact_reclassified.saturating_add(1);
        } else if records
            .iter()
            .any(|record| record.origin == OpenCodeLedgerOrigin::CumulativeRecovery)
        {
            counters.late_exact_ignored = counters.late_exact_ignored.saturating_add(1);
        } else {
            return Err(OpenCodeUsageLedgerError::IncompatibleSnapshot);
        }
    }

    ensure_explained_by_cumulative(
        snapshot,
        totals(&records)?,
        cost_rounding_tolerance(&records)?,
    )?;
    append_recovery_if_ready(
        transaction,
        snapshot,
        &mut records,
        next_recovery_sequence,
        counters,
    )
}

fn reconcile_known_exact(
    transaction: &Transaction<'_>,
    snapshot: &OpenCodeSessionLedgerSnapshot,
    exact: &OpenCodeExactUsage,
    existing: &mut OpenCodeLedgerRecord,
    counters: &mut ReconcileCounters,
) -> Result<(), OpenCodeUsageLedgerError> {
    if existing.tokens != exact.tokens || existing.cost_micros != exact.cost_micros {
        return Err(OpenCodeUsageLedgerError::IncompatibleSnapshot);
    }
    let existing_origin = exact_origin(existing.origin)?;
    let last_seen_at_ms = existing.last_seen_at_ms.max(snapshot.observed_at_ms);
    if exact.origin >= existing_origin {
        transaction
            .execute(
                "UPDATE opencode_usage_ledger
                 SET activity_at_ms = ?2, provider_id = ?3, raw_model_id = ?4,
                     origin = ?5, last_seen_at_ms = ?6
                 WHERE source_message_id = ?1",
                params![
                    exact.message_id,
                    exact.activity_at_ms,
                    exact.provider_id,
                    exact.raw_model_id,
                    exact_origin_value(exact.origin),
                    last_seen_at_ms,
                ],
            )
            .map_err(|_| OpenCodeUsageLedgerError::Storage)?;
        existing.activity_at_ms = exact.activity_at_ms;
        existing.provider_id = Some(exact.provider_id.clone());
        existing.raw_model_id.clone_from(&exact.raw_model_id);
        existing.origin = ledger_origin(exact.origin);
        existing.last_seen_at_ms = last_seen_at_ms;
        if exact.origin > existing_origin {
            counters.exact_records_accepted = counters.exact_records_accepted.saturating_add(1);
        }
    } else {
        transaction
            .execute(
                "UPDATE opencode_usage_ledger SET last_seen_at_ms = ?2
                 WHERE source_message_id = ?1",
                params![exact.message_id, last_seen_at_ms],
            )
            .map_err(|_| OpenCodeUsageLedgerError::Storage)?;
        existing.last_seen_at_ms = last_seen_at_ms;
    }
    Ok(())
}

fn append_recovery_if_ready(
    transaction: &Transaction<'_>,
    snapshot: &OpenCodeSessionLedgerSnapshot,
    records: &mut Vec<OpenCodeLedgerRecord>,
    next_recovery_sequence: u64,
    counters: &mut ReconcileCounters,
) -> Result<(Vec<OpenCodeLedgerRecord>, u64), OpenCodeUsageLedgerError> {
    if snapshot.recovery_disposition == OpenCodeRecoveryDisposition::DeferredLiveWrite {
        return Ok((std::mem::take(records), next_recovery_sequence));
    }

    let current = totals(records)?;
    let remainder_tokens = snapshot
        .cumulative_tokens
        .checked_sub(current.tokens)
        .ok_or(OpenCodeUsageLedgerError::IncompatibleSnapshot)?;
    let remainder_cost = cost_remainder(
        snapshot.cumulative_cost_micros,
        current.cost,
        cost_rounding_tolerance(records)?,
    )?;
    if remainder_tokens.is_zero() && remainder_cost.unwrap_or(0) == 0 {
        return Ok((std::mem::take(records), next_recovery_sequence));
    }

    let sequence_after = next_recovery_sequence
        .checked_add(1)
        .ok_or(OpenCodeUsageLedgerError::IncompatibleSnapshot)?;
    validate_optional_u64(Some(next_recovery_sequence))?;
    let activity_at_ms = snapshot
        .recovery_activity_at_ms
        .unwrap_or(snapshot.observed_at_ms);
    let timestamp_origin = if snapshot.recovery_activity_at_ms.is_some() {
        OpenCodeTimestampOrigin::SourceLifecycle
    } else {
        OpenCodeTimestampOrigin::FirstSeen
    };
    insert_recovery(
        transaction,
        snapshot,
        next_recovery_sequence,
        activity_at_ms,
        timestamp_origin,
        remainder_tokens,
        remainder_cost,
    )?;
    records.push(OpenCodeLedgerRecord {
        source_message_id: None,
        recovery_sequence: Some(next_recovery_sequence),
        session_id: snapshot.session_id.clone(),
        activity_at_ms,
        timestamp_origin,
        provider_id: None,
        raw_model_id: UNATTRIBUTED_MODEL.to_owned(),
        tokens: remainder_tokens,
        cost_micros: remainder_cost,
        origin: OpenCodeLedgerOrigin::CumulativeRecovery,
        quality: OpenCodeDataQuality::Partial,
        first_seen_at_ms: snapshot.observed_at_ms,
        last_seen_at_ms: snapshot.observed_at_ms,
    });
    counters.recovery_segments_created = counters.recovery_segments_created.saturating_add(1);
    Ok((std::mem::take(records), sequence_after))
}

fn exact_totals(
    exact_usage: &[OpenCodeExactUsage],
) -> Result<LedgerTotals, OpenCodeUsageLedgerError> {
    let records = exact_usage
        .iter()
        .map(|exact| exact_record("validation", exact, 0))
        .collect::<Vec<_>>();
    totals(&records)
}

fn totals(records: &[OpenCodeLedgerRecord]) -> Result<LedgerTotals, OpenCodeUsageLedgerError> {
    let mut tokens = OpenCodeTokenVector::default();
    let mut cost = Some(0_u64);
    for record in records {
        tokens = tokens
            .checked_add(record.tokens)
            .ok_or(OpenCodeUsageLedgerError::IncompatibleSnapshot)?;
        cost = match (cost, record.cost_micros) {
            (Some(total), Some(value)) => Some(
                total
                    .checked_add(value)
                    .ok_or(OpenCodeUsageLedgerError::IncompatibleSnapshot)?,
            ),
            _ => None,
        };
    }
    Ok(LedgerTotals { tokens, cost })
}

fn ensure_explained_by_cumulative(
    snapshot: &OpenCodeSessionLedgerSnapshot,
    actual: LedgerTotals,
    cost_tolerance: u64,
) -> Result<(), OpenCodeUsageLedgerError> {
    if snapshot
        .cumulative_tokens
        .checked_sub(actual.tokens)
        .is_none()
        || cost_is_lower_with_tolerance(
            snapshot.cumulative_cost_micros,
            actual.cost,
            cost_tolerance,
        )
    {
        Err(OpenCodeUsageLedgerError::IncompatibleSnapshot)
    } else {
        Ok(())
    }
}

fn cost_is_lower_with_tolerance(
    current: Option<u64>,
    accepted: Option<u64>,
    tolerance: u64,
) -> bool {
    matches!(
        (current, accepted),
        (Some(current), Some(accepted)) if current.saturating_add(tolerance) < accepted
    )
}

fn cost_remainder(
    cumulative: Option<u64>,
    actual: Option<u64>,
    tolerance: u64,
) -> Result<Option<u64>, OpenCodeUsageLedgerError> {
    match (cumulative, actual) {
        (Some(cumulative), Some(actual)) if actual > cumulative => {
            if actual - cumulative <= tolerance {
                Ok(Some(0))
            } else {
                Err(OpenCodeUsageLedgerError::IncompatibleSnapshot)
            }
        }
        (Some(cumulative), Some(actual)) => Ok(Some(cumulative - actual)),
        _ => Ok(None),
    }
}

fn cost_rounding_tolerance(
    records: &[OpenCodeLedgerRecord],
) -> Result<u64, OpenCodeUsageLedgerError> {
    u64::try_from(
        records
            .iter()
            .filter(|record| {
                record.origin != OpenCodeLedgerOrigin::CumulativeRecovery
                    && record.cost_micros.is_some()
            })
            .count(),
    )
    .map_err(|_| OpenCodeUsageLedgerError::IncompatibleSnapshot)
}

fn exact_cost_rounding_tolerance(
    records: &[OpenCodeExactUsage],
) -> Result<u64, OpenCodeUsageLedgerError> {
    u64::try_from(
        records
            .iter()
            .filter(|record| record.cost_micros.is_some())
            .count(),
    )
    .map_err(|_| OpenCodeUsageLedgerError::IncompatibleSnapshot)
}

fn reconciliation_state(
    snapshot: &OpenCodeSessionLedgerSnapshot,
    records: &[OpenCodeLedgerRecord],
) -> OpenCodeReconciliationState {
    if matches!(
        snapshot.recovery_disposition,
        OpenCodeRecoveryDisposition::DeferredLiveWrite
            | OpenCodeRecoveryDisposition::StableIncomplete
    ) {
        OpenCodeReconciliationState::DeferredLiveWrite
    } else if records
        .iter()
        .any(|record| record.quality == OpenCodeDataQuality::Partial)
    {
        OpenCodeReconciliationState::Partial
    } else {
        OpenCodeReconciliationState::Complete
    }
}

fn persist_checkpoint(
    transaction: &Transaction<'_>,
    snapshot: &OpenCodeSessionLedgerSnapshot,
    previous: Option<&OpenCodeSessionCheckpoint>,
    accepted: LedgerTotals,
    state: OpenCodeReconciliationState,
    next_recovery_sequence: u64,
) -> Result<OpenCodeSessionCheckpoint, OpenCodeUsageLedgerError> {
    let first_observed_at_ms =
        previous.map_or(snapshot.observed_at_ms, |value| value.first_observed_at_ms);
    let last_reconciled_at_ms = snapshot.observed_at_ms.max(first_observed_at_ms);
    transaction
        .execute(
            "UPDATE opencode_session_checkpoint SET
                accepted_input_tokens = ?2, accepted_output_tokens = ?3,
                accepted_reasoning_tokens = ?4, accepted_cache_read_tokens = ?5,
                accepted_cache_write_tokens = ?6, accepted_cost_micros = ?7,
                observed_input_tokens = ?8, observed_output_tokens = ?9,
                observed_reasoning_tokens = ?10, observed_cache_read_tokens = ?11,
                observed_cache_write_tokens = ?12, observed_cost_micros = ?13,
                source_updated_at_ms = ?14, reconciliation_state = ?15,
                next_recovery_sequence = ?16, last_reconciled_at_ms = ?17
             WHERE session_id = ?1",
            params![
                snapshot.session_id,
                sql_u64(accepted.tokens.input)?,
                sql_u64(accepted.tokens.output)?,
                sql_u64(accepted.tokens.reasoning)?,
                sql_u64(accepted.tokens.cache_read)?,
                sql_u64(accepted.tokens.cache_write)?,
                sql_optional_u64(accepted.cost)?,
                sql_u64(snapshot.cumulative_tokens.input)?,
                sql_u64(snapshot.cumulative_tokens.output)?,
                sql_u64(snapshot.cumulative_tokens.reasoning)?,
                sql_u64(snapshot.cumulative_tokens.cache_read)?,
                sql_u64(snapshot.cumulative_tokens.cache_write)?,
                sql_optional_u64(snapshot.cumulative_cost_micros)?,
                snapshot.source_updated_at_ms,
                reconciliation_state_value(state),
                sql_u64(next_recovery_sequence)?,
                last_reconciled_at_ms,
            ],
        )
        .map_err(|_| OpenCodeUsageLedgerError::Storage)?;

    Ok(OpenCodeSessionCheckpoint {
        session_id: snapshot.session_id.clone(),
        accepted_tokens: accepted.tokens,
        accepted_cost_micros: accepted.cost,
        observed_source_tokens: snapshot.cumulative_tokens,
        observed_source_cost_micros: snapshot.cumulative_cost_micros,
        source_updated_at_ms: snapshot.source_updated_at_ms,
        reconciliation_state: state,
        next_recovery_sequence,
        first_observed_at_ms,
        last_reconciled_at_ms,
    })
}

fn insert_exact(
    transaction: &Transaction<'_>,
    session_id: &str,
    exact: &OpenCodeExactUsage,
    observed_at_ms: i64,
) -> Result<(), OpenCodeUsageLedgerError> {
    transaction
        .execute(
            "INSERT INTO opencode_usage_ledger (
                source_message_id, recovery_sequence, session_id, activity_at_ms,
                timestamp_origin, provider_id, raw_model_id, input_tokens,
                output_tokens, reasoning_tokens, cache_read_tokens,
                cache_write_tokens, cost_micros, origin, data_quality,
                first_seen_at_ms, last_seen_at_ms
             ) VALUES (
                ?1, NULL, ?2, ?3, 'source_reported', ?4, ?5, ?6, ?7, ?8,
                ?9, ?10, ?11, ?12, 'complete', ?13, ?13
             )",
            params![
                exact.message_id,
                session_id,
                exact.activity_at_ms,
                exact.provider_id,
                exact.raw_model_id,
                sql_u64(exact.tokens.input)?,
                sql_u64(exact.tokens.output)?,
                sql_u64(exact.tokens.reasoning)?,
                sql_u64(exact.tokens.cache_read)?,
                sql_u64(exact.tokens.cache_write)?,
                sql_optional_u64(exact.cost_micros)?,
                exact_origin_value(exact.origin),
                observed_at_ms,
            ],
        )
        .map_err(|_| OpenCodeUsageLedgerError::Storage)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn insert_recovery(
    transaction: &Transaction<'_>,
    snapshot: &OpenCodeSessionLedgerSnapshot,
    sequence: u64,
    activity_at_ms: i64,
    timestamp_origin: OpenCodeTimestampOrigin,
    tokens: OpenCodeTokenVector,
    cost_micros: Option<u64>,
) -> Result<(), OpenCodeUsageLedgerError> {
    transaction
        .execute(
            "INSERT INTO opencode_usage_ledger (
                source_message_id, recovery_sequence, session_id, activity_at_ms,
                timestamp_origin, provider_id, raw_model_id, input_tokens,
                output_tokens, reasoning_tokens, cache_read_tokens,
                cache_write_tokens, cost_micros, origin, data_quality,
                first_seen_at_ms, last_seen_at_ms
             ) VALUES (
                NULL, ?1, ?2, ?3, ?4, NULL, ?5, ?6, ?7, ?8, ?9, ?10, ?11,
                'cumulative_recovery', 'partial', ?12, ?12
             )",
            params![
                sql_u64(sequence)?,
                snapshot.session_id,
                activity_at_ms,
                timestamp_origin_value(timestamp_origin),
                UNATTRIBUTED_MODEL,
                sql_u64(tokens.input)?,
                sql_u64(tokens.output)?,
                sql_u64(tokens.reasoning)?,
                sql_u64(tokens.cache_read)?,
                sql_u64(tokens.cache_write)?,
                sql_optional_u64(cost_micros)?,
                snapshot.observed_at_ms,
            ],
        )
        .map_err(|_| OpenCodeUsageLedgerError::Storage)?;
    Ok(())
}

fn delete_recovery(
    transaction: &Transaction<'_>,
    session_id: &str,
    sequence: Option<u64>,
) -> Result<(), OpenCodeUsageLedgerError> {
    let sequence = sequence.ok_or(OpenCodeUsageLedgerError::IncompatibleSnapshot)?;
    transaction
        .execute(
            "DELETE FROM opencode_usage_ledger
             WHERE session_id = ?1 AND recovery_sequence = ?2",
            params![session_id, sql_u64(sequence)?],
        )
        .map_err(|_| OpenCodeUsageLedgerError::Storage)?;
    Ok(())
}

fn source_message_belongs_to_other_session(
    transaction: &Transaction<'_>,
    snapshot: &OpenCodeSessionLedgerSnapshot,
    exact: &OpenCodeExactUsage,
) -> Result<bool, OpenCodeUsageLedgerError> {
    transaction
        .query_row(
            "SELECT session_id FROM opencode_usage_ledger WHERE source_message_id = ?1",
            [&exact.message_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map(|session| session.is_some_and(|session| session != snapshot.session_id))
        .map_err(|_| OpenCodeUsageLedgerError::Storage)
}

fn select_checkpoint(
    connection: &Connection,
    session_id: &str,
) -> Result<Option<OpenCodeSessionCheckpoint>, OpenCodeUsageLedgerError> {
    connection
        .query_row(
            "SELECT session_id,
                    accepted_input_tokens, accepted_output_tokens,
                    accepted_reasoning_tokens, accepted_cache_read_tokens,
                    accepted_cache_write_tokens, accepted_cost_micros,
                    observed_input_tokens, observed_output_tokens,
                    observed_reasoning_tokens, observed_cache_read_tokens,
                    observed_cache_write_tokens, observed_cost_micros,
                    source_updated_at_ms, reconciliation_state,
                    next_recovery_sequence, first_observed_at_ms,
                    last_reconciled_at_ms
             FROM opencode_session_checkpoint WHERE session_id = ?1",
            [session_id],
            map_checkpoint,
        )
        .optional()
        .map_err(|_| OpenCodeUsageLedgerError::Storage)
}

fn select_records(
    connection: &Connection,
    session_id: &str,
) -> Result<Vec<OpenCodeLedgerRecord>, OpenCodeUsageLedgerError> {
    let mut statement = connection
        .prepare(
            "SELECT source_message_id, recovery_sequence, session_id,
                    activity_at_ms, timestamp_origin, provider_id, raw_model_id,
                    input_tokens, output_tokens, reasoning_tokens,
                    cache_read_tokens, cache_write_tokens, cost_micros,
                    origin, data_quality, first_seen_at_ms, last_seen_at_ms
             FROM opencode_usage_ledger
             WHERE session_id = ?1
             ORDER BY activity_at_ms ASC, id ASC",
        )
        .map_err(|_| OpenCodeUsageLedgerError::Storage)?;
    let records = statement
        .query_map([session_id], map_record)
        .map_err(|_| OpenCodeUsageLedgerError::Storage)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| OpenCodeUsageLedgerError::Storage)?;
    Ok(records)
}

fn map_checkpoint(row: &rusqlite::Row<'_>) -> Result<OpenCodeSessionCheckpoint, rusqlite::Error> {
    Ok(OpenCodeSessionCheckpoint {
        session_id: row.get(0)?,
        accepted_tokens: row_vector(row, 1)?,
        accepted_cost_micros: row_optional_u64(row, 6)?,
        observed_source_tokens: row_vector(row, 7)?,
        observed_source_cost_micros: row_optional_u64(row, 12)?,
        source_updated_at_ms: row.get(13)?,
        reconciliation_state: parse_reconciliation_state(&row.get::<_, String>(14)?)?,
        next_recovery_sequence: row_u64(row, 15)?,
        first_observed_at_ms: row.get(16)?,
        last_reconciled_at_ms: row.get(17)?,
    })
}

fn map_record(row: &rusqlite::Row<'_>) -> Result<OpenCodeLedgerRecord, rusqlite::Error> {
    Ok(OpenCodeLedgerRecord {
        source_message_id: row.get(0)?,
        recovery_sequence: row_optional_u64(row, 1)?,
        session_id: row.get(2)?,
        activity_at_ms: row.get(3)?,
        timestamp_origin: parse_timestamp_origin(&row.get::<_, String>(4)?)?,
        provider_id: row.get(5)?,
        raw_model_id: row.get(6)?,
        tokens: row_vector(row, 7)?,
        cost_micros: row_optional_u64(row, 12)?,
        origin: parse_ledger_origin(&row.get::<_, String>(13)?)?,
        quality: parse_quality(&row.get::<_, String>(14)?)?,
        first_seen_at_ms: row.get(15)?,
        last_seen_at_ms: row.get(16)?,
    })
}

fn exact_record(
    session_id: &str,
    exact: &OpenCodeExactUsage,
    observed_at_ms: i64,
) -> OpenCodeLedgerRecord {
    OpenCodeLedgerRecord {
        source_message_id: Some(exact.message_id.clone()),
        recovery_sequence: None,
        session_id: session_id.to_owned(),
        activity_at_ms: exact.activity_at_ms,
        timestamp_origin: OpenCodeTimestampOrigin::SourceReported,
        provider_id: Some(exact.provider_id.clone()),
        raw_model_id: exact.raw_model_id.clone(),
        tokens: exact.tokens,
        cost_micros: exact.cost_micros,
        origin: ledger_origin(exact.origin),
        quality: OpenCodeDataQuality::Complete,
        first_seen_at_ms: observed_at_ms,
        last_seen_at_ms: observed_at_ms,
    }
}

fn validate_identity(value: &str) -> Result<(), OpenCodeUsageLedgerError> {
    if value.trim().is_empty() {
        Err(OpenCodeUsageLedgerError::IncompatibleSnapshot)
    } else {
        Ok(())
    }
}

fn validate_timestamp(value: i64) -> Result<(), OpenCodeUsageLedgerError> {
    if value < 0 {
        Err(OpenCodeUsageLedgerError::IncompatibleSnapshot)
    } else {
        Ok(())
    }
}

fn validate_vector(value: OpenCodeTokenVector) -> Result<(), OpenCodeUsageLedgerError> {
    for counter in [
        value.input,
        value.output,
        value.reasoning,
        value.cache_read,
        value.cache_write,
    ] {
        sql_u64(counter)?;
    }
    Ok(())
}

fn validate_optional_u64(value: Option<u64>) -> Result<(), OpenCodeUsageLedgerError> {
    sql_optional_u64(value).map(|_| ())
}

fn sql_u64(value: u64) -> Result<i64, OpenCodeUsageLedgerError> {
    i64::try_from(value).map_err(|_| OpenCodeUsageLedgerError::IncompatibleSnapshot)
}

fn sql_optional_u64(value: Option<u64>) -> Result<Option<i64>, OpenCodeUsageLedgerError> {
    value.map(sql_u64).transpose()
}

fn row_u64(row: &rusqlite::Row<'_>, index: usize) -> Result<u64, rusqlite::Error> {
    u64::try_from(row.get::<_, i64>(index)?).map_err(|_| rusqlite::Error::InvalidQuery)
}

fn row_optional_u64(row: &rusqlite::Row<'_>, index: usize) -> Result<Option<u64>, rusqlite::Error> {
    row.get::<_, Option<i64>>(index)?
        .map(|value| u64::try_from(value).map_err(|_| rusqlite::Error::InvalidQuery))
        .transpose()
}

fn row_vector(
    row: &rusqlite::Row<'_>,
    start: usize,
) -> Result<OpenCodeTokenVector, rusqlite::Error> {
    Ok(OpenCodeTokenVector {
        input: row_u64(row, start)?,
        output: row_u64(row, start + 1)?,
        reasoning: row_u64(row, start + 2)?,
        cache_read: row_u64(row, start + 3)?,
        cache_write: row_u64(row, start + 4)?,
    })
}

fn exact_origin(
    origin: OpenCodeLedgerOrigin,
) -> Result<OpenCodeExactOrigin, OpenCodeUsageLedgerError> {
    match origin {
        OpenCodeLedgerOrigin::V1Message => Ok(OpenCodeExactOrigin::V1Message),
        OpenCodeLedgerOrigin::V2Message => Ok(OpenCodeExactOrigin::V2Message),
        OpenCodeLedgerOrigin::CumulativeRecovery => {
            Err(OpenCodeUsageLedgerError::IncompatibleSnapshot)
        }
    }
}

const fn ledger_origin(origin: OpenCodeExactOrigin) -> OpenCodeLedgerOrigin {
    match origin {
        OpenCodeExactOrigin::V1Message => OpenCodeLedgerOrigin::V1Message,
        OpenCodeExactOrigin::V2Message => OpenCodeLedgerOrigin::V2Message,
    }
}

const fn exact_origin_value(origin: OpenCodeExactOrigin) -> &'static str {
    match origin {
        OpenCodeExactOrigin::V1Message => "v1_message",
        OpenCodeExactOrigin::V2Message => "v2_message",
    }
}

const fn timestamp_origin_value(origin: OpenCodeTimestampOrigin) -> &'static str {
    match origin {
        OpenCodeTimestampOrigin::SourceReported => "source_reported",
        OpenCodeTimestampOrigin::SourceLifecycle => "source_lifecycle",
        OpenCodeTimestampOrigin::FirstSeen => "first_seen",
    }
}

const fn reconciliation_state_value(state: OpenCodeReconciliationState) -> &'static str {
    match state {
        OpenCodeReconciliationState::Complete => "complete",
        OpenCodeReconciliationState::Partial => "partial",
        OpenCodeReconciliationState::DeferredLiveWrite => "deferred_live_write",
    }
}

fn parse_timestamp_origin(value: &str) -> Result<OpenCodeTimestampOrigin, rusqlite::Error> {
    match value {
        "source_reported" => Ok(OpenCodeTimestampOrigin::SourceReported),
        "source_lifecycle" => Ok(OpenCodeTimestampOrigin::SourceLifecycle),
        "first_seen" => Ok(OpenCodeTimestampOrigin::FirstSeen),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn parse_ledger_origin(value: &str) -> Result<OpenCodeLedgerOrigin, rusqlite::Error> {
    match value {
        "v1_message" => Ok(OpenCodeLedgerOrigin::V1Message),
        "v2_message" => Ok(OpenCodeLedgerOrigin::V2Message),
        "cumulative_recovery" => Ok(OpenCodeLedgerOrigin::CumulativeRecovery),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn parse_quality(value: &str) -> Result<OpenCodeDataQuality, rusqlite::Error> {
    match value {
        "complete" => Ok(OpenCodeDataQuality::Complete),
        "partial" => Ok(OpenCodeDataQuality::Partial),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn parse_reconciliation_state(value: &str) -> Result<OpenCodeReconciliationState, rusqlite::Error> {
    match value {
        "complete" => Ok(OpenCodeReconciliationState::Complete),
        "partial" => Ok(OpenCodeReconciliationState::Partial),
        "deferred_live_write" => Ok(OpenCodeReconciliationState::DeferredLiveWrite),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::database::test_database::TestDatabase;

    #[test]
    fn compatible_v2_metadata_replaces_v1_without_duplicate_usage() {
        let (_database, store) = migrated_store();
        store
            .reconcile_session(&snapshot(
                "session-overlap",
                10,
                vec![exact("message-shared", 10, OpenCodeExactOrigin::V1Message)],
                100,
                OpenCodeRecoveryDisposition::Ready,
            ))
            .expect("V1 reconcile");

        let mut v2 = exact("message-shared", 10, OpenCodeExactOrigin::V2Message);
        v2.provider_id = "provider-v2".to_owned();
        v2.raw_model_id = "model-v2".to_owned();
        let result = store
            .reconcile_session(&snapshot(
                "session-overlap",
                10,
                vec![
                    exact("message-shared", 10, OpenCodeExactOrigin::V1Message),
                    v2,
                ],
                200,
                OpenCodeRecoveryDisposition::Ready,
            ))
            .expect("V2 reconcile");

        assert_eq!(result.records.len(), 1);
        assert_eq!(result.records[0].origin, OpenCodeLedgerOrigin::V2Message);
        assert_eq!(
            result.records[0].provider_id.as_deref(),
            Some("provider-v2")
        );
        assert_eq!(result.records[0].raw_model_id, "model-v2");
        assert_eq!(result.checkpoint.accepted_tokens, vector(10));
    }

    #[test]
    fn compaction_retains_exact_usage_and_retry_keeps_recovery_stable() {
        let (_database, store) = migrated_store();
        let initial = snapshot(
            "session-compacted",
            10,
            vec![exact("message-visible", 4, OpenCodeExactOrigin::V2Message)],
            100,
            OpenCodeRecoveryDisposition::Ready,
        );
        let first = store
            .reconcile_session(&initial)
            .expect("initial reconcile");
        assert_eq!(first.records.len(), 2);
        assert_eq!(first.recovery_segments_created, 1);

        let compacted = snapshot(
            "session-compacted",
            10,
            Vec::new(),
            200,
            OpenCodeRecoveryDisposition::Ready,
        );
        let second = store
            .reconcile_session(&compacted)
            .expect("compacted reconcile");
        let third = store
            .reconcile_session(&compacted)
            .expect("idempotent retry");

        assert_eq!(second.records, third.records);
        assert_eq!(second.records.len(), 2);
        assert_eq!(second.recovery_segments_created, 0);
        let recovery = second
            .records
            .iter()
            .find(|record| record.origin == OpenCodeLedgerOrigin::CumulativeRecovery)
            .expect("recovery");
        assert_eq!(recovery.tokens, vector(6));
        assert_eq!(recovery.recovery_sequence, Some(0));
        assert_eq!(recovery.activity_at_ms, 99);
        assert_eq!(recovery.first_seen_at_ms, 100);
        assert_eq!(recovery.last_seen_at_ms, 100);
    }

    #[test]
    fn cumulative_growth_creates_a_new_immutable_recovery_segment() {
        let (_database, store) = migrated_store();
        store
            .reconcile_session(&snapshot(
                "session-growth",
                10,
                vec![exact("message-one", 4, OpenCodeExactOrigin::V2Message)],
                100,
                OpenCodeRecoveryDisposition::Ready,
            ))
            .expect("initial");

        let grown = store
            .reconcile_session(&snapshot(
                "session-growth",
                14,
                vec![exact("message-one", 4, OpenCodeExactOrigin::V2Message)],
                200,
                OpenCodeRecoveryDisposition::Ready,
            ))
            .expect("growth");

        let recoveries = grown
            .records
            .iter()
            .filter(|record| record.origin == OpenCodeLedgerOrigin::CumulativeRecovery)
            .collect::<Vec<_>>();
        assert_eq!(recoveries.len(), 2);
        assert_eq!(recoveries[0].tokens, vector(6));
        assert_eq!(recoveries[0].recovery_sequence, Some(0));
        assert_eq!(recoveries[1].tokens, vector(4));
        assert_eq!(recoveries[1].recovery_sequence, Some(1));
        assert_eq!(grown.checkpoint.next_recovery_sequence, 2);
    }

    #[test]
    fn live_write_defers_recovery_until_a_stable_retry() {
        let (_database, store) = migrated_store();
        let deferred = store
            .reconcile_session(&snapshot(
                "session-live",
                10,
                vec![exact("message-durable", 4, OpenCodeExactOrigin::V2Message)],
                100,
                OpenCodeRecoveryDisposition::DeferredLiveWrite,
            ))
            .expect("deferred");

        assert_eq!(deferred.records.len(), 1);
        assert_eq!(deferred.checkpoint.accepted_tokens, vector(4));
        assert_eq!(deferred.checkpoint.observed_source_tokens, vector(10));
        assert_eq!(
            deferred.checkpoint.reconciliation_state,
            OpenCodeReconciliationState::DeferredLiveWrite
        );

        let stable = store
            .reconcile_session(&snapshot(
                "session-live",
                10,
                vec![exact("message-durable", 4, OpenCodeExactOrigin::V2Message)],
                200,
                OpenCodeRecoveryDisposition::StableIncomplete,
            ))
            .expect("stable retry");
        assert_eq!(stable.records.len(), 2);
        assert_eq!(stable.recovery_segments_created, 1);
        assert_eq!(stable.checkpoint.accepted_tokens, vector(10));
        assert_eq!(
            stable.checkpoint.reconciliation_state,
            OpenCodeReconciliationState::DeferredLiveWrite
        );

        let repeated = store
            .reconcile_session(&snapshot(
                "session-live",
                10,
                vec![exact("message-durable", 4, OpenCodeExactOrigin::V2Message)],
                300,
                OpenCodeRecoveryDisposition::StableIncomplete,
            ))
            .expect("repeated stable retry");
        assert_eq!(
            repeated.checkpoint.accepted_tokens,
            stable.checkpoint.accepted_tokens
        );
        assert_eq!(
            repeated.checkpoint.accepted_cost_micros,
            stable.checkpoint.accepted_cost_micros
        );
        assert_eq!(repeated.records.len(), stable.records.len());
        assert_eq!(repeated.recovery_segments_created, 0);
    }

    #[test]
    fn unique_matching_recovery_is_replaced_by_late_exact_detail() {
        let (_database, store) = migrated_store();
        store
            .reconcile_session(&snapshot(
                "session-late",
                10,
                Vec::new(),
                100,
                OpenCodeRecoveryDisposition::Ready,
            ))
            .expect("recovery");

        let detailed = store
            .reconcile_session(&snapshot(
                "session-late",
                10,
                vec![exact("message-late", 10, OpenCodeExactOrigin::V2Message)],
                200,
                OpenCodeRecoveryDisposition::Ready,
            ))
            .expect("late detail");

        assert_eq!(detailed.records.len(), 1);
        assert_eq!(detailed.records[0].origin, OpenCodeLedgerOrigin::V2Message);
        assert_eq!(detailed.late_exact_reclassified, 1);
        assert_eq!(detailed.checkpoint.accepted_tokens, vector(10));
        assert_eq!(
            detailed.checkpoint.reconciliation_state,
            OpenCodeReconciliationState::Complete
        );
    }

    #[test]
    fn nonmatching_late_detail_remains_represented_by_partial_recovery() {
        let (_database, store) = migrated_store();
        store
            .reconcile_session(&snapshot(
                "session-ambiguous",
                10,
                Vec::new(),
                100,
                OpenCodeRecoveryDisposition::Ready,
            ))
            .expect("recovery");

        let result = store
            .reconcile_session(&snapshot(
                "session-ambiguous",
                10,
                vec![exact("message-late", 6, OpenCodeExactOrigin::V2Message)],
                200,
                OpenCodeRecoveryDisposition::Ready,
            ))
            .expect("ambiguous late detail");

        assert_eq!(result.records.len(), 1);
        assert_eq!(
            result.records[0].origin,
            OpenCodeLedgerOrigin::CumulativeRecovery
        );
        assert_eq!(result.late_exact_ignored, 1);
        assert_eq!(result.checkpoint.accepted_tokens, vector(10));
    }

    #[test]
    fn counter_regression_rebuilds_only_the_affected_session() {
        let (_database, store) = migrated_store();
        store
            .reconcile_session(&snapshot(
                "session-regressed",
                10,
                Vec::new(),
                100,
                OpenCodeRecoveryDisposition::Ready,
            ))
            .expect("initial recovery");
        store
            .reconcile_session(&snapshot(
                "session-stable",
                7,
                vec![exact("message-stable", 7, OpenCodeExactOrigin::V2Message)],
                100,
                OpenCodeRecoveryDisposition::Ready,
            ))
            .expect("stable session");

        let rebuilt = store
            .reconcile_session(&snapshot(
                "session-regressed",
                5,
                vec![exact("message-current", 5, OpenCodeExactOrigin::V2Message)],
                200,
                OpenCodeRecoveryDisposition::Ready,
            ))
            .expect("regression rebuild");

        assert_eq!(rebuilt.counter_regressions, 1);
        assert_eq!(rebuilt.records.len(), 1);
        assert_eq!(rebuilt.records[0].tokens, vector(5));
        assert_eq!(rebuilt.checkpoint.accepted_tokens, vector(5));
        assert_eq!(
            store
                .read_session_records("session-stable")
                .expect("stable records")[0]
                .tokens,
            vector(7)
        );
    }

    #[test]
    fn v1_only_history_is_retained_when_new_v2_detail_arrives() {
        let (_database, store) = migrated_store();
        store
            .reconcile_session(&snapshot(
                "session-migrated",
                5,
                vec![exact("message-v1-only", 5, OpenCodeExactOrigin::V1Message)],
                100,
                OpenCodeRecoveryDisposition::Ready,
            ))
            .expect("V1 history");

        let result = store
            .reconcile_session(&snapshot(
                "session-migrated",
                8,
                vec![exact("message-v2-new", 3, OpenCodeExactOrigin::V2Message)],
                200,
                OpenCodeRecoveryDisposition::Ready,
            ))
            .expect("V2 detail");

        assert_eq!(result.records.len(), 2);
        assert!(result.records.iter().any(|record| {
            record.source_message_id.as_deref() == Some("message-v1-only")
                && record.origin == OpenCodeLedgerOrigin::V1Message
        }));
        assert!(result.records.iter().any(|record| {
            record.source_message_id.as_deref() == Some("message-v2-new")
                && record.origin == OpenCodeLedgerOrigin::V2Message
        }));
        assert_eq!(result.checkpoint.accepted_tokens, vector(8));
    }

    #[test]
    fn unknown_exact_cost_does_not_invent_recovery_cost() {
        let (_database, store) = migrated_store();
        let mut usage = exact("message-unknown-cost", 4, OpenCodeExactOrigin::V2Message);
        usage.cost_micros = None;
        let result = store
            .reconcile_session(&snapshot(
                "session-unknown-cost",
                10,
                vec![usage],
                100,
                OpenCodeRecoveryDisposition::Ready,
            ))
            .expect("unknown cost");

        let recovery = result
            .records
            .iter()
            .find(|record| record.origin == OpenCodeLedgerOrigin::CumulativeRecovery)
            .expect("recovery");
        assert_eq!(recovery.tokens, vector(6));
        assert_eq!(recovery.cost_micros, None);
        assert_eq!(result.checkpoint.accepted_cost_micros, None);
        assert_eq!(result.checkpoint.observed_source_cost_micros, Some(100));
    }

    #[test]
    fn per_record_cost_rounding_tolerance_is_bounded_and_idempotent() {
        let (_database, store) = migrated_store();
        let mut first = exact("message-rounding-one", 1, OpenCodeExactOrigin::V2Message);
        first.cost_micros = Some(51);
        let mut second = exact("message-rounding-two", 1, OpenCodeExactOrigin::V2Message);
        second.cost_micros = Some(51);
        let mut rounded = snapshot(
            "session-rounding",
            2,
            vec![first, second],
            100,
            OpenCodeRecoveryDisposition::Ready,
        );
        rounded.cumulative_cost_micros = Some(101);

        let initial = store
            .reconcile_session(&rounded)
            .expect("one-micro aggregate rounding difference");
        let repeated = store
            .reconcile_session(&rounded)
            .expect("idempotent rounded retry");

        assert_eq!(initial.records, repeated.records);
        assert_eq!(repeated.checkpoint.accepted_cost_micros, Some(102));
        assert_eq!(repeated.checkpoint.observed_source_cost_micros, Some(101));
        assert_eq!(
            repeated.checkpoint.reconciliation_state,
            OpenCodeReconciliationState::Complete
        );

        let mut incompatible = rounded;
        incompatible.cumulative_cost_micros = Some(99);
        assert_eq!(
            store
                .reconcile_session(&incompatible)
                .expect_err("cost mismatch beyond per-record tolerance"),
            OpenCodeUsageLedgerError::IncompatibleSnapshot
        );
    }

    #[test]
    fn incompatible_overlap_and_unexplainable_initial_detail_roll_back() {
        let (_database, store) = migrated_store();
        store
            .reconcile_session(&snapshot(
                "session-rollback",
                5,
                vec![exact("message-conflict", 5, OpenCodeExactOrigin::V1Message)],
                100,
                OpenCodeRecoveryDisposition::Ready,
            ))
            .expect("initial");

        let conflict = store.reconcile_session(&snapshot(
            "session-rollback",
            6,
            vec![exact("message-conflict", 6, OpenCodeExactOrigin::V2Message)],
            200,
            OpenCodeRecoveryDisposition::Ready,
        ));
        assert_eq!(
            conflict.expect_err("conflicting overlap"),
            OpenCodeUsageLedgerError::IncompatibleSnapshot
        );
        assert_eq!(
            store
                .read_checkpoint("session-rollback")
                .expect("checkpoint")
                .expect("present")
                .accepted_tokens,
            vector(5)
        );

        let impossible = store.reconcile_session(&snapshot(
            "session-impossible",
            4,
            vec![exact(
                "message-too-large",
                5,
                OpenCodeExactOrigin::V2Message,
            )],
            100,
            OpenCodeRecoveryDisposition::Ready,
        ));
        assert_eq!(
            impossible.expect_err("unexplainable detail"),
            OpenCodeUsageLedgerError::IncompatibleSnapshot
        );
        assert!(store
            .read_checkpoint("session-impossible")
            .expect("checkpoint read")
            .is_none());
    }

    fn migrated_store() -> (TestDatabase, SqliteOpenCodeUsageLedgerStore) {
        let mut test_database = TestDatabase::open();
        test_database
            .database_mut()
            .migrate_to_latest()
            .expect("migrate");
        let store = SqliteOpenCodeUsageLedgerStore::new(
            Database::open(test_database.path()).expect("open ledger database"),
        );
        (test_database, store)
    }

    fn snapshot(
        session_id: &str,
        cumulative_input: u64,
        exact_usage: Vec<OpenCodeExactUsage>,
        observed_at_ms: i64,
        recovery_disposition: OpenCodeRecoveryDisposition,
    ) -> OpenCodeSessionLedgerSnapshot {
        OpenCodeSessionLedgerSnapshot {
            session_id: session_id.to_owned(),
            source_updated_at_ms: observed_at_ms - 1,
            recovery_activity_at_ms: Some(observed_at_ms - 1),
            cumulative_tokens: vector(cumulative_input),
            cumulative_cost_micros: Some(cumulative_input * 10),
            exact_usage,
            recovery_disposition,
            observed_at_ms,
        }
    }

    fn exact(message_id: &str, input: u64, origin: OpenCodeExactOrigin) -> OpenCodeExactUsage {
        OpenCodeExactUsage {
            message_id: message_id.to_owned(),
            activity_at_ms: 50,
            provider_id: "provider-v1".to_owned(),
            raw_model_id: "model-v1".to_owned(),
            tokens: vector(input),
            cost_micros: Some(input * 10),
            origin,
        }
    }

    const fn vector(input: u64) -> OpenCodeTokenVector {
        OpenCodeTokenVector {
            input,
            output: 0,
            reasoning: 0,
            cache_read: 0,
            cache_write: 0,
        }
    }
}
