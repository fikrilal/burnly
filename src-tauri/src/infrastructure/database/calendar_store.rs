use crate::infrastructure::database::Database;
use chrono::NaiveDate;
use std::sync::Mutex;

use crate::application::ports::calendar_store::{CalendarStore, CalendarStoreError};
use crate::application::ports::day_detail_store::{DayDetailStore, DayDetailStoreError};
use crate::application::usage::{
    CalendarDayInfo, CalendarPeriod, CalendarReadModel, DayDetailReadModel, OverviewCost,
    OverviewDataStatus, OverviewSource,
};
use crate::domain::source::SourceKey;
use crate::domain::usage::CurrencyCode;

pub(crate) struct SqliteCalendarStore {
    database: Mutex<Database>,
}

impl SqliteCalendarStore {
    pub(crate) fn new(database: Database) -> Self {
        Self {
            database: Mutex::new(database),
        }
    }
}

impl CalendarStore for SqliteCalendarStore {
    fn read_calendar(
        &self,
        period: &CalendarPeriod,
    ) -> Result<CalendarReadModel, CalendarStoreError> {
        let db = self
            .database
            .lock()
            .map_err(|_| CalendarStoreError::Backend)?;
        let conn = db.connection();

        let mut stmt = conn
            .prepare(
                r#"
                SELECT 
                    usage_date,
                    SUM(total_tokens) as total_tokens,
                    COUNT(DISTINCT source_id) as active_sources,
                    SUM(cost_amount_micros) as cost_micros,
                    MAX(cost_currency) as currency
                FROM daily_usage
                WHERE record_state = 'active'
                  AND usage_date >= ?
                  AND usage_date <= ?
                GROUP BY usage_date
                ORDER BY usage_date ASC
                "#,
            )
            .map_err(|_| CalendarStoreError::Backend)?;

        let start_date_str = period.start_date().to_string();
        let end_date_str = period.end_date().to_string();

        let rows = stmt
            .query_map([&start_date_str, &end_date_str], |row| {
                let usage_date: String = row.get(0)?;
                let date = NaiveDate::parse_from_str(&usage_date, "%Y-%m-%d")
                    .unwrap_or(period.start_date());

                let total_tokens: i64 = row.get(1).unwrap_or(0);
                let active_sources: i64 = row.get(2).unwrap_or(0);
                let cost_micros: Option<i64> = row.get(3)?;
                let currency_str: Option<String> = row.get(4)?;

                let currency = currency_str.and_then(|c| CurrencyCode::new(c.as_str()).ok());

                Ok(CalendarDayInfo {
                    date,
                    total_tokens: total_tokens as u64,
                    active_sources: active_sources as u32,
                    cost: OverviewCost {
                        amount_micros: cost_micros.map(|v| v as u64),
                        currency,
                        valuation: crate::application::usage::CostValuation::Unavailable, // Simplify for now
                        completeness: crate::application::usage::CostCompleteness::Unavailable,
                        unavailable_days: 0,
                    },
                    has_partial_data: false,
                })
            })
            .map_err(|_| CalendarStoreError::Backend)?;

        let mut days = Vec::new();
        for row in rows {
            days.push(row.map_err(|_| CalendarStoreError::Backend)?);
        }

        Ok(CalendarReadModel {
            period: period.clone(),
            days,
            data_status: OverviewDataStatus::Current,
        })
    }
}

impl DayDetailStore for SqliteCalendarStore {
    fn read_day_detail(
        &self,
        date: NaiveDate,
    ) -> Result<Option<DayDetailReadModel>, DayDetailStoreError> {
        let db = self
            .database
            .lock()
            .map_err(|_| DayDetailStoreError::Backend)?;
        let conn = db.connection();

        let date_str = date.to_string();

        let mut stmt = conn
            .prepare(
                r#"
                SELECT 
                    SUM(total_tokens) as total_tokens,
                    SUM(cost_amount_micros) as cost_micros,
                    MAX(cost_currency) as currency
                FROM daily_usage
                WHERE record_state = 'active'
                  AND usage_date = ?
                "#,
            )
            .map_err(|_| DayDetailStoreError::Backend)?;

        let mut rows = stmt
            .query([&date_str])
            .map_err(|_| DayDetailStoreError::Backend)?;

        let row = match rows.next().map_err(|_| DayDetailStoreError::Backend)? {
            Some(row) => row,
            None => return Ok(None),
        };

        let total_tokens: Option<i64> = row.get(0).map_err(|_| DayDetailStoreError::Backend)?;
        if total_tokens.is_none() {
            return Ok(None);
        }
        let total_tokens = total_tokens.unwrap_or(0);
        let cost_micros: Option<i64> = row.get(1).map_err(|_| DayDetailStoreError::Backend)?;
        let currency_str: Option<String> = row.get(2).map_err(|_| DayDetailStoreError::Backend)?;

        let currency = currency_str.and_then(|c| CurrencyCode::new(c.as_str()).ok());

        // Now get sources
        let mut source_stmt = conn
            .prepare(
                r#"
                SELECT 
                    source_key,
                    total_tokens,
                    cost_amount_micros,
                    cost_currency
                FROM daily_usage
                WHERE record_state = 'active'
                  AND usage_date = ?
                "#,
            )
            .map_err(|_| DayDetailStoreError::Backend)?;

        let source_rows = source_stmt
            .query_map([&date_str], |row| {
                let source_key_str: String = row.get(0)?;
                let source = match source_key_str.as_str() {
                    "claude-code" => SourceKey::ClaudeCode,
                    "codex" => SourceKey::Codex,
                    _ => SourceKey::ClaudeCode, // Fallback
                };
                let s_tokens: i64 = row.get(1)?;
                let s_cost: Option<i64> = row.get(2)?;
                let s_curr: Option<String> = row.get(3)?;

                let s_currency = s_curr.and_then(|c| CurrencyCode::new(c.as_str()).ok());

                Ok(OverviewSource {
                    source,
                    total_tokens: s_tokens as u64,
                    active_days: 1,
                    cost: OverviewCost {
                        amount_micros: s_cost.map(|v| v as u64),
                        currency: s_currency,
                        valuation: crate::application::usage::CostValuation::Unavailable,
                        completeness: crate::application::usage::CostCompleteness::Unavailable,
                        unavailable_days: 0,
                    },
                    has_partial_data: false,
                })
            })
            .map_err(|_| DayDetailStoreError::Backend)?;

        let mut sources = Vec::new();
        for s_row in source_rows {
            sources.push(s_row.map_err(|_| DayDetailStoreError::Backend)?);
        }

        Ok(Some(DayDetailReadModel {
            date,
            total_tokens: total_tokens as u64,
            cost: OverviewCost {
                amount_micros: cost_micros.map(|v| v as u64),
                currency,
                valuation: crate::application::usage::CostValuation::Unavailable,
                completeness: crate::application::usage::CostCompleteness::Unavailable,
                unavailable_days: 0,
            },
            sources,
        }))
    }
}
