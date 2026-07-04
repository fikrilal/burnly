use chrono::Utc;
use serde_json::{Map, Value};

use crate::application::collection::{
    CollectionProjection, CollectionRequest, CollectorFailureCode,
};
use crate::application::diagnostics::{
    DiagnosticArea, DiagnosticCode, DiagnosticContext, DiagnosticEvent, DiagnosticSeverity,
    DiagnosticSummary,
};
use crate::application::ports::diagnostic_recorder::DiagnosticRecorder;
use crate::domain::source::SourceKey;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::infrastructure::collectors) struct CollectorDiagnosticCounter {
    key: &'static str,
    value: u64,
}

impl CollectorDiagnosticCounter {
    pub(in crate::infrastructure::collectors) const fn new(key: &'static str, value: u64) -> Self {
        Self { key, value }
    }
}

pub(in crate::infrastructure::collectors) fn record_collector_diagnostic(
    recorder: Option<&dyn DiagnosticRecorder>,
    request: &CollectionRequest,
    severity: DiagnosticSeverity,
    code: &str,
    summary: &str,
    failure_code: Option<CollectorFailureCode>,
    counters: &[CollectorDiagnosticCounter],
) {
    let Some(recorder) = recorder else {
        return;
    };
    let Some(event) = collector_diagnostic_event(CollectorDiagnosticEventInput {
        source: request.source(),
        projection: request.projection(),
        severity,
        code,
        summary,
        failure_code,
        counters,
        created_at_ms: Utc::now().timestamp_millis(),
    }) else {
        return;
    };
    recorder.record(event);
}

struct CollectorDiagnosticEventInput<'a> {
    source: SourceKey,
    projection: CollectionProjection,
    severity: DiagnosticSeverity,
    code: &'a str,
    summary: &'a str,
    failure_code: Option<CollectorFailureCode>,
    counters: &'a [CollectorDiagnosticCounter],
    created_at_ms: i64,
}

fn collector_diagnostic_event(input: CollectorDiagnosticEventInput<'_>) -> Option<DiagnosticEvent> {
    let context = collector_diagnostic_context(
        input.source,
        input.projection,
        input.failure_code,
        input.counters,
    )?;
    DiagnosticEvent::new(
        DiagnosticArea::Collector,
        input.severity,
        DiagnosticCode::new(input.code).ok()?,
        DiagnosticSummary::new(input.summary).ok()?,
        Some(context),
        input.created_at_ms,
    )
    .ok()
}

fn collector_diagnostic_context(
    source: SourceKey,
    projection: CollectionProjection,
    failure_code: Option<CollectorFailureCode>,
    counters: &[CollectorDiagnosticCounter],
) -> Option<DiagnosticContext> {
    let mut context = Map::new();
    context.insert(
        "source".to_owned(),
        Value::String(source.as_str().to_owned()),
    );
    context.insert(
        "projection".to_owned(),
        Value::String(projection_name(projection).to_owned()),
    );
    context.insert(
        "failureCode".to_owned(),
        failure_code.map_or(Value::Null, |code| Value::String(code.code().to_owned())),
    );
    for counter in counters {
        if !is_safe_counter_key(counter.key) {
            return None;
        }
        context.insert(counter.key.to_owned(), Value::Number(counter.value.into()));
    }

    DiagnosticContext::new(Value::Object(context).to_string()).ok()
}

fn projection_name(projection: CollectionProjection) -> &'static str {
    match projection {
        CollectionProjection::Daily => "daily",
        CollectionProjection::Session => "session",
    }
}

fn is_safe_counter_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 64
        && key
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
        && !contains_forbidden_fragment(key)
}

fn contains_forbidden_fragment(key: &str) -> bool {
    let lowercase = key.to_ascii_lowercase();
    ["path", "prompt", "response", "content", "raw", "secret"]
        .iter()
        .any(|fragment| lowercase.contains(fragment))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_bounded_collector_diagnostic_event() {
        let event = collector_diagnostic_event(CollectorDiagnosticEventInput {
            source: SourceKey::Cline,
            projection: CollectionProjection::Daily,
            severity: DiagnosticSeverity::Warning,
            code: "cline.collection_failed",
            summary: "Cline collection failed.",
            failure_code: Some(CollectorFailureCode::IncompatibleEnvelope),
            counters: &[
                CollectorDiagnosticCounter::new("sessionsFound", 2),
                CollectorDiagnosticCounter::new("recordsRejected", 1),
            ],
            created_at_ms: 100,
        })
        .expect("event");

        assert_eq!(event.area, DiagnosticArea::Collector);
        assert_eq!(event.severity, DiagnosticSeverity::Warning);
        let context = event.context.expect("context").as_str().to_owned();
        assert!(context.contains(r#""source":"cline""#));
        assert!(context.contains(r#""projection":"daily""#));
        assert!(context.contains(r#""failureCode":"collector.incompatible_envelope""#));
        assert!(context.contains(r#""sessionsFound":2"#));
        assert!(context.contains(r#""recordsRejected":1"#));
    }

    #[test]
    fn rejects_sensitive_counter_keys() {
        let event = collector_diagnostic_event(CollectorDiagnosticEventInput {
            source: SourceKey::ZCode,
            projection: CollectionProjection::Session,
            severity: DiagnosticSeverity::Warning,
            code: "zcode.collection_failed",
            summary: "ZCode collection failed.",
            failure_code: Some(CollectorFailureCode::SourceInvalidLocation),
            counters: &[CollectorDiagnosticCounter::new("databasePath", 1)],
            created_at_ms: 100,
        });

        assert!(event.is_none());
    }

    #[test]
    fn rejects_unbounded_counter_keys() {
        let event = collector_diagnostic_event(CollectorDiagnosticEventInput {
            source: SourceKey::ZCode,
            projection: CollectionProjection::Session,
            severity: DiagnosticSeverity::Warning,
            code: "zcode.collection_failed",
            summary: "ZCode collection failed.",
            failure_code: Some(CollectorFailureCode::SourceInvalidLocation),
            counters: &[CollectorDiagnosticCounter::new("rows-rejected", 1)],
            created_at_ms: 100,
        });

        assert!(event.is_none());
    }
}
