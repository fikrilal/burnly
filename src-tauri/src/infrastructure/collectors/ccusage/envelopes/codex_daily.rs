use chrono::NaiveDate;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

use crate::application::collection::{CollectorFailure, CollectorFailureCode};

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexDailyReport {
    pub daily: Vec<CodexDailyRow>,
    pub totals: CodexTokenTotals,
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexDailyRow {
    pub date: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_output_tokens: u64,
    pub total_tokens: u64,
    pub total_cost: f64,
    pub models: HashMap<String, CodexModelBreakdown>,
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexModelBreakdown {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_output_tokens: u64,
    pub cost: f64,
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexTokenTotals {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_output_tokens: u64,
    pub total_tokens: u64,
    pub total_cost: f64,
}

pub(crate) fn decode(input: &str) -> Result<CodexDailyReport, CollectorFailure> {
    let report = serde_json::from_str::<CodexDailyReport>(input).map_err(decode_failure)?;
    validate(&report)?;
    Ok(report)
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

fn validate(report: &CodexDailyReport) -> Result<(), CollectorFailure> {
    if !valid_totals(&report.totals) || !totals_match_rows(report) {
        return Err(incompatible());
    }

    let mut dates = HashSet::with_capacity(report.daily.len());
    for row in &report.daily {
        if NaiveDate::parse_from_str(&row.date, "%Y-%m-%d").is_err()
            || !dates.insert(row.date.as_str())
            || !valid_nonnegative(row.total_cost)
            || !valid_row_tokens(row)
            || row
                .models
                .iter()
                .any(|(name, model)| !valid_model(name, model))
        {
            return Err(incompatible());
        }
    }

    Ok(())
}

fn valid_totals(totals: &CodexTokenTotals) -> bool {
    valid_nonnegative(totals.total_cost)
        && categorized(
            totals.input_tokens,
            totals.output_tokens,
            totals.reasoning_output_tokens,
        )
        .is_some_and(|tokens| totals.total_tokens >= tokens)
}

fn valid_model(name: &str, model: &CodexModelBreakdown) -> bool {
    !name.trim().is_empty() && valid_nonnegative(model.cost)
}

fn valid_row_tokens(row: &CodexDailyRow) -> bool {
    categorized(
        row.input_tokens,
        row.output_tokens,
        row.reasoning_output_tokens,
    )
    .is_some_and(|tokens| row.total_tokens >= tokens)
}

fn categorized(input: u64, output: u64, reasoning: u64) -> Option<u64> {
    input.checked_add(output)?.checked_add(reasoning)
}

fn totals_match_rows(report: &CodexDailyReport) -> bool {
    sum_rows(&report.daily, |row| row.input_tokens) == Some(report.totals.input_tokens)
        && sum_rows(&report.daily, |row| row.output_tokens) == Some(report.totals.output_tokens)
        && sum_rows(&report.daily, |row| row.reasoning_output_tokens)
            == Some(report.totals.reasoning_output_tokens)
        && sum_rows(&report.daily, |row| row.total_tokens) == Some(report.totals.total_tokens)
}

fn sum_rows(rows: &[CodexDailyRow], value: impl Fn(&CodexDailyRow) -> u64) -> Option<u64> {
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

    const FIXTURES: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/collectors/ccusage/codex-daily/"
    );

    #[test]
    fn decodes_reviewed_valid_fixture() {
        let report = decode(fixture("valid.json")).expect("valid report");

        assert_eq!(report.daily.len(), 2);
        assert_eq!(report.daily[0].date, "2026-06-13");
        assert_eq!(report.daily[0].total_tokens, 1_650);
        assert_eq!(report.daily[0].models.len(), 2);
        assert_eq!(report.totals.total_tokens, 2_500);
    }

    #[test]
    fn accepts_empty_and_compatible_fixtures() {
        let empty = decode(fixture("empty.json")).expect("empty report");
        assert!(empty.daily.is_empty());
        assert_eq!(empty.totals.total_tokens, 0);
    }

    #[test]
    fn distinguishes_malformed_json_from_incompatible_envelopes() {
        assert_eq!(
            decode(fixture("invalid-json.json"))
                .expect_err("invalid json")
                .code,
            CollectorFailureCode::InvalidJson
        );

        let error =
            decode(fixture("incompatible-envelope.json")).expect_err("incompatible envelope");
        assert_eq!(error.code, CollectorFailureCode::IncompatibleEnvelope);
        assert_eq!(
            error.to_string(),
            "The collector returned incompatible output."
        );
    }

    fn fixture(name: &str) -> &'static str {
        match name {
            "valid.json" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../tests/fixtures/collectors/ccusage/codex-daily/valid.json"
            )),
            "empty.json" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../tests/fixtures/collectors/ccusage/codex-daily/empty.json"
            )),
            "invalid-json.json" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../tests/fixtures/collectors/ccusage/codex-daily/invalid-json.json"
            )),
            "incompatible-envelope.json" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../tests/fixtures/collectors/ccusage/codex-daily/incompatible-envelope.json"
            )),
            _ => panic!("unknown fixture under {FIXTURES}"),
        }
    }
}
