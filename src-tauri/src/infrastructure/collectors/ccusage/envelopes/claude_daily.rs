use chrono::NaiveDate;
use serde::Deserialize;
use std::collections::HashSet;

use crate::application::collection::{CollectorFailure, CollectorFailureCode};

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ClaudeDailyReport {
    pub daily: Vec<ClaudeDailyRow>,
    pub totals: TokenTotals,
    #[serde(default)]
    projects: Option<serde::de::IgnoredAny>,
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ClaudeDailyRow {
    pub date: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cache_read_tokens: u64,
    pub total_tokens: u64,
    pub total_cost: f64,
    pub models_used: Vec<String>,
    pub model_breakdowns: Vec<ModelBreakdown>,
    pub credits: Option<f64>,
    #[serde(default)]
    project: Option<serde::de::IgnoredAny>,
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModelBreakdown {
    pub model_name: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cache_read_tokens: u64,
    pub cost: f64,
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TokenTotals {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cache_read_tokens: u64,
    pub total_tokens: u64,
    pub total_cost: f64,
    pub credits: Option<f64>,
}

pub(crate) fn decode(input: &str) -> Result<ClaudeDailyReport, CollectorFailure> {
    let report = serde_json::from_str::<ClaudeDailyReport>(input).map_err(decode_failure)?;
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

fn validate(report: &ClaudeDailyReport) -> Result<(), CollectorFailure> {
    if report.projects.is_some() || !valid_totals(&report.totals) || !totals_match_rows(report) {
        return Err(incompatible());
    }

    let mut dates = HashSet::with_capacity(report.daily.len());
    for row in &report.daily {
        if row.project.is_some()
            || NaiveDate::parse_from_str(&row.date, "%Y-%m-%d").is_err()
            || !dates.insert(row.date.as_str())
            || !valid_nonnegative(row.total_cost)
            || !valid_optional_nonnegative(row.credits)
            || categorized_tokens(row).is_none_or(|tokens| row.total_tokens < tokens)
            || !unique_nonempty(&row.models_used)
            || row.model_breakdowns.iter().any(|model| !valid_model(model))
            || !breakdowns_match_models(row)
        {
            return Err(incompatible());
        }
    }

    Ok(())
}

fn valid_totals(totals: &TokenTotals) -> bool {
    valid_nonnegative(totals.total_cost)
        && valid_optional_nonnegative(totals.credits)
        && categorized(
            totals.input_tokens,
            totals.output_tokens,
            totals.cache_creation_tokens,
            totals.cache_read_tokens,
        )
        .is_some_and(|tokens| totals.total_tokens >= tokens)
}

fn valid_model(model: &ModelBreakdown) -> bool {
    !model.model_name.trim().is_empty() && valid_nonnegative(model.cost)
}

fn categorized_tokens(row: &ClaudeDailyRow) -> Option<u64> {
    categorized(
        row.input_tokens,
        row.output_tokens,
        row.cache_creation_tokens,
        row.cache_read_tokens,
    )
}

fn categorized(input: u64, output: u64, cache_creation: u64, cache_read: u64) -> Option<u64> {
    input
        .checked_add(output)?
        .checked_add(cache_creation)?
        .checked_add(cache_read)
}

fn totals_match_rows(report: &ClaudeDailyReport) -> bool {
    sum_rows(&report.daily, |row| row.input_tokens) == Some(report.totals.input_tokens)
        && sum_rows(&report.daily, |row| row.output_tokens) == Some(report.totals.output_tokens)
        && sum_rows(&report.daily, |row| row.cache_creation_tokens)
            == Some(report.totals.cache_creation_tokens)
        && sum_rows(&report.daily, |row| row.cache_read_tokens)
            == Some(report.totals.cache_read_tokens)
        && sum_rows(&report.daily, |row| row.total_tokens) == Some(report.totals.total_tokens)
}

fn sum_rows(rows: &[ClaudeDailyRow], value: impl Fn(&ClaudeDailyRow) -> u64) -> Option<u64> {
    rows.iter()
        .try_fold(0_u64, |total, row| total.checked_add(value(row)))
}

fn unique_nonempty(models: &[String]) -> bool {
    let mut seen = HashSet::with_capacity(models.len());
    models
        .iter()
        .all(|model| !model.trim().is_empty() && seen.insert(model.as_str()))
}

fn breakdowns_match_models(row: &ClaudeDailyRow) -> bool {
    let known = row
        .models_used
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let mut seen = HashSet::with_capacity(row.model_breakdowns.len());
    row.model_breakdowns.iter().all(|model| {
        known.contains(model.model_name.as_str()) && seen.insert(model.model_name.as_str())
    })
}

fn valid_nonnegative(value: f64) -> bool {
    value.is_finite() && value >= 0.0
}

fn valid_optional_nonnegative(value: Option<f64>) -> bool {
    value.is_none_or(valid_nonnegative)
}

fn incompatible() -> CollectorFailure {
    CollectorFailure::new(CollectorFailureCode::IncompatibleEnvelope, None, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURES: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/collectors/ccusage/claude-daily/"
    );

    #[test]
    fn decodes_reviewed_valid_fixture() {
        let report = decode(fixture("valid.json")).expect("valid report");

        assert_eq!(report.daily.len(), 2);
        assert_eq!(report.daily[0].date, "2026-06-13");
        assert_eq!(report.daily[0].total_tokens, 1_650);
        assert_eq!(report.daily[0].model_breakdowns.len(), 2);
        assert_eq!(report.totals.total_tokens, 2_500);
    }

    #[test]
    fn accepts_empty_and_additive_compatible_fixtures() {
        let empty = decode(fixture("empty.json")).expect("empty report");
        assert!(empty.daily.is_empty());
        assert_eq!(empty.totals.total_tokens, 0);

        let additive = decode(fixture("additive-fields.json")).expect("additive report");
        assert_eq!(additive.daily.len(), 1);
        assert_eq!(additive.daily[0].models_used, ["claude-sonnet-4"]);
    }

    #[test]
    fn distinguishes_malformed_json_from_incompatible_envelopes() {
        assert_eq!(
            decode(fixture("invalid-json.json"))
                .expect_err("invalid json")
                .code,
            CollectorFailureCode::InvalidJson
        );

        for name in [
            "incompatible-envelope.json",
            "invalid-date.json",
            "invalid-number.json",
        ] {
            let error = decode(fixture(name)).expect_err("incompatible envelope");
            assert_eq!(error.code, CollectorFailureCode::IncompatibleEnvelope);
            assert_eq!(
                error.to_string(),
                "The collector returned incompatible output."
            );
        }

        for input in [
            "{}",
            r#"{"daily":"not-an-array","totals":{}}"#,
            r#"{"daily":[],"projects":{},"totals":{"inputTokens":0,"outputTokens":0,"cacheCreationTokens":0,"cacheReadTokens":0,"totalTokens":0,"totalCost":0}}"#,
        ] {
            assert_eq!(
                decode(input).expect_err("incompatible shape").code,
                CollectorFailureCode::IncompatibleEnvelope
            );
        }
    }

    fn fixture(name: &str) -> &'static str {
        match name {
            "valid.json" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../tests/fixtures/collectors/ccusage/claude-daily/valid.json"
            )),
            "empty.json" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../tests/fixtures/collectors/ccusage/claude-daily/empty.json"
            )),
            "additive-fields.json" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../tests/fixtures/collectors/ccusage/claude-daily/additive-fields.json"
            )),
            "invalid-json.json" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../tests/fixtures/collectors/ccusage/claude-daily/invalid-json.json"
            )),
            "incompatible-envelope.json" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../tests/fixtures/collectors/ccusage/claude-daily/incompatible-envelope.json"
            )),
            "invalid-date.json" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../tests/fixtures/collectors/ccusage/claude-daily/invalid-date.json"
            )),
            "invalid-number.json" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../tests/fixtures/collectors/ccusage/claude-daily/invalid-number.json"
            )),
            _ => panic!("unknown fixture under {FIXTURES}"),
        }
    }
}
