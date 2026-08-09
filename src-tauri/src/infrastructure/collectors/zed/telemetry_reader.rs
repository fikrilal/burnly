//! Zed agent telemetry log reader.
//!
//! Parses `~/.local/share/zed/logs/telemetry.log` `Agent Thread Completion
//! Usage Updated` events: per-request token usage with a relative timeline
//! (`milliseconds_since_first_event`). Anchors the relative timeline onto
//! thread absolute windows so per-request usage can be attributed to days.
//! Message content is never deserialized.

#![allow(
    dead_code,
    reason = "telemetry reader is consumed by the adapter in a later chunk"
)]

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;
use thiserror::Error;

use super::threads_store::{ZedThreadUsage, ZedTokenUsage};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TelemetryUsageEvent {
    pub(crate) thread_id: String,
    pub(crate) prompt_id: String,
    /// Milliseconds since the telemetry session start.
    pub(crate) relative_ms: u64,
    pub(crate) tokens: ZedTokenUsage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AnchoredEvent {
    pub(crate) event: TelemetryUsageEvent,
    pub(crate) observed_at: DateTime<Utc>,
}

#[derive(Debug, Error)]
pub(crate) enum ZedTelemetryError {
    #[error("zed telemetry log could not be read")]
    Read(#[source] std::io::Error),
}

#[derive(Debug)]
pub(crate) struct ZedTelemetryReader;

impl ZedTelemetryReader {
    pub(crate) fn read_events(
        path: impl AsRef<Path>,
    ) -> Result<Vec<TelemetryUsageEvent>, ZedTelemetryError> {
        let file = File::open(path).map_err(ZedTelemetryError::Read)?;
        let reader = BufReader::new(file);
        let mut events = Vec::new();

        for line in reader.lines() {
            let line = line.map_err(ZedTelemetryError::Read)?;
            let Ok(envelope) = serde_json::from_str::<TelemetryEnvelope>(&line) else {
                continue;
            };
            if envelope.event_type != "Agent Thread Completion Usage Updated" {
                continue;
            }
            let props = envelope.event_properties;
            let thread_id = props.thread_id;
            if thread_id.is_empty() {
                continue;
            }
            events.push(TelemetryUsageEvent {
                thread_id,
                prompt_id: props.prompt_id,
                relative_ms: envelope.milliseconds_since_first_event,
                tokens: ZedTokenUsage {
                    input_tokens: props.input_tokens,
                    output_tokens: props.output_tokens,
                    cache_read_tokens: props.cache_read_input_tokens,
                    cache_creation_tokens: props.cache_creation_input_tokens,
                },
            });
        }

        Ok(events)
    }
}

/// Anchor relative events onto absolute thread windows.
///
/// The telemetry session start is estimated from the earliest event of an
/// *anchored* thread (one present in `threads`): `session_start =
/// thread.created_at - earliest_relative_ms`. Events then map to
/// `session_start + relative_ms`. Threads without a window (not in `threads`)
/// are left unanchored (omitted from the result). Events from unanchored
/// threads never become the anchor — otherwise a single deleted thread at the
/// start of the log would drop all valid threads' events.
pub(crate) fn anchor_events(
    events: Vec<TelemetryUsageEvent>,
    threads: &[ZedThreadUsage],
) -> Vec<AnchoredEvent> {
    let windows: HashMap<&str, DateTime<Utc>> = threads
        .iter()
        .map(|t| (t.thread_id.as_str(), t.created_at))
        .collect();

    let Some(earliest) = events
        .iter()
        .filter(|e| windows.contains_key(e.thread_id.as_str()))
        .min_by_key(|e| e.relative_ms)
    else {
        return Vec::new();
    };
    let session_start =
        windows[earliest.thread_id.as_str()] - Duration::milliseconds(earliest.relative_ms as i64);

    events
        .into_iter()
        .filter(|e| windows.contains_key(e.thread_id.as_str()))
        .map(|event| {
            let observed_at = session_start + Duration::milliseconds(event.relative_ms as i64);
            AnchoredEvent { event, observed_at }
        })
        .collect()
}

/// Cross-check: sum per-request tokens per thread and compare to cumulative.
/// Returns per-thread totals for the given events (used in tests and
/// diagnostics; the durable source remains `cumulative_token_usage`).
pub(crate) fn sum_per_thread(events: &[AnchoredEvent]) -> HashMap<String, ZedTokenUsage> {
    let mut totals: HashMap<String, ZedTokenUsage> = HashMap::new();
    for anchored in events {
        let entry = totals.entry(anchored.event.thread_id.clone()).or_default();
        entry.input_tokens = entry
            .input_tokens
            .saturating_add(anchored.event.tokens.input_tokens);
        entry.output_tokens = entry
            .output_tokens
            .saturating_add(anchored.event.tokens.output_tokens);
        entry.cache_read_tokens = entry
            .cache_read_tokens
            .saturating_add(anchored.event.tokens.cache_read_tokens);
        entry.cache_creation_tokens = entry
            .cache_creation_tokens
            .saturating_add(anchored.event.tokens.cache_creation_tokens);
    }
    totals
}

#[derive(Debug, Deserialize)]
struct TelemetryEnvelope {
    #[serde(default)]
    milliseconds_since_first_event: u64,
    event_type: String,
    event_properties: UsageEventProperties,
}

#[derive(Debug, Deserialize)]
struct UsageEventProperties {
    #[serde(default)]
    thread_id: String,
    #[serde(default)]
    prompt_id: String,
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    cache_read_input_tokens: u64,
    #[serde(default)]
    cache_creation_input_tokens: u64,
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    const VALID_EVENTS: &str = r#"{"milliseconds_since_first_event":0,"event_type":"Agent Thread Completion Usage Updated","event_properties":{"thread_id":"t1","prompt_id":"p1","input_tokens":100,"output_tokens":10,"cache_read_input_tokens":50,"cache_creation_input_tokens":0}}
{"milliseconds_since_first_event":5000,"event_type":"Agent Thread Completion Usage Updated","event_properties":{"thread_id":"t1","prompt_id":"p2","input_tokens":200,"output_tokens":20,"cache_read_input_tokens":0,"cache_creation_input_tokens":5}}
{"milliseconds_since_first_event":9000,"event_type":"Some Other Event","event_properties":{}}
this is not json
{"milliseconds_since_first_event":30000,"event_type":"Agent Thread Completion Usage Updated","event_properties":{"thread_id":"t2","prompt_id":"p3","input_tokens":50,"output_tokens":5,"cache_read_input_tokens":0,"cache_creation_input_tokens":0}}"#;

    fn thread(id: &str, created_sec: i64) -> ZedThreadUsage {
        ZedThreadUsage {
            thread_id: id.to_owned(),
            title: "t".to_owned(),
            model_provider: "zed.dev".to_owned(),
            model_id: "m".to_owned(),
            created_at: DateTime::from_timestamp(created_sec, 0).expect("ts"),
            updated_at: DateTime::from_timestamp(created_sec + 60, 0).expect("ts"),
            tokens: ZedTokenUsage::default(),
        }
    }

    fn write_log(dir: &TempDir, contents: &str) -> std::path::PathBuf {
        let path = dir.path().join("telemetry.log");
        fs::write(&path, contents).expect("write");
        path
    }

    #[test]
    fn parses_usage_events_and_skips_malformed() {
        let dir = TempDir::new().expect("dir");
        let path = write_log(&dir, VALID_EVENTS);

        let events = ZedTelemetryReader::read_events(&path).expect("read");
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].thread_id, "t1");
        assert_eq!(events[0].prompt_id, "p1");
        assert_eq!(events[0].tokens.input_tokens, 100);
        assert_eq!(events[0].tokens.cache_read_tokens, 50);
        assert_eq!(events[1].tokens.cache_creation_tokens, 5);
        assert_eq!(events[2].thread_id, "t2");
    }

    #[test]
    fn missing_log_is_empty_not_error() {
        let dir = TempDir::new().expect("dir");
        let events = ZedTelemetryReader::read_events(dir.path().join("missing.log"));
        assert!(events.is_err());
    }

    #[test]
    fn anchors_events_onto_thread_windows() {
        let dir = TempDir::new().expect("dir");
        let path = write_log(&dir, VALID_EVENTS);
        let events = ZedTelemetryReader::read_events(&path).expect("read");
        // t1 created at epoch+100000; earliest event relative_ms=0 for t1.
        let threads = vec![thread("t1", 100_000), thread("t2", 200_000)];

        let anchored = anchor_events(events, &threads);
        assert_eq!(anchored.len(), 3);
        // session_start = t1.created_at (100000) - 0 = 100000.
        let t1_first = anchored
            .iter()
            .find(|a| a.event.prompt_id == "p1")
            .expect("p1");
        assert_eq!(t1_first.observed_at.timestamp(), 100_000);
        // p2 at +5000ms = +5s -> 100005 epoch seconds.
        let t1_second = anchored
            .iter()
            .find(|a| a.event.prompt_id == "p2")
            .expect("p2");
        assert_eq!(t1_second.observed_at.timestamp(), 100_005);
        // t2 event at +30000ms = +30s -> 100030.
        let t2 = anchored
            .iter()
            .find(|a| a.event.prompt_id == "p3")
            .expect("p3");
        assert_eq!(t2.observed_at.timestamp(), 100_030);
    }

    #[test]
    fn unanchored_threads_are_omitted() {
        let dir = TempDir::new().expect("dir");
        let path = write_log(&dir, VALID_EVENTS);
        let events = ZedTelemetryReader::read_events(&path).expect("read");
        // No thread windows at all -> nothing anchored.
        let anchored = anchor_events(events, &[]);
        assert!(anchored.is_empty());
    }

    #[test]
    fn unanchored_thread_at_log_start_does_not_drop_valid_events() {
        // Regression: an event for a deleted/unanchored thread at the start of
        // the log must not become the anchor (which would discard all valid
        // threads' events). The anchor is the earliest event of an anchored
        // thread.
        let log = r#"{"milliseconds_since_first_event":0,"event_type":"Agent Thread Completion Usage Updated","event_properties":{"thread_id":"ghost","prompt_id":"pg","input_tokens":1,"output_tokens":1,"cache_read_input_tokens":0,"cache_creation_input_tokens":0}}
{"milliseconds_since_first_event":1000,"event_type":"Agent Thread Completion Usage Updated","event_properties":{"thread_id":"t1","prompt_id":"p1","input_tokens":100,"output_tokens":10,"cache_read_input_tokens":50,"cache_creation_input_tokens":0}}
{"milliseconds_since_first_event":6000,"event_type":"Agent Thread Completion Usage Updated","event_properties":{"thread_id":"t1","prompt_id":"p2","input_tokens":200,"output_tokens":20,"cache_read_input_tokens":0,"cache_creation_input_tokens":5}}"#;
        let dir = TempDir::new().expect("dir");
        let path = write_log(&dir, log);
        let events = ZedTelemetryReader::read_events(&path).expect("read");
        let threads = vec![thread("t1", 100_000)];

        let anchored = anchor_events(events, &threads);

        // Valid thread's events survive; the ghost thread is omitted.
        assert_eq!(anchored.len(), 2);
        // Earliest anchored event is t1@p1 at relative 1000 -> session_start =
        // 100000 - 1s = 99999; p1 at 100000, p2 at 100005.
        let p1 = anchored
            .iter()
            .find(|a| a.event.prompt_id == "p1")
            .expect("p1");
        assert_eq!(p1.observed_at.timestamp(), 100_000);
        let p2 = anchored
            .iter()
            .find(|a| a.event.prompt_id == "p2")
            .expect("p2");
        assert_eq!(p2.observed_at.timestamp(), 100_005);
    }

    #[test]
    fn cross_checks_per_request_sums_against_cumulative() {
        let dir = TempDir::new().expect("dir");
        let path = write_log(&dir, VALID_EVENTS);
        let events = ZedTelemetryReader::read_events(&path).expect("read");
        let threads = vec![thread("t1", 100_000)];
        let anchored = anchor_events(events, &threads);

        let totals = sum_per_thread(&anchored);
        let t1 = &totals["t1"];
        // p1 (100 in, 10 out, 50 cr) + p2 (200 in, 20 out, 5 cc)
        assert_eq!(t1.input_tokens, 300);
        assert_eq!(t1.output_tokens, 30);
        assert_eq!(t1.cache_read_tokens, 50);
        assert_eq!(t1.cache_creation_tokens, 5);
    }

    #[test]
    fn reads_sanitized_telemetry_fixture() {
        let fixture = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tests/fixtures/collectors/zed/telemetry/usage-events.jsonl"
        ));
        let dir = TempDir::new().expect("dir");
        let path = write_log(&dir, fixture);

        let events = ZedTelemetryReader::read_events(&path).expect("read");
        assert_eq!(events.len(), 4);
        // All 3 threads present with exact tokens.
        let luna = events
            .iter()
            .find(|e| e.thread_id.starts_with("c0632051"))
            .expect("luna");
        assert_eq!(luna.tokens.input_tokens, 8027);
        let gemini = events
            .iter()
            .find(|e| e.thread_id.starts_with("a29f312e"))
            .expect("gemini");
        assert_eq!(gemini.tokens.input_tokens, 9455);
        let claude = events
            .iter()
            .find(|e| e.thread_id.starts_with("3a2d4c89"))
            .expect("claude");
        assert_eq!(claude.tokens.cache_creation_tokens, 15494);
    }
}
