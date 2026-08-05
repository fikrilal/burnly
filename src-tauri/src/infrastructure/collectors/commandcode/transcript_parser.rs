//! Command Code transcript parsing.
//!
//! Parses `projects/**/<session-id>.jsonl` transcripts into usage-only typed
//! records. Only top-level identity, timing, `usage`, `model`, and `effort`
//! fields are decoded; `message.content` is never deserialized. Malformed
//! lines, partial trailing lines, invalid timestamps, and invalid token counts
//! are skipped rather than failing the whole file.

use chrono::{DateTime, Utc};
use serde::Deserialize;

/// Result of parsing one transcript file.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ParsedTranscript {
    /// Session identity from the `type: session` record.
    pub(crate) session_id: String,
    /// Working directory from the session record.
    pub(crate) cwd: Option<String>,
    /// Session start timestamp from the session record.
    pub(crate) started_at: DateTime<Utc>,
    /// Per-message usage records (only usage-bearing assistant messages).
    pub(crate) usages: Vec<TranscriptUsage>,
}

/// One usage-bearing assistant message.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TranscriptUsage {
    /// File-scoped message id.
    pub(crate) message_id: String,
    /// Message timestamp (RFC 3339 UTC).
    pub(crate) timestamp: DateTime<Utc>,
    /// Provider-reported token usage.
    pub(crate) tokens: ParsedTokens,
    /// Raw `costUsd` value; conversion to micros happens in the mapper (Phase 3).
    pub(crate) cost_usd: Option<f64>,
    /// Full provider/model id, e.g. `deepseek/deepseek-v4-flash`.
    pub(crate) model: Option<String>,
    /// `low`, `medium`, or `max`.
    pub(crate) effort: Option<String>,
}

/// Token counts from one `usage` block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ParsedTokens {
    pub(crate) input: u64,
    pub(crate) output: u64,
    pub(crate) cache_read: u64,
    pub(crate) cache_write: u64,
}

/// How a transcript file was classified during parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TranscriptKind {
    /// New-format file (has `type: session`) with at least one usage record.
    NewFormatWithUsage,
    /// New-format file with no usage records yet.
    NewFormatNoUsage,
    /// Pre-1.11 flat-schema file; carries no usage and is skipped.
    Legacy,
}

/// Summary of a transcript parse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct TranscriptParseSummary {
    pub(crate) lines_read: u32,
    pub(crate) messages_seen: u32,
    pub(crate) usage_records: u32,
    pub(crate) lines_skipped: u32,
}

/// Parse a new-format transcript. Legacy files return `TranscriptKind::Legacy`.
pub(crate) fn parse_transcript(
    contents: &str,
) -> (
    TranscriptKind,
    Option<ParsedTranscript>,
    TranscriptParseSummary,
) {
    let mut summary = TranscriptParseSummary::default();
    let mut session_id = None;
    let mut cwd = None;
    let mut started_at = None;
    let mut usages = Vec::new();
    let mut saw_type_field = false;

    for raw_line in contents.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            summary.lines_skipped += 1;
            continue;
        }
        summary.lines_read += 1;

        // A trailing partial line from a live append fails JSON parsing; skip
        // it rather than failing the file.
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            summary.lines_skipped += 1;
            continue;
        };
        let Some(obj) = value.as_object() else {
            summary.lines_skipped += 1;
            continue;
        };

        if obj.contains_key("type") {
            saw_type_field = true;
        }
        match obj.get("type").and_then(|t| t.as_str()) {
            Some("session") => {
                if let Ok(record) = serde_json::from_value::<SessionRecord>(value.clone()) {
                    session_id = Some(record.id);
                    cwd = record.cwd;
                    if let Ok(ts) = parse_timestamp(&record.timestamp) {
                        started_at = Some(ts);
                    }
                }
            }
            Some("message") => {
                summary.messages_seen += 1;
                match serde_json::from_value::<MessageRecord>(value.clone()) {
                    Ok(record) => {
                        if let Some(usage) = record.usage {
                            match TranscriptUsage::try_from_record(
                                record.id,
                                &record.timestamp,
                                usage,
                                record.model,
                                record.effort,
                            ) {
                                Ok(usage) => {
                                    summary.usage_records += 1;
                                    usages.push(usage);
                                }
                                Err(_) => summary.lines_skipped += 1,
                            }
                        }
                    }
                    Err(_) => summary.lines_skipped += 1,
                }
            }
            _ => summary.lines_skipped += 1,
        }
    }

    // Flat records without a `type` field are the pre-1.11 legacy schema.
    if !saw_type_field {
        return (TranscriptKind::Legacy, None, summary);
    }

    let Some(session_id) = session_id else {
        // A new-format file must have a session record; without one it cannot
        // be attributed and is skipped.
        return (TranscriptKind::NewFormatNoUsage, None, summary);
    };

    let kind = if usages.is_empty() {
        TranscriptKind::NewFormatNoUsage
    } else {
        TranscriptKind::NewFormatWithUsage
    };

    (
        kind,
        Some(ParsedTranscript {
            session_id,
            cwd,
            started_at: started_at.unwrap_or_else(|| {
                usages
                    .iter()
                    .map(|usage| usage.timestamp)
                    .min()
                    .unwrap_or_else(Utc::now)
            }),
            usages,
        }),
        summary,
    )
}

impl TranscriptUsage {
    fn try_from_record(
        message_id: String,
        timestamp: &str,
        usage: UsageRecord,
        model: Option<String>,
        effort: Option<String>,
    ) -> Result<Self, UsageRecordError> {
        let timestamp =
            parse_timestamp(timestamp).map_err(|_| UsageRecordError::InvalidTimestamp)?;
        let tokens = ParsedTokens {
            input: non_negative_u64(usage.input_tokens)?,
            output: non_negative_u64(usage.output_tokens)?,
            cache_read: non_negative_u64(usage.cache_read_tokens)?,
            cache_write: non_negative_u64(usage.cache_write_tokens)?,
        };
        let cost_usd = usage
            .cost_usd
            .filter(|value| value.is_finite() && *value >= 0.0);

        Ok(Self {
            message_id,
            timestamp,
            tokens,
            cost_usd,
            model,
            effort,
        })
    }
}

fn parse_timestamp(value: &str) -> Result<DateTime<Utc>, ()> {
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|_| ())
}

fn non_negative_u64(value: i64) -> Result<u64, UsageRecordError> {
    u64::try_from(value).map_err(|_| UsageRecordError::NegativeTokenCount)
}

#[derive(Debug)]
enum UsageRecordError {
    InvalidTimestamp,
    NegativeTokenCount,
}

/// Top-level `type: session` record (allowed fields only).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionRecord {
    id: String,
    #[serde(default)]
    timestamp: String,
    #[serde(default)]
    cwd: Option<String>,
}

/// Top-level `type: message` record (allowed fields only; `content` omitted).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MessageRecord {
    id: String,
    #[serde(default)]
    timestamp: String,
    #[serde(default)]
    usage: Option<UsageRecord>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    effort: Option<String>,
}

/// Top-level `usage` block on assistant messages.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UsageRecord {
    #[serde(default)]
    input_tokens: i64,
    #[serde(default)]
    output_tokens: i64,
    #[serde(default)]
    cache_read_tokens: i64,
    #[serde(default)]
    cache_write_tokens: i64,
    #[serde(default)]
    cost_usd: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_TRANSCRIPT: &str = r#"{"type":"session","version":3,"id":"sess-1","timestamp":"2026-08-04T10:00:00Z","cwd":"/tmp/proj"}
{"type":"message","id":"m1","parentId":null,"timestamp":"2026-08-04T10:00:01Z","message":{"role":"user","content":[{"type":"text","text":"redacted"}]}}
{"type":"message","id":"m2","parentId":"m1","timestamp":"2026-08-04T10:00:02Z","message":{"role":"assistant","content":[{"type":"text","text":"redacted"}]},"usage":{"inputTokens":10,"outputTokens":2,"cacheReadTokens":3,"cacheWriteTokens":0,"costUsd":0.001},"model":"deepseek/deepseek-v4-flash","effort":"max"}"#;

    #[test]
    fn parses_valid_transcript_into_usage_records() {
        let (kind, parsed, summary) = parse_transcript(VALID_TRANSCRIPT);

        assert_eq!(kind, TranscriptKind::NewFormatWithUsage);
        let parsed = parsed.expect("parsed");
        assert_eq!(parsed.session_id, "sess-1");
        assert_eq!(parsed.cwd.as_deref(), Some("/tmp/proj"));
        assert_eq!(parsed.usages.len(), 1);
        assert_eq!(parsed.usages[0].message_id, "m2");
        assert_eq!(parsed.usages[0].tokens.input, 10);
        assert_eq!(parsed.usages[0].tokens.output, 2);
        assert_eq!(parsed.usages[0].tokens.cache_read, 3);
        assert_eq!(parsed.usages[0].tokens.cache_write, 0);
        assert_eq!(parsed.usages[0].cost_usd, Some(0.001));
        assert_eq!(
            parsed.usages[0].model.as_deref(),
            Some("deepseek/deepseek-v4-flash")
        );
        assert_eq!(parsed.usages[0].effort.as_deref(), Some("max"));
        assert_eq!(summary.usage_records, 1);
    }

    #[test]
    fn ignores_non_usage_messages() {
        let (kind, parsed, summary) = parse_transcript(VALID_TRANSCRIPT);

        assert_eq!(kind, TranscriptKind::NewFormatWithUsage);
        let parsed = parsed.expect("parsed");
        assert_eq!(parsed.usages.len(), 1);
        assert_eq!(summary.messages_seen, 2);
    }

    #[test]
    fn classifies_legacy_transcript() {
        let legacy = r#"{"id":"legacy-1","timestamp":"2026-05-07T03:23:01Z","sessionId":"sess-legacy","parentId":null,"role":"user","content":[{"type":"text","text":"redacted"}]}"#;

        let (kind, parsed, _) = parse_transcript(legacy);

        assert_eq!(kind, TranscriptKind::Legacy);
        assert!(parsed.is_none());
    }

    #[test]
    fn classifies_new_format_without_usage() {
        let no_usage = r#"{"type":"session","version":3,"id":"sess-2","timestamp":"2026-08-04T10:00:00Z","cwd":"/tmp/proj"}
{"type":"message","id":"m1","parentId":null,"timestamp":"2026-08-04T10:00:01Z","message":{"role":"user","content":[{"type":"text","text":"redacted"}]}}"#;

        let (kind, parsed, _) = parse_transcript(no_usage);

        assert_eq!(kind, TranscriptKind::NewFormatNoUsage);
        let parsed = parsed.expect("parsed");
        assert!(parsed.usages.is_empty());
    }

    #[test]
    fn tolerates_partial_trailing_line() {
        let contents = format!("{VALID_TRANSCRIPT}\n{{\"type\":\"message\"");

        let (kind, parsed, summary) = parse_transcript(&contents);

        assert_eq!(kind, TranscriptKind::NewFormatWithUsage);
        let parsed = parsed.expect("parsed");
        assert_eq!(parsed.usages.len(), 1);
        assert_eq!(summary.lines_skipped, 1);
    }

    #[test]
    fn rejects_negative_token_counts() {
        let negative = r#"{"type":"session","version":3,"id":"sess-3","timestamp":"2026-08-04T10:00:00Z","cwd":"/tmp/proj"}
{"type":"message","id":"m1","parentId":null,"timestamp":"2026-08-04T10:00:01Z","message":{"role":"assistant","content":[]},"usage":{"inputTokens":-5,"outputTokens":2,"cacheReadTokens":0,"cacheWriteTokens":0,"costUsd":0.001},"model":"m","effort":"max"}"#;

        let (kind, parsed, summary) = parse_transcript(negative);

        assert_eq!(kind, TranscriptKind::NewFormatNoUsage);
        let parsed = parsed.expect("parsed");
        assert!(parsed.usages.is_empty());
        assert_eq!(summary.lines_skipped, 1);
    }

    #[test]
    fn rejects_token_counts_beyond_i64_range() {
        // 2^64 exceeds the JSON i64 field range; serde fails to decode the
        // usage block, so the line is skipped.
        let overflow = r#"{"type":"session","version":3,"id":"sess-4","timestamp":"2026-08-04T10:00:00Z","cwd":"/tmp/proj"}
{"type":"message","id":"m1","parentId":null,"timestamp":"2026-08-04T10:00:01Z","message":{"role":"assistant","content":[]},"usage":{"inputTokens":18446744073709551616,"outputTokens":2,"cacheReadTokens":0,"cacheWriteTokens":0,"costUsd":0.001},"model":"m","effort":"max"}"#;

        let (kind, _, summary) = parse_transcript(overflow);

        assert_eq!(kind, TranscriptKind::NewFormatNoUsage);
        assert_eq!(summary.lines_skipped, 1);
    }

    #[test]
    fn accepts_large_but_in_range_token_counts() {
        let large = format!(
            r#"{{"type":"session","version":3,"id":"sess-4b","timestamp":"2026-08-04T10:00:00Z","cwd":"/tmp/proj"}}
{{"type":"message","id":"m1","parentId":null,"timestamp":"2026-08-04T10:00:01Z","message":{{"role":"assistant","content":[]}},"usage":{{"inputTokens":{},"outputTokens":2,"cacheReadTokens":0,"cacheWriteTokens":0,"costUsd":0.001}},"model":"m","effort":"max"}}"#,
            i64::MAX
        );

        let (kind, parsed, _) = parse_transcript(&large);

        assert_eq!(kind, TranscriptKind::NewFormatWithUsage);
        let parsed = parsed.expect("parsed");
        assert_eq!(parsed.usages[0].tokens.input, i64::MAX as u64);
    }

    #[test]
    fn rejects_invalid_timestamp() {
        let invalid_ts = r#"{"type":"session","version":3,"id":"sess-5","timestamp":"2026-08-04T10:00:00Z","cwd":"/tmp/proj"}
{"type":"message","id":"m1","parentId":null,"timestamp":"not-a-timestamp","message":{"role":"assistant","content":[]},"usage":{"inputTokens":5,"outputTokens":2,"cacheReadTokens":0,"cacheWriteTokens":0,"costUsd":0.001},"model":"m","effort":"max"}"#;

        let (kind, parsed, summary) = parse_transcript(invalid_ts);

        assert_eq!(kind, TranscriptKind::NewFormatNoUsage);
        let parsed = parsed.expect("parsed");
        assert!(parsed.usages.is_empty());
        assert_eq!(summary.lines_skipped, 1);
    }

    #[test]
    fn parses_multiple_usage_records_in_one_file() {
        let multi = r#"{"type":"session","version":3,"id":"sess-6","timestamp":"2026-08-04T10:00:00Z","cwd":"/tmp/proj"}
{"type":"message","id":"m1","parentId":null,"timestamp":"2026-08-04T10:00:01Z","message":{"role":"assistant","content":[]},"usage":{"inputTokens":10,"outputTokens":2,"cacheReadTokens":0,"cacheWriteTokens":0,"costUsd":0.001},"model":"m","effort":"max"}
{"type":"message","id":"m2","parentId":"m1","timestamp":"2026-08-04T10:01:00Z","message":{"role":"assistant","content":[]},"usage":{"inputTokens":20,"outputTokens":4,"cacheReadTokens":0,"cacheWriteTokens":0,"costUsd":0.002},"model":"m","effort":"max"}"#;

        let (kind, parsed, summary) = parse_transcript(multi);

        assert_eq!(kind, TranscriptKind::NewFormatWithUsage);
        let parsed = parsed.expect("parsed");
        assert_eq!(parsed.usages.len(), 2);
        assert_eq!(summary.usage_records, 2);
    }

    #[test]
    fn started_at_falls_back_to_earliest_usage_when_session_timestamp_invalid() {
        let bad_session_ts = r#"{"type":"session","version":3,"id":"sess-7","timestamp":"garbage","cwd":"/tmp/proj"}
{"type":"message","id":"m1","parentId":null,"timestamp":"2026-08-04T10:00:01Z","message":{"role":"assistant","content":[]},"usage":{"inputTokens":10,"outputTokens":2,"cacheReadTokens":0,"cacheWriteTokens":0,"costUsd":0.001},"model":"m","effort":"max"}"#;

        let (_, parsed, _) = parse_transcript(bad_session_ts);

        let parsed = parsed.expect("parsed");
        assert_eq!(
            parsed.started_at,
            DateTime::parse_from_rfc3339("2026-08-04T10:00:01Z")
                .expect("ts")
                .with_timezone(&Utc)
        );
    }
}
