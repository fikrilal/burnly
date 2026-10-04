//! Usage-only DeepSeek Harness event parsing.
//!
//! The parser consumes the post-header JSONL bytes produced by the bounded
//! session-log reader and extracts only usage, route, and retry-boundary
//! events. Unknown event types and content-bearing fields are never
//! deserialized into Burnly-owned values.

#![allow(
    dead_code,
    reason = "event parser is wired into the collector in a later chunk"
)]

use serde::Deserialize;
use thiserror::Error;

use crate::domain::usage::{TokenUsage, UsageValidationError};

pub(crate) const REJECTION_INVALID_JSON: &str = "deepseek_harness.event_invalid_json";
pub(crate) const REJECTION_INVALID_EVENT: &str = "deepseek_harness.event_invalid_event";
pub(crate) const REJECTION_INVALID_USAGE: &str = "deepseek_harness.event_invalid_usage";
pub(crate) const REJECTION_SEQUENCE_GAP: &str = "deepseek_harness.event_sequence_gap";

/// A provider/model route observed on an assistant message or request context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionModelRoute {
    pub(crate) provider: String,
    pub(crate) model: String,
}

/// Which DSH settlement produced one usage sample.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UsageSettlementKind {
    AssistantMessage,
    AssistantAttempt,
}

/// One normalized usage sample selected from an assistant settlement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UsageEvent {
    pub(crate) seq: u64,
    pub(crate) observed_at_ms: i64,
    pub(crate) turn: u64,
    pub(crate) step: u64,
    pub(crate) kind: UsageSettlementKind,
    pub(crate) route: Option<SessionModelRoute>,
    pub(crate) tokens: TokenUsage,
}

/// Fold-relevant events in durable order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SessionEvent {
    Usage(UsageEvent),
    Route(SessionModelRoute),
    RetryStarted { turn: u64, step: u64 },
}

/// One rejected event or usage sample. Rejections carry no raw payload data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EventRejection {
    pub(crate) seq: Option<u64>,
    pub(crate) code: &'static str,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct ParseOutcome {
    pub(crate) events: Vec<SessionEvent>,
    pub(crate) rejections: Vec<EventRejection>,
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UsageNormalizationError {
    #[error("input and output token counts are required")]
    MissingRequired,

    #[error("cache read and cache write token counts are required when the total is absent")]
    MissingCacheBuckets,

    #[error("total token count is invalid")]
    InvalidTotal,

    #[error("token count overflowed")]
    Overflow,

    #[error("reasoning tokens exceed output tokens")]
    InvalidReasoning,
}

/// Parse post-header DeepSeek Harness JSONL into fold-relevant events.
pub(crate) fn parse_session_events(events_jsonl: &[u8]) -> ParseOutcome {
    let mut outcome = ParseOutcome::default();
    let mut last_seq: Option<u64> = None;

    for chunk in events_jsonl.split_inclusive(|byte| *byte == b'\n') {
        let Some(line) = complete_line(chunk) else {
            continue;
        };
        if line.is_empty() {
            continue;
        }

        let probe = match serde_json::from_slice::<EventTypeProbe>(line) {
            Ok(probe) => probe,
            Err(_) => {
                outcome.rejections.push(EventRejection {
                    seq: None,
                    code: REJECTION_INVALID_JSON,
                });
                continue;
            }
        };

        if let Some(seq) = probe.seq {
            if let Some(previous) = last_seq {
                if seq != previous.saturating_add(1) {
                    outcome.rejections.push(EventRejection {
                        seq: Some(seq),
                        code: REJECTION_SEQUENCE_GAP,
                    });
                }
            }
            last_seq = Some(seq);
        }

        match probe.kind.as_str() {
            "assistant/message" => parse_assistant_message(line, probe.seq, &mut outcome),
            "assistant/attempt" => parse_assistant_attempt(line, probe.seq, &mut outcome),
            "request/context" => parse_request_context(line, probe.seq, &mut outcome),
            "llm/retry-started" => parse_retry_started(line, probe.seq, &mut outcome),
            _ => {}
        }
    }

    outcome
}

fn complete_line(chunk: &[u8]) -> Option<&[u8]> {
    let line = chunk.strip_suffix(b"\n")?;
    Some(line.strip_suffix(b"\r").unwrap_or(line))
}

#[derive(Debug, Deserialize)]
struct EventTypeProbe {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    seq: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawAssistantMessageEvent {
    seq: u64,
    time: i64,
    data: RawAssistantMessageData,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawAssistantMessageData {
    turn: u64,
    step: u64,
    message: RawAssistantMessageBody,
    #[serde(default)]
    usage: Option<RawTokenUsage>,
    #[serde(default)]
    stream: Option<Vec<RawStreamRecord>>,
}

#[derive(Debug, Deserialize)]
struct RawAssistantMessageBody {
    #[serde(default)]
    source: Option<RawMessageSource>,
}

#[derive(Debug, Deserialize)]
struct RawMessageSource {
    #[serde(default)]
    provider: Option<String>,
    #[serde(default)]
    model: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawAssistantAttemptEvent {
    seq: u64,
    time: i64,
    data: RawAssistantAttemptData,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawAssistantAttemptData {
    turn: u64,
    step: u64,
    #[serde(default)]
    stream: Option<Vec<RawStreamRecord>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawRequestContextEvent {
    seq: u64,
    time: i64,
    data: RawRequestContextData,
}

#[derive(Debug, Deserialize)]
struct RawRequestContextData {
    provider: String,
    model: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawRetryStartedEvent {
    seq: u64,
    time: i64,
    data: RawRetryStartedData,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawRetryStartedData {
    turn: u64,
    step: u64,
}

#[derive(Debug, Deserialize)]
struct RawStreamRecord {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    chunk: Option<RawStreamChunk>,
}

#[derive(Debug, Deserialize)]
struct RawStreamChunk {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    usage: Option<RawTokenUsage>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawTokenUsage {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    total_tokens: Option<u64>,
    cache_read_tokens: Option<u64>,
    cache_write_tokens: Option<u64>,
    reasoning_tokens: Option<u64>,
}

fn parse_assistant_message(line: &[u8], probe_seq: Option<u64>, outcome: &mut ParseOutcome) {
    let event: RawAssistantMessageEvent = match serde_json::from_slice(line) {
        Ok(event) => event,
        Err(_) => return reject_invalid_event(outcome, probe_seq),
    };
    if event.time < 0 {
        return reject_invalid_event(outcome, Some(event.seq));
    }

    let route = message_route(&event.data);
    let sample = event
        .data
        .usage
        .as_ref()
        .or_else(|| last_stream_usage(event.data.stream.as_deref()));
    let Some(sample) = sample else {
        return;
    };

    match normalize_usage(sample) {
        Ok(tokens) => outcome.events.push(SessionEvent::Usage(UsageEvent {
            seq: event.seq,
            observed_at_ms: event.time,
            turn: event.data.turn,
            step: event.data.step,
            kind: UsageSettlementKind::AssistantMessage,
            route,
            tokens,
        })),
        Err(_) => outcome.rejections.push(EventRejection {
            seq: Some(event.seq),
            code: REJECTION_INVALID_USAGE,
        }),
    }
}

fn parse_assistant_attempt(line: &[u8], probe_seq: Option<u64>, outcome: &mut ParseOutcome) {
    let event: RawAssistantAttemptEvent = match serde_json::from_slice(line) {
        Ok(event) => event,
        Err(_) => return reject_invalid_event(outcome, probe_seq),
    };
    if event.time < 0 {
        return reject_invalid_event(outcome, Some(event.seq));
    }

    let Some(sample) = last_stream_usage(event.data.stream.as_deref()) else {
        return;
    };
    match normalize_usage(sample) {
        Ok(tokens) => outcome.events.push(SessionEvent::Usage(UsageEvent {
            seq: event.seq,
            observed_at_ms: event.time,
            turn: event.data.turn,
            step: event.data.step,
            kind: UsageSettlementKind::AssistantAttempt,
            route: None,
            tokens,
        })),
        Err(_) => outcome.rejections.push(EventRejection {
            seq: Some(event.seq),
            code: REJECTION_INVALID_USAGE,
        }),
    }
}

fn parse_request_context(line: &[u8], probe_seq: Option<u64>, outcome: &mut ParseOutcome) {
    let event: RawRequestContextEvent = match serde_json::from_slice(line) {
        Ok(event) => event,
        Err(_) => return reject_invalid_event(outcome, probe_seq),
    };
    if event.time < 0 {
        return reject_invalid_event(outcome, Some(event.seq));
    }

    let provider = event.data.provider.trim();
    let model = event.data.model.trim();
    if provider.is_empty() || model.is_empty() {
        return reject_invalid_event(outcome, Some(event.seq));
    }

    outcome.events.push(SessionEvent::Route(SessionModelRoute {
        provider: provider.to_owned(),
        model: model.to_owned(),
    }));
}

fn parse_retry_started(line: &[u8], probe_seq: Option<u64>, outcome: &mut ParseOutcome) {
    let event: RawRetryStartedEvent = match serde_json::from_slice(line) {
        Ok(event) => event,
        Err(_) => return reject_invalid_event(outcome, probe_seq),
    };
    if event.time < 0 {
        return reject_invalid_event(outcome, Some(event.seq));
    }

    outcome.events.push(SessionEvent::RetryStarted {
        turn: event.data.turn,
        step: event.data.step,
    });
}

fn reject_invalid_event(outcome: &mut ParseOutcome, seq: Option<u64>) {
    outcome.rejections.push(EventRejection {
        seq,
        code: REJECTION_INVALID_EVENT,
    });
}

fn message_route(data: &RawAssistantMessageData) -> Option<SessionModelRoute> {
    let source = data.message.source.as_ref()?;
    let provider = source.provider.as_deref()?.trim();
    let model = source.model.as_deref()?.trim();
    if provider.is_empty() || model.is_empty() {
        return None;
    }

    Some(SessionModelRoute {
        provider: provider.to_owned(),
        model: model.to_owned(),
    })
}

fn last_stream_usage(stream: Option<&[RawStreamRecord]>) -> Option<&RawTokenUsage> {
    stream?.iter().rev().find_map(|record| {
        if record.kind != "chunk" {
            return None;
        }
        let chunk = record.chunk.as_ref()?;
        if chunk.kind != "usage" {
            return None;
        }
        chunk.usage.as_ref()
    })
}

fn normalize_usage(raw: &RawTokenUsage) -> Result<TokenUsage, UsageNormalizationError> {
    let input = raw
        .input_tokens
        .ok_or(UsageNormalizationError::MissingRequired)?;
    let output = raw
        .output_tokens
        .ok_or(UsageNormalizationError::MissingRequired)?;

    if let Some(reasoning) = raw.reasoning_tokens {
        if reasoning > output {
            return Err(UsageNormalizationError::InvalidReasoning);
        }
    }

    let known_prompt = input
        .checked_add(raw.cache_read_tokens.unwrap_or(0))
        .and_then(|value| value.checked_add(raw.cache_write_tokens.unwrap_or(0)))
        .ok_or(UsageNormalizationError::Overflow)?;

    let total = match raw.total_tokens {
        Some(total) => {
            if total < output {
                return Err(UsageNormalizationError::InvalidTotal);
            }
            let prompt_tokens = total - output;
            if prompt_tokens < known_prompt {
                return Err(UsageNormalizationError::InvalidTotal);
            }
            if raw.cache_read_tokens.is_some()
                && raw.cache_write_tokens.is_some()
                && prompt_tokens != known_prompt
            {
                return Err(UsageNormalizationError::InvalidTotal);
            }
            total
        }
        None => {
            if raw.cache_read_tokens.is_none() || raw.cache_write_tokens.is_none() {
                return Err(UsageNormalizationError::MissingCacheBuckets);
            }
            known_prompt
                .checked_add(output)
                .ok_or(UsageNormalizationError::Overflow)?
        }
    };

    TokenUsage::new(
        Some(input),
        Some(output),
        raw.cache_write_tokens,
        raw.cache_read_tokens,
        total,
    )
    .map_err(|error| match error {
        UsageValidationError::TokenOverflow => UsageNormalizationError::Overflow,
        _ => UsageNormalizationError::InvalidTotal,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_HEADER_AND_EVENT: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/collectors/deepseek-harness/sessions/valid-root-session.jsonl"
    ));

    fn events_fixture() -> &'static str {
        VALID_HEADER_AND_EVENT
            .split_once('\n')
            .expect("fixture header")
            .1
    }

    fn parse(events: &str) -> ParseOutcome {
        let mut bytes = events.as_bytes().to_vec();
        if !bytes.ends_with(b"\n") {
            bytes.push(b'\n');
        }
        parse_session_events(&bytes)
    }

    #[test]
    fn parses_assistant_message_usage_and_route() {
        let outcome = parse(events_fixture());

        assert!(outcome.rejections.is_empty());
        assert_eq!(outcome.events.len(), 1);
        let SessionEvent::Usage(usage) = &outcome.events[0] else {
            panic!("expected usage event");
        };
        assert_eq!(usage.seq, 1);
        assert_eq!(usage.turn, 1);
        assert_eq!(usage.step, 1);
        assert_eq!(usage.tokens.total_tokens(), 12);
        assert_eq!(
            usage.route,
            Some(SessionModelRoute {
                provider: "deepseek-official".to_owned(),
                model: "deepseek-flash".to_owned(),
            })
        );
    }

    #[test]
    fn falls_back_to_last_stream_usage_chunk() {
        let events = r#"{"type":"assistant/message","seq":1,"time":100,"data":{"turn":1,"step":1,"message":{"source":{"provider":"provider","model":"model"}},"stream":[{"type":"chunk","chunk":{"type":"usage","usage":{"inputTokens":1,"outputTokens":1,"totalTokens":2}}},{"type":"chunk","chunk":{"type":"usage","usage":{"inputTokens":5,"outputTokens":3,"totalTokens":8}}}]}}"#;

        let outcome = parse(events);

        assert!(outcome.rejections.is_empty());
        assert_eq!(outcome.events.len(), 1);
        let SessionEvent::Usage(usage) = &outcome.events[0] else {
            panic!("expected usage event");
        };
        assert_eq!(usage.tokens.total_tokens(), 8);
    }

    #[test]
    fn derives_total_only_when_both_cache_buckets_are_present() {
        let missing_one_cache = r#"{"type":"assistant/message","seq":1,"time":100,"data":{"turn":1,"step":1,"message":{"source":{"provider":"provider","model":"model"}},"stream":[],"usage":{"inputTokens":5,"outputTokens":3,"cacheReadTokens":2}}}"#;
        let both_caches = r#"{"type":"assistant/message","seq":2,"time":101,"data":{"turn":1,"step":1,"message":{"source":{"provider":"provider","model":"model"}},"stream":[],"usage":{"inputTokens":5,"outputTokens":3,"cacheReadTokens":2,"cacheWriteTokens":0}}}"#;

        let missing = parse(missing_one_cache);
        assert_eq!(missing.events.len(), 0);
        assert_eq!(missing.rejections.len(), 1);
        assert_eq!(missing.rejections[0].code, REJECTION_INVALID_USAGE);

        let derived = parse(both_caches);
        assert!(derived.rejections.is_empty());
        let SessionEvent::Usage(usage) = &derived.events[0] else {
            panic!("expected usage event");
        };
        assert_eq!(usage.tokens.total_tokens(), 10);
    }

    #[test]
    fn rejects_invalid_usage_without_dropping_later_valid_usage() {
        let events = r#"{"type":"assistant/message","seq":1,"time":100,"data":{"turn":1,"step":1,"message":{"source":{"provider":"provider","model":"model"}},"stream":[],"usage":{"inputTokens":5,"outputTokens":5,"totalTokens":9}}}
{"type":"assistant/message","seq":2,"time":101,"data":{"turn":1,"step":2,"message":{"source":{"provider":"provider","model":"model"}},"stream":[],"usage":{"inputTokens":5,"outputTokens":3,"totalTokens":8}}}"#;

        let outcome = parse(events);

        assert_eq!(outcome.events.len(), 1);
        assert_eq!(outcome.rejections.len(), 1);
        assert_eq!(outcome.rejections[0].code, REJECTION_INVALID_USAGE);
        let SessionEvent::Usage(usage) = &outcome.events[0] else {
            panic!("expected usage event");
        };
        assert_eq!(usage.seq, 2);
    }

    #[test]
    fn parses_attempt_usage_without_route() {
        let events = r#"{"type":"assistant/attempt","seq":1,"time":100,"data":{"turn":1,"step":1,"stream":[{"type":"chunk","chunk":{"type":"usage","usage":{"inputTokens":1,"outputTokens":2,"totalTokens":3}}}]}}"#;

        let outcome = parse(events);

        assert!(outcome.rejections.is_empty());
        assert_eq!(outcome.events.len(), 1);
        let SessionEvent::Usage(usage) = &outcome.events[0] else {
            panic!("expected usage event");
        };
        assert_eq!(usage.route, None);
        assert_eq!(usage.tokens.total_tokens(), 3);
    }

    #[test]
    fn parses_route_and_retry_events_in_order() {
        let events = r#"{"type":"request/context","seq":1,"time":100,"data":{"provider":"provider","model":"model"}}
{"type":"llm/retry-started","seq":2,"time":101,"data":{"turn":1,"step":1}}"#;

        let outcome = parse(events);

        assert!(outcome.rejections.is_empty());
        assert_eq!(outcome.events.len(), 2);
        assert!(matches!(outcome.events[0], SessionEvent::Route(_)));
        assert_eq!(
            outcome.events[1],
            SessionEvent::RetryStarted { turn: 1, step: 1 }
        );
    }

    #[test]
    fn ignores_unknown_event_types_and_partial_trailing_lines() {
        let events = "{\"type\":\"tool/call\",\"seq\":1,\"time\":1,\"data\":{\"arguments\":\"secret\"}}\n{\"type\":\"assistant/message\"";

        let outcome = parse_session_events(events.as_bytes());

        assert!(outcome.events.is_empty());
        assert!(outcome.rejections.is_empty());
    }

    #[test]
    fn reports_sequence_gaps() {
        let events = r#"{"type":"request/context","seq":1,"time":100,"data":{"provider":"provider","model":"model"}}
{"type":"llm/retry-started","seq":3,"time":101,"data":{"turn":1,"step":1}}"#;

        let outcome = parse(events);

        assert_eq!(outcome.rejections.len(), 1);
        assert_eq!(outcome.rejections[0].code, REJECTION_SEQUENCE_GAP);
        assert_eq!(outcome.rejections[0].seq, Some(3));
    }

    #[test]
    fn rejects_invalid_json_and_preserves_later_usage() {
        let events = "not-json\n{\"type\":\"assistant/message\",\"seq\":1,\"time\":100,\"data\":{\"turn\":1,\"step\":1,\"message\":{\"source\":{\"provider\":\"provider\",\"model\":\"model\"}},\"stream\":[],\"usage\":{\"inputTokens\":5,\"outputTokens\":3,\"totalTokens\":8}}}";

        let outcome = parse(events);

        assert_eq!(outcome.rejections.len(), 1);
        assert_eq!(outcome.rejections[0].code, REJECTION_INVALID_JSON);
        assert_eq!(outcome.events.len(), 1);
        let SessionEvent::Usage(usage) = &outcome.events[0] else {
            panic!("expected usage event");
        };
        assert_eq!(usage.tokens.total_tokens(), 8);
    }

    #[test]
    fn rejects_reasoning_tokens_above_output() {
        let events = r#"{"type":"assistant/message","seq":1,"time":100,"data":{"turn":1,"step":1,"message":{"source":{"provider":"provider","model":"model"}},"stream":[],"usage":{"inputTokens":1,"outputTokens":2,"totalTokens":3,"reasoningTokens":3}}}"#;

        let outcome = parse(events);

        assert!(outcome.events.is_empty());
        assert_eq!(outcome.rejections.len(), 1);
        assert_eq!(outcome.rejections[0].code, REJECTION_INVALID_USAGE);
    }

    #[test]
    fn rejects_total_tokens_below_output() {
        let events = r#"{"type":"assistant/message","seq":1,"time":100,"data":{"turn":1,"step":1,"message":{"source":{"provider":"provider","model":"model"}},"stream":[],"usage":{"inputTokens":1,"outputTokens":3,"totalTokens":2}}}"#;

        let outcome = parse(events);

        assert!(outcome.events.is_empty());
        assert_eq!(outcome.rejections.len(), 1);
        assert_eq!(outcome.rejections[0].code, REJECTION_INVALID_USAGE);
    }

    #[test]
    fn rejects_total_prompt_mismatch_when_both_cache_buckets_are_present() {
        let events = r#"{"type":"assistant/message","seq":1,"time":100,"data":{"turn":1,"step":1,"message":{"source":{"provider":"provider","model":"model"}},"stream":[],"usage":{"inputTokens":1,"outputTokens":1,"totalTokens":4,"cacheReadTokens":1,"cacheWriteTokens":0}}}"#;

        let outcome = parse(events);

        assert!(outcome.events.is_empty());
        assert_eq!(outcome.rejections.len(), 1);
        assert_eq!(outcome.rejections[0].code, REJECTION_INVALID_USAGE);
    }

    #[test]
    fn rejects_malformed_known_event_shapes() {
        let events =
            r#"{"type":"assistant/message","seq":1,"time":100,"data":{"turn":1,"step":1}}"#;

        let outcome = parse(events);

        assert!(outcome.events.is_empty());
        assert_eq!(outcome.rejections.len(), 1);
        assert_eq!(outcome.rejections[0].code, REJECTION_INVALID_EVENT);

        let invalid = r#"{"type":"assistant/message","seq":2,"time":100,"data":{"turn":"not-a-number","step":1,"message":{"source":{"provider":"provider","model":"model"}},"stream":[]}}"#;
        let outcome = parse(invalid);

        assert!(outcome.events.is_empty());
        assert_eq!(outcome.rejections.len(), 1);
        assert_eq!(outcome.rejections[0].code, REJECTION_INVALID_EVENT);
    }
}
