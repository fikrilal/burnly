use rusqlite::{params, Row};
use std::sync::{Arc, Mutex};

use super::Database;
use crate::application::ports::session_store::{
    SessionPagination, SessionStore, SessionStoreError,
};
use crate::domain::usage::{
    CostKind, CurrencyCode, SessionDetail, SessionModelUsage, TokenUsage, UsageCost, UsageSession,
    ValuedCostStatus,
};

pub(crate) struct SqliteSessionStore {
    database: Arc<Mutex<Database>>,
}

impl SqliteSessionStore {
    pub(crate) fn new(database: Arc<Mutex<Database>>) -> Self {
        Self { database }
    }
}

impl SessionStore for SqliteSessionStore {
    fn get_sessions(
        &self,
        source_id: Option<i64>,
        pagination: SessionPagination,
    ) -> Result<Vec<UsageSession>, SessionStoreError> {
        let database = self
            .database
            .lock()
            .map_err(|_| SessionStoreError::Backend)?;
        let connection = database.connection();

        let mut statement = connection
            .prepare(
                "SELECT s.id, s.source_id, s.source_session_id, s.project_id, p.raw_path,
                s.first_activity_at_ms, s.last_activity_at_ms,
                s.input_tokens, s.output_tokens, s.cache_creation_tokens, s.cache_read_tokens,
                s.total_tokens, s.unclassified_tokens,
                s.cost_amount_micros, s.cost_currency, s.cost_kind, s.cost_status
            FROM sessions s
            LEFT JOIN projects p ON s.project_id = p.id
            WHERE s.record_state != 'removed'
              AND (?1 IS NULL OR s.source_id = ?1)
              AND (
                ?2 = 0
                OR (
                  ?3 IS NOT NULL
                  AND (
                    s.last_activity_at_ms < ?3
                    OR (s.last_activity_at_ms = ?3 AND s.id < ?4)
                    OR s.last_activity_at_ms IS NULL
                  )
                )
                OR (
                  ?3 IS NULL
                  AND s.last_activity_at_ms IS NULL
                  AND s.id < ?4
                )
              )
            ORDER BY s.last_activity_at_ms DESC, s.id DESC
            LIMIT ?5",
            )
            .map_err(|_| SessionStoreError::Backend)?;

        let cursor = pagination.after;
        let cursor_present = if cursor.is_some() { 1_i64 } else { 0_i64 };
        let cursor_activity = cursor.and_then(|value| value.last_activity_at_ms);
        let cursor_session_id = cursor.map_or(0_i64, |value| value.session_id);
        let limit = i64::from(pagination.limit);

        let rows = statement
            .query_map(
                params![
                    source_id,
                    cursor_present,
                    cursor_activity,
                    cursor_session_id,
                    limit
                ],
                parse_session,
            )
            .map_err(|_| SessionStoreError::Backend)?;

        let mut sessions = Vec::new();
        for row in rows {
            sessions.push(row.map_err(|_| SessionStoreError::Backend)?);
        }

        Ok(sessions)
    }

    fn get_session_detail(&self, session_id: i64) -> Result<SessionDetail, SessionStoreError> {
        let database = self
            .database
            .lock()
            .map_err(|_| SessionStoreError::Backend)?;
        let connection = database.connection();

        let session = connection
            .query_row(
                "SELECT s.id, s.source_id, s.source_session_id, s.project_id, p.raw_path,
                    s.first_activity_at_ms, s.last_activity_at_ms,
                    s.input_tokens, s.output_tokens, s.cache_creation_tokens, s.cache_read_tokens,
                    s.total_tokens, s.unclassified_tokens,
                    s.cost_amount_micros, s.cost_currency, s.cost_kind, s.cost_status
                FROM sessions s
                LEFT JOIN projects p ON s.project_id = p.id
                WHERE s.id = ?1 AND s.record_state != 'removed'",
                params![session_id],
                parse_session,
            )
            .map_err(|err| match err {
                rusqlite::Error::QueryReturnedNoRows => SessionStoreError::NotFound,
                _ => SessionStoreError::Backend,
            })?;

        let mut statement = connection
            .prepare(
                "SELECT m.raw_model_id,
                    u.input_tokens, u.output_tokens, u.cache_creation_tokens, u.cache_read_tokens,
                    u.total_tokens, u.unclassified_tokens,
                    u.cost_amount_micros, u.cost_currency, u.cost_status
                FROM session_model_usage u
                LEFT JOIN source_models m ON u.model_id = m.id
                WHERE u.session_id = ?1
                ORDER BY u.total_tokens DESC",
            )
            .map_err(|_| SessionStoreError::Backend)?;

        let rows = statement
            .query_map(params![session_id], parse_model_usage)
            .map_err(|_| SessionStoreError::Backend)?;

        let mut model_breakdowns = Vec::new();
        for row in rows {
            model_breakdowns.push(row.map_err(|_| SessionStoreError::Backend)?);
        }

        Ok(SessionDetail {
            session,
            model_breakdowns,
        })
    }
}

fn parse_session(row: &Row<'_>) -> Result<UsageSession, rusqlite::Error> {
    Ok(UsageSession {
        session_id: row.get(0)?,
        source_id: row.get(1)?,
        source_session_id: row.get(2)?,
        project_id: row.get(3)?,
        project_path: row.get(4)?,
        first_activity_at_ms: row.get(5)?,
        last_activity_at_ms: row.get(6)?,
        tokens: database_token_usage(
            row.get(7)?,
            row.get(8)?,
            row.get(9)?,
            row.get(10)?,
            row.get(11)?,
        )
        .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(0, 0))?,
        cost: database_usage_cost(row.get(13)?, row.get(14)?, row.get(15)?, row.get(16)?)
            .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(0, 0))?,
    })
}

fn parse_model_usage(row: &Row<'_>) -> Result<SessionModelUsage, rusqlite::Error> {
    Ok(SessionModelUsage {
        raw_model_id: row.get(0)?,
        tokens: database_token_usage(
            row.get(1)?,
            row.get(2)?,
            row.get(3)?,
            row.get(4)?,
            row.get(5)?,
        )
        .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(0, 0))?,
        cost: database_model_usage_cost(row.get(7)?, row.get(8)?, row.get(9)?)
            .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(0, 0))?,
    })
}

fn database_token_usage(
    input: Option<i64>,
    output: Option<i64>,
    cache_creation: Option<i64>,
    cache_read: Option<i64>,
    total: i64,
) -> Result<TokenUsage, ()> {
    let input_tokens = input.map(u64::try_from).transpose().map_err(|_| ())?;
    let output_tokens = output.map(u64::try_from).transpose().map_err(|_| ())?;
    let cache_creation_tokens = cache_creation
        .map(u64::try_from)
        .transpose()
        .map_err(|_| ())?;
    let cache_read_tokens = cache_read.map(u64::try_from).transpose().map_err(|_| ())?;
    let total_tokens = u64::try_from(total).map_err(|_| ())?;

    TokenUsage::new(
        input_tokens,
        output_tokens,
        cache_creation_tokens,
        cache_read_tokens,
        total_tokens,
    )
    .map_err(|_| ())
}

fn database_usage_cost(
    amount_micros: Option<i64>,
    currency: Option<String>,
    kind: String,
    status: String,
) -> Result<UsageCost, ()> {
    let cost_kind = match kind.as_str() {
        "source_reported" => CostKind::SourceReported,
        "collector_calculated" => CostKind::CollectorCalculated,
        "collector_mixed" => CostKind::CollectorMixed,
        "burnly_calculated" => CostKind::BurnlyCalculated,
        _ => CostKind::Unknown,
    };

    match status.as_str() {
        "available" | "estimated" => {
            let amount = u64::try_from(amount_micros.ok_or(())?).map_err(|_| ())?;
            let currency = CurrencyCode::new(currency.ok_or(())?).map_err(|_| ())?;
            let status = if status == "estimated" {
                ValuedCostStatus::Estimated
            } else {
                ValuedCostStatus::Available
            };

            Ok(UsageCost::Valued {
                amount_micros: amount,
                currency,
                kind: cost_kind,
                status,
            })
        }
        "not_applicable" => Ok(UsageCost::NotApplicable { kind: cost_kind }),
        _ => Ok(UsageCost::Unavailable { kind: cost_kind }),
    }
}

fn database_model_usage_cost(
    amount_micros: Option<i64>,
    currency: Option<String>,
    status: String,
) -> Result<UsageCost, ()> {
    // Model usage only uses 'estimated' or 'unavailable', kind is always CollectorCalculated for now.
    match status.as_str() {
        "estimated" => {
            let amount = u64::try_from(amount_micros.ok_or(())?).map_err(|_| ())?;
            let currency = CurrencyCode::new(currency.ok_or(())?).map_err(|_| ())?;
            Ok(UsageCost::Valued {
                amount_micros: amount,
                currency,
                kind: CostKind::CollectorCalculated,
                status: ValuedCostStatus::Estimated,
            })
        }
        _ => Ok(UsageCost::Unavailable {
            kind: CostKind::CollectorCalculated,
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::application::ports::session_store::SessionPageCursor;

    fn migrated_store() -> (tempfile::TempDir, SqliteSessionStore) {
        let directory = tempfile::tempdir().expect("temporary directory");
        let database_path = directory.path().join("burnly.sqlite3");
        let mut database = Database::open(&database_path).expect("open database");
        database.migrate_to_latest().expect("migrate database");
        seed_settings(database.connection());
        let database = Arc::new(Mutex::new(database));
        (directory, SqliteSessionStore::new(database))
    }

    fn seed_settings(connection: &rusqlite::Connection) {
        connection
            .execute(
                "INSERT INTO app_settings (
                    id, reporting_timezone, background_refresh_enabled,
                    refresh_interval_minutes, launch_at_login, close_behavior,
                    notifications_enabled, store_project_paths,
                    created_at_ms, updated_at_ms
                ) VALUES (1, 'UTC', 0, 15, 0, 'quit', 0, 0, 0, 0)",
                [],
            )
            .expect("seed settings");
    }

    fn seed_source(connection: &rusqlite::Connection) {
        connection
            .execute(
                "INSERT INTO sources (
                    id, source_key, display_name, enabled, detection_state,
                    created_at_ms, updated_at_ms
                ) VALUES (1, 'claude-code', 'Claude Code', 1, 'available', 0, 0)",
                [],
            )
            .expect("seed source");
    }

    fn seed_import(connection: &rusqlite::Connection) -> i64 {
        connection
            .execute(
                "INSERT INTO refresh_runs (
                    id, job_key, trigger, status, started_at_ms, finished_at_ms,
                    requested_by_app_version, created_at_ms
                ) VALUES (1, 'job-1', 'manual', 'succeeded', 100, 200, '0.1.0', 100)",
                [],
            )
            .expect("seed refresh");
        connection
            .execute(
                "INSERT INTO import_runs (
                    refresh_run_id, source_id, collector_key, collector_version,
                    profile_version, projection, scope_kind, aggregation_timezone,
                    status, records_seen, records_rejected, started_at_ms,
                    finished_at_ms
                ) VALUES (1, 1, 'ccusage', '20.0.11', 1, 'session', 'full',
                    NULL, 'succeeded', 1, 0, 100, 200)",
                [],
            )
            .expect("seed import");
        connection.last_insert_rowid()
    }

    fn seed_session(
        connection: &rusqlite::Connection,
        id: i64,
        source_session_id: &str,
        last_activity_at_ms: Option<i64>,
        total_tokens: i64,
        import_id: i64,
    ) {
        connection
            .execute(
                "INSERT INTO sessions (
                    id, source_id, source_key, identity_version, source_session_id,
                    total_tokens, cost_kind, cost_status, data_quality, record_state,
                    absence_count, first_seen_at_ms, last_seen_at_ms,
                    first_activity_at_ms, last_activity_at_ms, latest_import_id
                ) VALUES (?1, 1, ?2, 1, ?3, ?4, 'collector_calculated',
                    'unavailable', 'complete', 'active', 0, 100, 200, 100,
                    ?5, ?6)",
                params![
                    id,
                    format!("claude-code:session:v1:{source_session_id}"),
                    source_session_id,
                    total_tokens,
                    last_activity_at_ms,
                    import_id
                ],
            )
            .expect("seed session");
    }

    #[test]
    fn paginates_duplicate_activity_timestamps_with_session_id_tiebreaker() {
        let (_directory, store) = migrated_store();
        let import_id = {
            let database = store.database.lock().expect("lock database");
            let connection = database.connection();
            seed_source(connection);
            let import_id = seed_import(connection);
            seed_session(connection, 1, "session-1", Some(1_000), 10, import_id);
            seed_session(connection, 2, "session-2", Some(1_000), 20, import_id);
            seed_session(connection, 3, "session-3", Some(1_000), 30, import_id);
            import_id
        };
        assert!(import_id > 0);

        let first_page = store
            .get_sessions(
                None,
                SessionPagination {
                    limit: 2,
                    after: None,
                },
            )
            .expect("first page");

        assert_eq!(
            first_page
                .iter()
                .map(|session| session.session_id)
                .collect::<Vec<_>>(),
            vec![3, 2]
        );

        let second_page = store
            .get_sessions(
                None,
                SessionPagination {
                    limit: 2,
                    after: Some(SessionPageCursor {
                        last_activity_at_ms: first_page[1].last_activity_at_ms,
                        session_id: first_page[1].session_id,
                    }),
                },
            )
            .expect("second page");

        assert_eq!(
            second_page
                .iter()
                .map(|session| session.session_id)
                .collect::<Vec<_>>(),
            vec![1]
        );
    }

    #[test]
    fn rejects_negative_token_values_instead_of_coercing_to_zero() {
        assert!(database_token_usage(Some(-1), None, None, None, 10).is_err());
        assert!(database_token_usage(None, None, None, None, -1).is_err());
    }
}
