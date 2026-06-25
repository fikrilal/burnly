use chrono::DateTime;
use serde::Deserialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

use super::codex_daily::{CodexModelBreakdown, CodexTokenTotals};
use crate::application::collection::{CollectorFailure, CollectorFailureCode};

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexSessionReport {
    pub sessions: Vec<CodexSessionRow>,
    pub totals: CodexTokenTotals,
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexSessionRow {
    pub session_id: String,
    pub first_activity_at: Option<String>,
    pub last_activity_at: Option<String>,
    pub last_activity: Option<Value>,
    pub session_file: String,
    pub directory: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_tokens: Option<u64>,
    pub cache_read_tokens: Option<u64>,
    pub reasoning_output_tokens: u64,
    pub total_tokens: u64,
    #[serde(alias = "costUSD")]
    pub total_cost: f64,
    pub models: HashMap<String, CodexModelBreakdown>,
}

pub(crate) fn decode(input: &str) -> Result<CodexSessionReport, CollectorFailure> {
    let report = serde_json::from_str::<CodexSessionReport>(input).map_err(decode_failure)?;
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

fn validate(report: &CodexSessionReport) -> Result<(), CollectorFailure> {
    if !valid_totals(&report.totals) || !totals_match_rows(report) {
        return Err(incompatible());
    }

    let mut session_ids = HashSet::with_capacity(report.sessions.len());
    for row in &report.sessions {
        if !session_ids.insert(row.session_id.as_str())
            || !valid_optional_timestamp(row.first_activity_at.as_deref())
            || !valid_optional_timestamp(last_activity_timestamp(row).as_deref())
            || !valid_nonnegative(row.total_cost)
            || !valid_row_tokens(row)
            || row
                .models
                .iter()
                .any(|(name, model)| !valid_model(name, model))
            || row.session_file.trim().is_empty()
            || row.directory.trim().is_empty()
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
            totals.cache_creation_tokens,
            totals.cache_read_tokens,
        )
        .is_some_and(|tokens| totals.total_tokens >= tokens)
}

fn valid_model(name: &str, model: &CodexModelBreakdown) -> bool {
    if name.trim().is_empty() || model.cost.is_some_and(|cost| !valid_nonnegative(cost)) {
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

fn valid_row_tokens(row: &CodexSessionRow) -> bool {
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

fn totals_match_rows(report: &CodexSessionReport) -> bool {
    sum_rows(&report.sessions, |row| row.input_tokens) == Some(report.totals.input_tokens)
        && sum_rows(&report.sessions, |row| row.output_tokens) == Some(report.totals.output_tokens)
        && sum_rows(&report.sessions, |row| row.reasoning_output_tokens)
            == Some(report.totals.reasoning_output_tokens)
        && sum_rows(&report.sessions, |row| row.total_tokens) == Some(report.totals.total_tokens)
}

fn sum_rows(rows: &[CodexSessionRow], value: impl Fn(&CodexSessionRow) -> u64) -> Option<u64> {
    rows.iter()
        .try_fold(0_u64, |total, row| total.checked_add(value(row)))
}

fn valid_nonnegative(value: f64) -> bool {
    value.is_finite() && value >= 0.0
}

pub(crate) fn first_activity_timestamp(row: &CodexSessionRow) -> Option<&str> {
    row.first_activity_at.as_deref()
}

pub(crate) fn last_activity_timestamp(row: &CodexSessionRow) -> Option<String> {
    row.last_activity_at.clone().or_else(|| {
        row.last_activity
            .as_ref()
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
    })
}

fn valid_optional_timestamp(value: Option<&str>) -> bool {
    value.is_none_or(|timestamp| DateTime::parse_from_rfc3339(timestamp).is_ok())
}

fn incompatible() -> CollectorFailure {
    CollectorFailure::new(CollectorFailureCode::IncompatibleEnvelope, None, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURES: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/collectors/ccusage/codex-session/"
    );

    #[test]
    fn decodes_reviewed_valid_fixture() {
        let report = decode(fixture("valid.json")).expect("valid report");

        assert_eq!(report.sessions.len(), 1);
        assert_eq!(report.sessions[0].session_id, "session-1");
        assert_eq!(report.sessions[0].session_file, "session_1.json");
        assert_eq!(report.sessions[0].directory, "/tmp/burnly-fixture/project");
        assert_eq!(report.sessions[0].total_tokens, 1_650);
        assert_eq!(report.sessions[0].models.len(), 2);
        assert_eq!(report.totals.total_tokens, 1_650);
    }

    #[test]
    fn accepts_empty_and_compatible_fixtures() {
        let empty = decode(fixture("empty.json")).expect("empty report");
        assert!(empty.sessions.is_empty());
        assert_eq!(empty.totals.total_tokens, 0);
    }

    #[test]
    fn accepts_real_cost_aliases_missing_first_activity_and_last_activity_alias() {
        let report = decode(fixture("real-shape.json")).expect("real-shape report");

        assert_eq!(report.sessions.len(), 1);
        let row = &report.sessions[0];
        assert_eq!(row.total_cost, 12.34);
        assert_eq!(row.first_activity_at, None);
        assert_eq!(
            last_activity_timestamp(row).as_deref(),
            Some("2026-06-24T08:33:53.771Z")
        );
        let model = row.models.get("gpt-5.5").expect("model");
        assert_eq!(model.cost, None);
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
                "/../tests/fixtures/collectors/ccusage/codex-session/valid.json"
            )),
            "empty.json" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../tests/fixtures/collectors/ccusage/codex-session/empty.json"
            )),
            "invalid-json.json" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../tests/fixtures/collectors/ccusage/codex-session/invalid-json.json"
            )),
            "incompatible-envelope.json" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../tests/fixtures/collectors/ccusage/codex-session/incompatible-envelope.json"
            )),
            "real-shape.json" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../tests/fixtures/collectors/ccusage/codex-session/real-shape.json"
            )),
            _ => panic!("unknown fixture under {FIXTURES}"),
        }
    }
}
