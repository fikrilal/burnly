//! Source model and project identity resolution helpers used by daily and
//! session reconciliation.

use rusqlite::{params, Transaction};

use crate::application::collection::ModelUsageCandidate;
use crate::application::ports::usage_store::UsageStoreError;
use crate::application::reconciliation::SourceId;
use crate::infrastructure::project_identity::ProjectPathIdentity;

pub(super) fn resolve_model(
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

pub(super) fn resolve_project(
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
