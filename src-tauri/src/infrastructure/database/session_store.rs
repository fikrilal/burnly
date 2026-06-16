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

        let mut query = String::from(
            "SELECT s.id, s.source_id, s.source_session_id, s.project_id, p.raw_path,
                s.first_activity_at_ms, s.last_activity_at_ms,
                s.input_tokens, s.output_tokens, s.cache_creation_tokens, s.cache_read_tokens,
                s.total_tokens, s.unclassified_tokens,
                s.cost_amount_micros, s.cost_currency, s.cost_kind, s.cost_status
            FROM sessions s
            LEFT JOIN projects p ON s.project_id = p.id
            WHERE s.record_state != 'removed'",
        );

        if source_id.is_some() {
            query.push_str(" AND s.source_id = ?1");
        }
        if pagination.after_activity_ms.is_some() {
            if source_id.is_some() {
                query.push_str(" AND s.last_activity_at_ms < ?2");
            } else {
                query.push_str(" AND s.last_activity_at_ms < ?1");
            }
        }

        query.push_str(" ORDER BY s.last_activity_at_ms DESC, s.id DESC LIMIT ");
        if source_id.is_some() && pagination.after_activity_ms.is_some() {
            query.push_str("?3");
        } else if source_id.is_some() || pagination.after_activity_ms.is_some() {
            query.push_str("?2");
        } else {
            query.push_str("?1");
        }

        let mut statement = connection
            .prepare(&query)
            .map_err(|_| SessionStoreError::Backend)?;

        let params_source = (source_id, pagination.after_activity_ms);
        let params: Vec<&dyn rusqlite::ToSql> = match params_source {
            (Some(ref s), Some(ref a)) => vec![s, a, &pagination.limit],
            (Some(ref s), None) => vec![s, &pagination.limit],
            (None, Some(ref a)) => vec![a, &pagination.limit],
            (None, None) => vec![&pagination.limit],
        };

        let rows = statement
            .query_map(&*params, parse_session)
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
    let input_tokens = input.map(|v| v.try_into().unwrap_or(0));
    let output_tokens = output.map(|v| v.try_into().unwrap_or(0));
    let cache_creation_tokens = cache_creation.map(|v| v.try_into().unwrap_or(0));
    let cache_read_tokens = cache_read.map(|v| v.try_into().unwrap_or(0));
    let total_tokens = total.try_into().unwrap_or(0);

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
            let amount = amount_micros.ok_or(())?.try_into().unwrap_or(0);
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
            let amount = amount_micros.ok_or(())?.try_into().unwrap_or(0);
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
