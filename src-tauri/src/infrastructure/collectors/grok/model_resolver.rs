use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::Deserialize;

use super::grok_home::sessions_root;
use super::session_index::GrokSessionSummary;
use super::unified_log_reader::GrokInferenceUsage;

const TURN_STARTED_EVENT: &str = "turn_started";
const UNKNOWN_MODEL: &str = "grok-unknown";

#[derive(Debug, Clone, PartialEq, Eq)]
struct TurnStartedEvent {
    session_id: String,
    model_id: String,
    observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub(crate) struct GrokModelResolver {
    turn_events: BTreeMap<String, Vec<TurnStartedEvent>>,
    summary_models: BTreeMap<String, String>,
    signals_models: BTreeMap<String, String>,
}

impl GrokModelResolver {
    pub(crate) fn from_grok_home(
        grok_home: &Path,
        summaries: &[GrokSessionSummary],
    ) -> Result<Self, ModelResolverError> {
        let mut summary_models = BTreeMap::new();
        for summary in summaries {
            if let Some(model_id) = summary
                .current_model_id
                .as_ref()
                .filter(|value| !value.trim().is_empty())
            {
                summary_models.insert(summary.session_id.clone(), model_id.clone());
            }
        }

        let sessions_root = sessions_root(grok_home);
        let mut turn_events = BTreeMap::new();
        let mut signals_models = BTreeMap::new();
        if sessions_root.is_dir() {
            let cwd_entries =
                fs::read_dir(&sessions_root).map_err(ModelResolverError::UnreadableSessionsRoot)?;
            for cwd_entry in cwd_entries {
                let cwd_entry = cwd_entry.map_err(ModelResolverError::UnreadableSessionsRoot)?;
                if !cwd_entry
                    .file_type()
                    .map_err(ModelResolverError::UnreadableSessionsRoot)?
                    .is_dir()
                {
                    continue;
                }

                let session_entries = fs::read_dir(cwd_entry.path())
                    .map_err(ModelResolverError::UnreadableSessionsRoot)?;
                for session_entry in session_entries {
                    let session_entry =
                        session_entry.map_err(ModelResolverError::UnreadableSessionsRoot)?;
                    if !session_entry
                        .file_type()
                        .map_err(ModelResolverError::UnreadableSessionsRoot)?
                        .is_dir()
                    {
                        continue;
                    }

                    let session_dir = session_entry.path();
                    load_turn_events(&session_dir, &mut turn_events)?;
                    load_signals_model(&session_dir, &mut signals_models)?;
                }
            }
        }

        for events in turn_events.values_mut() {
            events.sort_by_key(|event| event.observed_at);
        }

        Ok(Self {
            turn_events,
            summary_models,
            signals_models,
        })
    }

    pub(crate) fn resolve(&self, inference: &GrokInferenceUsage) -> String {
        if let Some(model_id) = self.resolve_from_turn_events(inference) {
            return model_id;
        }
        if let Some(model_id) = self.summary_models.get(&inference.session_id) {
            return model_id.clone();
        }
        if let Some(model_id) = self.signals_models.get(&inference.session_id) {
            return model_id.clone();
        }
        UNKNOWN_MODEL.to_owned()
    }
}

impl GrokModelResolver {
    fn resolve_from_turn_events(&self, inference: &GrokInferenceUsage) -> Option<String> {
        let events = self.turn_events.get(&inference.session_id)?;
        events
            .iter()
            .filter(|event| event.observed_at <= inference.observed_at)
            .map(|event| event.model_id.clone())
            .next_back()
    }
}

fn load_turn_events(
    session_dir: &Path,
    turn_events: &mut BTreeMap<String, Vec<TurnStartedEvent>>,
) -> Result<(), ModelResolverError> {
    let events_path = session_dir.join("events.jsonl");
    if !events_path.is_file() {
        return Ok(());
    }

    let contents =
        fs::read_to_string(&events_path).map_err(ModelResolverError::UnreadableEventLog)?;
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(event) = parse_turn_started_line(trimmed) {
            turn_events
                .entry(event.session_id.clone())
                .or_default()
                .push(event);
        }
    }
    Ok(())
}

fn load_signals_model(
    session_dir: &Path,
    signals_models: &mut BTreeMap<String, String>,
) -> Result<(), ModelResolverError> {
    let signals_path = session_dir.join("signals.json");
    if !signals_path.is_file() {
        return Ok(());
    }

    let contents =
        fs::read_to_string(&signals_path).map_err(ModelResolverError::UnreadableSignalsFile)?;
    let raw: SignalsFile =
        serde_json::from_str(&contents).map_err(|_| ModelResolverError::IncompatibleSignalsFile)?;
    let Some(session_id) = session_dir
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
    else {
        return Ok(());
    };
    let Some(model_id) = raw
        .primary_model_id
        .filter(|value| !value.trim().is_empty())
    else {
        return Ok(());
    };
    signals_models.insert(session_id.to_owned(), model_id);
    Ok(())
}

fn parse_turn_started_line(line: &str) -> Option<TurnStartedEvent> {
    let envelope = serde_json::from_str::<TurnStartedEnvelope>(line).ok()?;
    if envelope.event_type != TURN_STARTED_EVENT {
        return None;
    }
    let session_id = envelope.session_id?;
    let model_id = envelope.model_id?;
    if session_id.trim().is_empty() || model_id.trim().is_empty() {
        return None;
    }
    let observed_at = DateTime::parse_from_rfc3339(&envelope.ts)
        .ok()?
        .with_timezone(&Utc);
    Some(TurnStartedEvent {
        session_id,
        model_id,
        observed_at,
    })
}

#[derive(Debug, Deserialize)]
struct TurnStartedEnvelope {
    ts: String,
    #[serde(rename = "type")]
    event_type: String,
    session_id: Option<String>,
    model_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SignalsFile {
    #[serde(rename = "primaryModelId")]
    primary_model_id: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum ModelResolverError {
    #[error("grok sessions root could not be read")]
    UnreadableSessionsRoot(#[source] std::io::Error),
    #[error("grok events log could not be read")]
    UnreadableEventLog(#[source] std::io::Error),
    #[error("grok signals file could not be read")]
    UnreadableSignalsFile(#[source] std::io::Error),
    #[error("grok signals file is incompatible")]
    IncompatibleSignalsFile,
}

#[cfg(test)]
mod tests {
    use std::fs;

    use chrono::TimeZone;
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn resolves_model_from_latest_turn_started_before_inference() {
        let temp = TempDir::new().expect("temp dir");
        let grok_home = temp.path();
        seed_session(
            grok_home,
            "019f0000-0000-7000-8000-000000000001",
            Some("grok-composer-2.5-fast"),
            Some("grok-composer-2.5-fast"),
            "events/turn-started.jsonl",
        );

        let summaries = vec![GrokSessionSummary {
            session_id: "019f0000-0000-7000-8000-000000000001".to_owned(),
            cwd: "/tmp/grok-fixture-project".to_owned(),
            current_model_id: Some("summary-model".to_owned()),
            agent_name: None,
            git_root_dir: None,
            head_branch: None,
            created_at: None,
            updated_at: None,
        }];
        let resolver = GrokModelResolver::from_grok_home(grok_home, &summaries).expect("resolver");

        let inference = GrokInferenceUsage {
            session_id: "019f0000-0000-7000-8000-000000000001".to_owned(),
            observed_at: Utc
                .with_ymd_and_hms(2026, 7, 6, 10, 0, 15)
                .single()
                .expect("timestamp"),
            pid: 1001,
            loop_index: 1,
            prompt_tokens: 10,
            cached_prompt_tokens: 0,
            completion_tokens: 1,
            reasoning_tokens: 0,
        };

        assert_eq!(resolver.resolve(&inference), "grok-composer-2.5-fast");
    }

    #[test]
    fn falls_back_to_summary_model_when_turn_events_are_absent() {
        let temp = TempDir::new().expect("temp dir");
        let grok_home = temp.path();
        seed_session(grok_home, "session-1", Some("summary-model"), None, "");

        let summaries = vec![GrokSessionSummary {
            session_id: "session-1".to_owned(),
            cwd: "/tmp/project".to_owned(),
            current_model_id: Some("summary-model".to_owned()),
            agent_name: None,
            git_root_dir: None,
            head_branch: None,
            created_at: None,
            updated_at: None,
        }];
        let resolver = GrokModelResolver::from_grok_home(grok_home, &summaries).expect("resolver");
        let inference = inference_at("session-1", 2026, 7, 6, 10, 0, 0);

        assert_eq!(resolver.resolve(&inference), "summary-model");
    }

    #[test]
    fn falls_back_to_signals_model_when_summary_model_is_absent() {
        let temp = TempDir::new().expect("temp dir");
        let grok_home = temp.path();
        seed_session(grok_home, "session-1", None, Some("signals-model"), "");

        let summaries = vec![GrokSessionSummary {
            session_id: "session-1".to_owned(),
            cwd: "/tmp/project".to_owned(),
            current_model_id: None,
            agent_name: None,
            git_root_dir: None,
            head_branch: None,
            created_at: None,
            updated_at: None,
        }];
        let resolver = GrokModelResolver::from_grok_home(grok_home, &summaries).expect("resolver");
        let inference = inference_at("session-1", 2026, 7, 6, 10, 0, 0);

        assert_eq!(resolver.resolve(&inference), "signals-model");
    }

    #[test]
    fn uses_unknown_model_when_no_attribution_sources_exist() {
        let temp = TempDir::new().expect("temp dir");
        let grok_home = temp.path();
        seed_session(grok_home, "session-1", None, None, "");

        let resolver = GrokModelResolver::from_grok_home(grok_home, &[]).expect("resolver");
        let inference = inference_at("session-1", 2026, 7, 6, 10, 0, 0);

        assert_eq!(resolver.resolve(&inference), UNKNOWN_MODEL);
    }

    fn seed_session(
        grok_home: &Path,
        session_id: &str,
        summary_model: Option<&str>,
        signals_model: Option<&str>,
        events_fixture: &str,
    ) {
        let session_dir = grok_home
            .join("sessions")
            .join("encoded-cwd")
            .join(session_id);
        fs::create_dir_all(&session_dir).expect("session dir");

        if let Some(model_id) = summary_model {
            let summary = format!(
                r#"{{
                  "info": {{
                    "id": "{session_id}",
                    "cwd": "/tmp/project"
                  }},
                  "current_model_id": "{model_id}"
                }}"#
            );
            fs::write(session_dir.join("summary.json"), summary).expect("summary");
        }

        if let Some(model_id) = signals_model {
            let signals = format!(r#"{{"primaryModelId":"{model_id}"}}"#);
            fs::write(session_dir.join("signals.json"), signals).expect("signals");
        }

        if !events_fixture.is_empty() {
            fs::copy(
                fixture_path(events_fixture),
                session_dir.join("events.jsonl"),
            )
            .expect("events");
        }
    }

    fn fixture_path(relative: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/collectors/grok")
            .join(relative)
    }

    fn inference_at(
        session_id: &str,
        year: i32,
        month: u32,
        day: u32,
        hour: u32,
        minute: u32,
        second: u32,
    ) -> GrokInferenceUsage {
        GrokInferenceUsage {
            session_id: session_id.to_owned(),
            observed_at: Utc
                .with_ymd_and_hms(year, month, day, hour, minute, second)
                .single()
                .expect("timestamp"),
            pid: 1,
            loop_index: 1,
            prompt_tokens: 1,
            cached_prompt_tokens: 0,
            completion_tokens: 1,
            reasoning_tokens: 0,
        }
    }
}
