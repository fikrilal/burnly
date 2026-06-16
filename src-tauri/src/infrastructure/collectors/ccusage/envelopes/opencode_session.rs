use chrono::DateTime;
use serde::Deserialize;
use std::collections::HashSet;

use super::opencode_daily::{ModelBreakdown, TokenTotals};
use crate::application::collection::{CollectorFailure, CollectorFailureCode};

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OpenCodeSessionReport {
    pub sessions: Vec<OpenCodeSessionRow>,
    pub totals: TokenTotals,
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OpenCodeSessionRow {
    pub session_id: String,
    pub first_activity_at: String,
    pub last_activity_at: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub total_cost: f64,
    #[serde(default)]
    pub models_used: Vec<String>,
    #[serde(default)]
    pub model_breakdowns: Vec<ModelBreakdown>,
}

pub(crate) fn decode(input: &str) -> Result<OpenCodeSessionReport, CollectorFailure> {
    let report = serde_json::from_str::<OpenCodeSessionReport>(input).map_err(decode_failure)?;
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

fn validate(report: &OpenCodeSessionReport) -> Result<(), CollectorFailure> {
    if !valid_totals(&report.totals) || !totals_match_rows(report) {
        return Err(incompatible());
    }

    let mut session_ids = HashSet::with_capacity(report.sessions.len());
    for row in &report.sessions {
        if !session_ids.insert(row.session_id.as_str())
            || DateTime::parse_from_rfc3339(&row.first_activity_at).is_err()
            || DateTime::parse_from_rfc3339(&row.last_activity_at).is_err()
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
        && totals
            .input_tokens
            .checked_add(totals.output_tokens)
            .is_some_and(|tokens| totals.total_tokens >= tokens)
}

fn valid_model(model: &ModelBreakdown) -> bool {
    !model.model_name.trim().is_empty() && valid_nonnegative(model.cost)
}

fn valid_row_tokens(row: &OpenCodeSessionRow) -> bool {
    row.input_tokens
        .checked_add(row.output_tokens)
        .is_some_and(|tokens| row.total_tokens >= tokens)
}

fn totals_match_rows(report: &OpenCodeSessionReport) -> bool {
    sum_rows(&report.sessions, |row| row.input_tokens) == Some(report.totals.input_tokens)
        && sum_rows(&report.sessions, |row| row.output_tokens) == Some(report.totals.output_tokens)
        && sum_rows(&report.sessions, |row| row.total_tokens) == Some(report.totals.total_tokens)
}

fn sum_rows(
    rows: &[OpenCodeSessionRow],
    value: impl Fn(&OpenCodeSessionRow) -> u64,
) -> Option<u64> {
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
        "/../tests/fixtures/collectors/ccusage/opencode-session/"
    );

    #[test]
    fn decodes_reviewed_valid_fixture() {
        let report = decode(fixture("valid.json")).expect("valid report");

        assert_eq!(report.sessions.len(), 1);
        assert_eq!(report.sessions[0].session_id, "session-1");
        assert_eq!(report.sessions[0].total_tokens, 1_650);
        assert_eq!(report.sessions[0].model_breakdowns.len(), 1);
        assert_eq!(report.totals.total_tokens, 1_650);
    }

    #[test]
    fn accepts_empty_and_compatible_fixtures() {
        let empty = decode(fixture("empty.json")).expect("empty report");
        assert!(empty.sessions.is_empty());
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
                "/../tests/fixtures/collectors/ccusage/opencode-session/valid.json"
            )),
            "empty.json" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../tests/fixtures/collectors/ccusage/opencode-session/empty.json"
            )),
            "invalid-json.json" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../tests/fixtures/collectors/ccusage/opencode-session/invalid-json.json"
            )),
            "incompatible-envelope.json" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../tests/fixtures/collectors/ccusage/opencode-session/incompatible-envelope.json"
            )),
            _ => panic!("unknown fixture under {FIXTURES}"),
        }
    }
}
