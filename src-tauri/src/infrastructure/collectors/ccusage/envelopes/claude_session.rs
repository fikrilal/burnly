use chrono::DateTime;
use serde::Deserialize;
use std::collections::HashSet;

use super::claude_daily::{ModelBreakdown, TokenTotals};
use crate::application::collection::{CollectorFailure, CollectorFailureCode};

/// Claude Code session report from ccusage 20.0.14.
///
/// Real sidecar rows use `firstActivity` / `lastActivity` (not
/// `firstActivityAt` / `lastActivityAt`) and optional `projectPath` string.
/// Legacy `*At` names remain accepted via serde aliases for older fixtures.
#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ClaudeSessionReport {
    pub sessions: Vec<ClaudeSessionRow>,
    pub totals: TokenTotals,
    #[serde(default)]
    projects: Option<serde::de::IgnoredAny>,
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ClaudeSessionRow {
    pub session_id: String,
    #[serde(alias = "firstActivityAt")]
    pub first_activity: String,
    #[serde(alias = "lastActivityAt")]
    pub last_activity: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cache_read_tokens: u64,
    pub total_tokens: u64,
    pub total_cost: f64,
    pub models_used: Vec<String>,
    #[serde(default)]
    pub model_breakdowns: Vec<ModelBreakdown>,
    pub credits: Option<f64>,
    /// Real ccusage field; fixture files must omit this key (privacy harness).
    #[serde(default)]
    pub project_path: Option<String>,
}

pub(crate) fn decode(input: &str) -> Result<ClaudeSessionReport, CollectorFailure> {
    let report = serde_json::from_str::<ClaudeSessionReport>(input).map_err(decode_failure)?;
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

fn validate(report: &ClaudeSessionReport) -> Result<(), CollectorFailure> {
    if report.projects.is_some() || !valid_totals(&report.totals) || !totals_match_rows(report) {
        return Err(incompatible());
    }

    let mut session_ids = HashSet::with_capacity(report.sessions.len());
    for row in &report.sessions {
        if !session_ids.insert(row.session_id.as_str())
            || DateTime::parse_from_rfc3339(&row.first_activity).is_err()
            || DateTime::parse_from_rfc3339(&row.last_activity).is_err()
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

fn categorized_tokens(row: &ClaudeSessionRow) -> Option<u64> {
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

fn totals_match_rows(report: &ClaudeSessionReport) -> bool {
    sum_rows(&report.sessions, |row| row.input_tokens) == Some(report.totals.input_tokens)
        && sum_rows(&report.sessions, |row| row.output_tokens) == Some(report.totals.output_tokens)
        && sum_rows(&report.sessions, |row| row.cache_creation_tokens)
            == Some(report.totals.cache_creation_tokens)
        && sum_rows(&report.sessions, |row| row.cache_read_tokens)
            == Some(report.totals.cache_read_tokens)
        && sum_rows(&report.sessions, |row| row.total_tokens) == Some(report.totals.total_tokens)
}

fn sum_rows(rows: &[ClaudeSessionRow], value: impl Fn(&ClaudeSessionRow) -> u64) -> Option<u64> {
    rows.iter()
        .try_fold(0_u64, |total, row| total.checked_add(value(row)))
}

fn unique_nonempty(models: &[String]) -> bool {
    let mut seen = HashSet::with_capacity(models.len());
    models
        .iter()
        .all(|model| !model.trim().is_empty() && seen.insert(model.as_str()))
}

fn breakdowns_match_models(row: &ClaudeSessionRow) -> bool {
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

    #[test]
    fn decodes_empty_fixture() {
        let report = decode(fixture("empty.json")).expect("empty report");
        assert!(report.sessions.is_empty());
        assert_eq!(report.totals.total_tokens, 0);
    }

    #[test]
    fn decodes_valid_fixture_with_real_activity_field_names() {
        let report = decode(fixture("valid.json")).expect("valid report");
        assert_eq!(report.sessions.len(), 1);
        assert_eq!(report.sessions[0].session_id, "session-1");
        assert_eq!(
            report.sessions[0].first_activity,
            "2026-06-13T10:00:00.000Z"
        );
        assert_eq!(report.sessions[0].last_activity, "2026-06-13T12:00:00.000Z");
        assert_eq!(report.sessions[0].total_tokens, 1_650);
        assert_eq!(report.sessions[0].model_breakdowns.len(), 2);
        assert_eq!(report.totals.total_tokens, 1_650);
    }

    #[test]
    fn decodes_real_shape_fixture_from_ccusage_20() {
        let report = decode(fixture("real-shape.json")).expect("real-shape report");
        assert_eq!(report.sessions.len(), 3);
        assert_eq!(report.sessions[0].session_id, "session-1");
        assert_eq!(
            report.sessions[0].first_activity,
            "2026-05-08T12:46:44.017Z"
        );
        assert_eq!(report.sessions[0].last_activity, "2026-05-08T14:49:34.353Z");
        assert!(report.sessions[0].project_path.is_none());
        assert_eq!(report.totals.total_tokens, 8_977_414);
    }

    #[test]
    fn accepts_legacy_activity_at_field_aliases() {
        let input = r#"{
          "sessions": [{
            "sessionId": "legacy-1",
            "firstActivityAt": "2026-06-13T10:00:00.000Z",
            "lastActivityAt": "2026-06-13T11:00:00.000Z",
            "inputTokens": 10,
            "outputTokens": 5,
            "cacheCreationTokens": 0,
            "cacheReadTokens": 0,
            "totalTokens": 15,
            "totalCost": 0.01,
            "modelsUsed": ["claude-sonnet-4"],
            "modelBreakdowns": []
          }],
          "totals": {
            "inputTokens": 10,
            "outputTokens": 5,
            "cacheCreationTokens": 0,
            "cacheReadTokens": 0,
            "totalTokens": 15,
            "totalCost": 0.01
          }
        }"#;
        let report = decode(input).expect("legacy aliases");
        assert_eq!(report.sessions[0].first_activity, "2026-06-13T10:00:00.000Z");
        assert_eq!(report.sessions[0].last_activity, "2026-06-13T11:00:00.000Z");
    }

    #[test]
    fn accepts_project_path_string_when_present() {
        let input = r#"{
          "sessions": [{
            "sessionId": "path-1",
            "firstActivity": "2026-06-13T10:00:00.000Z",
            "lastActivity": "2026-06-13T11:00:00.000Z",
            "inputTokens": 10,
            "outputTokens": 5,
            "cacheCreationTokens": 0,
            "cacheReadTokens": 0,
            "totalTokens": 15,
            "totalCost": 0.01,
            "modelsUsed": ["claude-sonnet-4"],
            "modelBreakdowns": [],
            "projectPath": "/tmp/sanitized-project"
          }],
          "totals": {
            "inputTokens": 10,
            "outputTokens": 5,
            "cacheCreationTokens": 0,
            "cacheReadTokens": 0,
            "totalTokens": 15,
            "totalCost": 0.01
          }
        }"#;
        let report = decode(input).expect("project path");
        assert_eq!(
            report.sessions[0].project_path.as_deref(),
            Some("/tmp/sanitized-project")
        );
    }

    #[test]
    fn accepts_empty_report_with_negative_zero_cost() {
        let input = r#"{
          "sessions": [],
          "totals": {
            "cacheCreationTokens": 0,
            "cacheReadTokens": 0,
            "inputTokens": 0,
            "outputTokens": 0,
            "totalCost": -0.0,
            "totalTokens": 0
          }
        }"#;
        let report = decode(input).expect("neg zero cost empty");
        assert!(report.sessions.is_empty());
    }

    #[test]
    fn distinguishes_malformed_json_from_incompatible_envelopes() {
        assert_eq!(
            decode(fixture("invalid-json.json"))
                .expect_err("invalid json")
                .code,
            CollectorFailureCode::InvalidJson
        );

        assert_eq!(
            decode(fixture("incompatible-envelope.json"))
                .expect_err("incompatible")
                .code,
            CollectorFailureCode::IncompatibleEnvelope
        );

        // Missing required activity fields (old bug shape without firstActivity*)
        let missing_activity = r#"{
          "sessions": [{
            "sessionId": "x",
            "inputTokens": 1,
            "outputTokens": 0,
            "cacheCreationTokens": 0,
            "cacheReadTokens": 0,
            "totalTokens": 1,
            "totalCost": 0.0,
            "modelsUsed": ["m"],
            "modelBreakdowns": []
          }],
          "totals": {
            "inputTokens": 1,
            "outputTokens": 0,
            "cacheCreationTokens": 0,
            "cacheReadTokens": 0,
            "totalTokens": 1,
            "totalCost": 0.0
          }
        }"#;
        assert_eq!(
            decode(missing_activity).expect_err("missing activity").code,
            CollectorFailureCode::IncompatibleEnvelope
        );
    }

    fn fixture(name: &str) -> &'static str {
        match name {
            "empty.json" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../tests/fixtures/collectors/ccusage/claude-session/empty.json"
            )),
            "valid.json" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../tests/fixtures/collectors/ccusage/claude-session/valid.json"
            )),
            "real-shape.json" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../tests/fixtures/collectors/ccusage/claude-session/real-shape.json"
            )),
            "incompatible-envelope.json" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../tests/fixtures/collectors/ccusage/claude-session/incompatible-envelope.json"
            )),
            "invalid-json.json" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../tests/fixtures/collectors/ccusage/claude-session/invalid-json.json"
            )),
            other => panic!("unknown fixture {other}"),
        }
    }
}
