use chrono::DateTime;
use serde::Deserialize;
use std::collections::HashSet;

use super::claude_daily::{ModelBreakdown, TokenTotals};
use crate::application::collection::{CollectorFailure, CollectorFailureCode};

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
    pub first_activity_at: String,
    pub last_activity_at: String,
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
    pub project: Option<ProjectRef>,
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectRef {
    pub path: String,
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
            || DateTime::parse_from_rfc3339(&row.first_activity_at).is_err()
            || DateTime::parse_from_rfc3339(&row.last_activity_at).is_err()
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
