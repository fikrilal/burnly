use chrono::NaiveDate;
use serde::Deserialize;
use std::collections::HashSet;

use crate::application::collection::{CollectorFailure, CollectorFailureCode};

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PiDailyReport {
    pub daily: Vec<PiDailyRow>,
    pub totals: TokenTotals,
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPiDailyReport {
    daily: Vec<PiDailyRow>,
    totals: Option<TokenTotals>,
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PiDailyRow {
    pub date: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_tokens: Option<u64>,
    pub cache_read_tokens: Option<u64>,
    pub total_tokens: u64,
    pub total_cost: f64,
    #[serde(default)]
    pub models_used: Vec<String>,
    #[serde(default)]
    pub model_breakdowns: Vec<PiModelBreakdown>,
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PiModelBreakdown {
    pub model_name: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_tokens: Option<u64>,
    pub cache_read_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    pub cost: f64,
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TokenTotals {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_tokens: Option<u64>,
    pub cache_read_tokens: Option<u64>,
    pub total_tokens: u64,
    pub total_cost: f64,
}

pub(crate) fn decode(input: &str) -> Result<PiDailyReport, CollectorFailure> {
    let report = serde_json::from_str::<RawPiDailyReport>(input).map_err(decode_failure)?;
    let totals = match report.totals {
        Some(totals) => totals,
        None if report.daily.is_empty() => empty_totals(),
        None => return Err(incompatible()),
    };
    let report = PiDailyReport {
        daily: report.daily,
        totals,
    };
    validate(&report)?;
    Ok(report)
}

fn empty_totals() -> TokenTotals {
    TokenTotals {
        input_tokens: 0,
        output_tokens: 0,
        cache_creation_tokens: Some(0),
        cache_read_tokens: Some(0),
        total_tokens: 0,
        total_cost: 0.0,
    }
}

fn decode_failure(error: serde_json::Error) -> CollectorFailure {
    let code = match error.classify() {
        serde_json::error::Category::Syntax | serde_json::error::Category::Eof => {
            CollectorFailureCode::InvalidJson
        }
        serde_json::error::Category::Data | serde_json::error::Category::Io => {
            CollectorFailureCode::IncompatibleEnvelope
        }
    };
    CollectorFailure::new(code, None, None)
}

fn validate(report: &PiDailyReport) -> Result<(), CollectorFailure> {
    if !valid_totals(&report.totals) || !totals_match_rows(report) {
        return Err(incompatible());
    }

    let mut dates = HashSet::with_capacity(report.daily.len());
    for row in &report.daily {
        if NaiveDate::parse_from_str(&row.date, "%Y-%m-%d").is_err()
            || !dates.insert(row.date.as_str())
            || !valid_nonnegative(row.total_cost)
            || !valid_row_tokens(row)
            || row.model_breakdowns.iter().any(|model| !valid_model(model))
        {
            return Err(incompatible());
        }
    }
    Ok(())
}

fn valid_totals(totals: &TokenTotals) -> bool {
    valid_nonnegative(totals.total_cost)
        && categorized(
            totals.input_tokens,
            totals.output_tokens,
            totals.cache_creation_tokens,
            totals.cache_read_tokens,
        )
        .is_some_and(|tokens| totals.total_tokens >= tokens)
}

fn valid_model(model: &PiModelBreakdown) -> bool {
    if model.model_name.trim().is_empty() || !valid_nonnegative(model.cost) {
        return false;
    }
    let Some(classified) = categorized(
        model.input_tokens,
        model.output_tokens,
        model.cache_creation_tokens,
        model.cache_read_tokens,
    ) else {
        return false;
    };
    model
        .total_tokens
        .is_none_or(|total_tokens| total_tokens >= classified)
}

fn valid_row_tokens(row: &PiDailyRow) -> bool {
    categorized(
        row.input_tokens,
        row.output_tokens,
        row.cache_creation_tokens,
        row.cache_read_tokens,
    )
    .is_some_and(|tokens| row.total_tokens >= tokens)
}

fn categorized(
    input: u64,
    output: u64,
    cache_creation: Option<u64>,
    cache_read: Option<u64>,
) -> Option<u64> {
    input
        .checked_add(output)?
        .checked_add(cache_creation.unwrap_or(0))?
        .checked_add(cache_read.unwrap_or(0))
}

fn totals_match_rows(report: &PiDailyReport) -> bool {
    sum_rows(&report.daily, |row| row.input_tokens) == Some(report.totals.input_tokens)
        && sum_rows(&report.daily, |row| row.output_tokens) == Some(report.totals.output_tokens)
        && sum_rows(&report.daily, |row| row.total_tokens) == Some(report.totals.total_tokens)
}

fn sum_rows(rows: &[PiDailyRow], value: impl Fn(&PiDailyRow) -> u64) -> Option<u64> {
    rows.iter()
        .try_fold(0_u64, |total, row| total.checked_add(value(row)))
}

fn valid_nonnegative(value: f64) -> bool {
    value.is_finite() && value >= 0.0
}

fn incompatible() -> CollectorFailure {
    CollectorFailure::new(CollectorFailureCode::IncompatibleEnvelope, None, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_empty_pi_daily_output_with_null_totals() {
        let report = decode(r#"{"daily":[],"totals":null}"#).expect("empty pi report");
        assert!(report.daily.is_empty());
        assert_eq!(report.totals.total_tokens, 0);
        assert_eq!(report.totals.total_cost, 0.0);
    }

    #[test]
    fn decodes_reviewed_pi_fixture() {
        let report = decode(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tests/fixtures/collectors/ccusage/pi-daily/valid.json"
        )))
        .expect("valid Pi report");
        assert_eq!(report.daily.len(), 2);
        assert_eq!(report.daily[0].models_used, ["[pi] gpt-5.4-mini"]);
        assert_eq!(report.totals.total_tokens, 1_650);
    }

    #[test]
    fn rejects_non_empty_pi_daily_output_with_null_totals() {
        let error = decode(
            r#"{
              "daily": [{
                "date":"2026-07-03","inputTokens":10,"outputTokens":5,
                "totalTokens":15,"totalCost":0.01
              }],
              "totals":null
            }"#,
        )
        .expect_err("incompatible pi report");
        assert_eq!(error.code, CollectorFailureCode::IncompatibleEnvelope);
    }
}
