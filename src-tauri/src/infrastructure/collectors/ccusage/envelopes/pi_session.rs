use chrono::DateTime;
use serde::Deserialize;
use std::collections::HashSet;

use super::opencode_daily::TokenTotals;
use crate::application::collection::{CollectorFailure, CollectorFailureCode};

/// Pi session report.
///
/// Pi's ccusage `session` output mirrors the OpenCode-family token/cost shape but
/// names activity fields `firstActivity` / `lastActivity` (not
/// `firstActivityAt` / `lastActivityAt`) and never emits `modelBreakdowns`. Pi
/// also emits a `projectPath`; it is intentionally not modeled here so it is
/// ignored on decode and never persisted, consistent with the fixture privacy
/// harness and the OpenCode-family sessions.
#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PiSessionReport {
    pub sessions: Vec<PiSessionRow>,
    pub totals: TokenTotals,
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PiSessionRow {
    pub session_id: String,
    pub first_activity: Option<String>,
    pub last_activity: Option<String>,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_tokens: Option<u64>,
    pub cache_read_tokens: Option<u64>,
    pub total_tokens: u64,
    pub total_cost: f64,
    #[serde(default)]
    pub models_used: Vec<String>,
}

pub(crate) fn decode(input: &str) -> Result<PiSessionReport, CollectorFailure> {
    let report = serde_json::from_str::<PiSessionReport>(input).map_err(decode_failure)?;
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

fn validate(report: &PiSessionReport) -> Result<(), CollectorFailure> {
    if !valid_totals(&report.totals) || !totals_match_rows(report) {
        return Err(incompatible());
    }

    let mut session_ids = HashSet::with_capacity(report.sessions.len());
    for row in &report.sessions {
        if !session_ids.insert(row.session_id.as_str())
            || !valid_optional_timestamp(row.first_activity.as_deref())
            || !valid_optional_timestamp(row.last_activity.as_deref())
            || !valid_nonnegative(row.total_cost)
            || !valid_row_tokens(row)
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

fn valid_row_tokens(row: &PiSessionRow) -> bool {
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

fn totals_match_rows(report: &PiSessionReport) -> bool {
    sum_rows(&report.sessions, |row| row.input_tokens) == Some(report.totals.input_tokens)
        && sum_rows(&report.sessions, |row| row.output_tokens) == Some(report.totals.output_tokens)
        && sum_rows(&report.sessions, |row| row.total_tokens) == Some(report.totals.total_tokens)
}

fn sum_rows(rows: &[PiSessionRow], value: impl Fn(&PiSessionRow) -> u64) -> Option<u64> {
    rows.iter()
        .try_fold(0_u64, |total, row| total.checked_add(value(row)))
}

fn valid_nonnegative(value: f64) -> bool {
    value.is_finite() && value >= 0.0
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
        "/../tests/fixtures/collectors/ccusage/pi-session/"
    );

    #[test]
    fn decodes_reviewed_valid_fixture() {
        let report = decode(fixture("valid.json")).expect("valid report");

        assert_eq!(report.sessions.len(), 1);
        assert_eq!(report.sessions[0].session_id, "session-1");
        assert_eq!(
            report.sessions[0].first_activity.as_deref(),
            Some("2026-07-01T00:57:01.464Z")
        );
        assert_eq!(report.sessions[0].total_tokens, 550);
        assert_eq!(report.sessions[0].models_used, ["[pi] gpt-5.4-mini"]);
        assert_eq!(report.totals.total_tokens, 550);
    }

    #[test]
    fn accepts_empty_fixture() {
        let empty = decode(fixture("empty.json")).expect("empty report");
        assert!(empty.sessions.is_empty());
        assert_eq!(empty.totals.total_tokens, 0);
    }

    #[test]
    fn accepts_real_rows_without_activity_timestamps() {
        let report = decode(fixture("real-shape.json")).expect("real-shape report");

        assert_eq!(report.sessions.len(), 1);
        assert_eq!(report.sessions[0].first_activity, None);
        assert_eq!(report.sessions[0].last_activity, None);
        assert_eq!(report.sessions[0].cache_read_tokens, Some(400));
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
                "/../tests/fixtures/collectors/ccusage/pi-session/valid.json"
            )),
            "empty.json" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../tests/fixtures/collectors/ccusage/pi-session/empty.json"
            )),
            "invalid-json.json" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../tests/fixtures/collectors/ccusage/pi-session/invalid-json.json"
            )),
            "incompatible-envelope.json" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../tests/fixtures/collectors/ccusage/pi-session/incompatible-envelope.json"
            )),
            "real-shape.json" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../tests/fixtures/collectors/ccusage/pi-session/real-shape.json"
            )),
            _ => panic!("unknown fixture under {FIXTURES}"),
        }
    }
}
