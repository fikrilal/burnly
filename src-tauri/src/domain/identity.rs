//! Deterministic identity for canonical usage records.
//!
//! The same construction is shared by collector mapping and by reconciliation so
//! a daily fact has one stable source key and identity version across the
//! collection and persistence boundaries. Identity must never be reconstructed
//! independently at a second site.

use chrono::NaiveDate;
use thiserror::Error;

use crate::domain::source::SourceKey;

/// Identity-scheme version for daily usage source keys.
///
/// Bumping this is a reconciliation event: it invalidates previously persisted
/// daily identities for the affected source and requires rebuilding that
/// source's daily projection rather than silently mixing schemes.
pub(crate) const DAILY_IDENTITY_VERSION: u16 = 1;

/// Builds the deterministic source key for one daily usage record.
///
/// Identity is `source + usage_date + aggregation_timezone`, version-tagged. The
/// daily grain is one record per source, local date, and aggregation timezone;
/// model breakdowns are child records and are not part of this identity.
///
/// The aggregation timezone is part of the identity because a reporting-timezone
/// change re-buckets activity into different local dates. Distinct timezones
/// therefore produce distinct keys, and switching timezone is handled by
/// rebuilding the affected daily projection rather than overwriting in place.
pub(crate) fn daily_source_key(
    source: SourceKey,
    usage_date: NaiveDate,
    aggregation_timezone: &str,
) -> Result<String, IdentityError> {
    if aggregation_timezone.trim().is_empty() {
        return Err(IdentityError::EmptyAggregationTimezone);
    }

    Ok(format!(
        "{source}:daily:v{DAILY_IDENTITY_VERSION}:{aggregation_timezone}:{usage_date}",
        source = source.as_str(),
    ))
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub(crate) enum IdentityError {
    #[error("daily source key requires a non-empty aggregation timezone")]
    EmptyAggregationTimezone,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
    }

    #[test]
    fn builds_a_versioned_key_from_source_date_and_timezone() {
        let key = daily_source_key(SourceKey::ClaudeCode, date(2026, 6, 13), "Asia/Jakarta")
            .expect("valid key");

        assert_eq!(key, "claude-code:daily:v1:Asia/Jakarta:2026-06-13");
    }

    #[test]
    fn identical_inputs_produce_identical_keys() {
        let first =
            daily_source_key(SourceKey::ClaudeCode, date(2026, 6, 13), "UTC").expect("first key");
        let second =
            daily_source_key(SourceKey::ClaudeCode, date(2026, 6, 13), "UTC").expect("second key");

        assert_eq!(first, second);
    }

    #[test]
    fn differing_source_date_or_timezone_produce_distinct_keys() {
        let base =
            daily_source_key(SourceKey::ClaudeCode, date(2026, 6, 13), "UTC").expect("base key");
        let other_source =
            daily_source_key(SourceKey::Codex, date(2026, 6, 13), "UTC").expect("other source");
        let other_date =
            daily_source_key(SourceKey::ClaudeCode, date(2026, 6, 14), "UTC").expect("other date");
        let other_timezone =
            daily_source_key(SourceKey::ClaudeCode, date(2026, 6, 13), "Asia/Jakarta")
                .expect("other timezone");

        assert_ne!(base, other_source);
        assert_ne!(base, other_date);
        assert_ne!(base, other_timezone);
    }

    #[test]
    fn rejects_an_empty_aggregation_timezone() {
        assert_eq!(
            daily_source_key(SourceKey::ClaudeCode, date(2026, 6, 13), "   "),
            Err(IdentityError::EmptyAggregationTimezone)
        );
    }
}
