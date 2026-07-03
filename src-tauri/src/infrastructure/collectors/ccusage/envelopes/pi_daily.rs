use serde::Deserialize;

use crate::application::collection::{CollectorFailure, CollectorFailureCode};

use super::opencode_daily::{OpenCodeDailyReport, OpenCodeDailyRow, TokenTotals};

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PiDailyReport {
    daily: Vec<OpenCodeDailyRow>,
    totals: Option<TokenTotals>,
}

pub(crate) fn decode(input: &str) -> Result<OpenCodeDailyReport, CollectorFailure> {
    let report = serde_json::from_str::<PiDailyReport>(input).map_err(decode_failure)?;
    let totals = match report.totals {
        Some(totals) => totals,
        None if report.daily.is_empty() => empty_totals(),
        None => return Err(incompatible()),
    };

    let report = OpenCodeDailyReport {
        daily: report.daily,
        totals,
    };
    super::opencode_daily::validate(&report)?;
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

fn incompatible() -> CollectorFailure {
    CollectorFailure::new(CollectorFailureCode::IncompatibleEnvelope, None, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_empty_pi_daily_output_with_null_totals() {
        let report = decode(
            r#"{
              "daily": [],
              "totals": null
            }"#,
        )
        .expect("empty pi report");

        assert!(report.daily.is_empty());
        assert_eq!(report.totals.total_tokens, 0);
        assert_eq!(report.totals.total_cost, 0.0);
    }

    #[test]
    fn accepts_non_empty_pi_daily_output_with_totals() {
        let report = decode(
            r#"{
              "daily": [
                {
                  "date": "2026-07-03",
                  "inputTokens": 10,
                  "outputTokens": 5,
                  "cacheCreationTokens": 2,
                  "cacheReadTokens": 3,
                  "totalTokens": 20,
                  "totalCost": 0.01,
                  "modelsUsed": ["[pi] gpt-5.4-mini"],
                  "modelBreakdowns": [
                    {
                      "modelName": "[pi] gpt-5.4-mini",
                      "inputTokens": 10,
                      "outputTokens": 5,
                      "cacheCreationTokens": 2,
                      "cacheReadTokens": 3,
                      "totalTokens": 20,
                      "cost": 0.01
                    }
                  ]
                }
              ],
              "totals": {
                "inputTokens": 10,
                "outputTokens": 5,
                "cacheCreationTokens": 2,
                "cacheReadTokens": 3,
                "totalTokens": 20,
                "totalCost": 0.01
              }
            }"#,
        )
        .expect("non-empty pi report");

        assert_eq!(report.daily.len(), 1);
        assert_eq!(report.totals.total_tokens, 20);
    }

    #[test]
    fn rejects_non_empty_pi_daily_output_with_null_totals() {
        let error = decode(
            r#"{
              "daily": [
                {
                  "date": "2026-07-03",
                  "inputTokens": 10,
                  "outputTokens": 5,
                  "totalTokens": 15,
                  "totalCost": 0.01
                }
              ],
              "totals": null
            }"#,
        )
        .expect_err("incompatible pi report");

        assert_eq!(error.code, CollectorFailureCode::IncompatibleEnvelope);
    }
}
