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
    let Some(event) = collector_diagnostic_event(
        request.source(),
        request.projection(),
        severity,
        code,
        summary,
        failure_code,
        counters,
        Utc::now().timestamp_millis(),
    ) else {
        return;
    };
    recorder.record(event);
}

pub(in crate::infrastructure::collectors) fn collector_diagnostic_event(
    source: SourceKey,
    projection: CollectionProjection,
    severity: DiagnosticSeverity,
    code: &str,
    summary: &str,
    failure_code: Option<CollectorFailureCode>,
    counters: &[CollectorDiagnosticCounter],
    created_at_ms: i64,
) -> Option<DiagnosticEvent> {
    let context = collector_diagnostic_context(source, projection, failure_code, counters)?;
    DiagnosticEvent::new(
        DiagnosticArea::Collector,
        severity,
        DiagnosticCode::new(code).ok()?,
        DiagnosticSummary::new(summary).ok()?,
        Some(context),
        created_at_ms,
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
        let event = collector_diagnostic_event(
            SourceKey::Cline,
            CollectionProjection::Daily,
            DiagnosticSeverity::Warning,
            "cline.collection_failed",
            "Cline collection failed.",
            Some(CollectorFailureCode::IncompatibleEnvelope),
            &[
                CollectorDiagnosticCounter::new("sessionsFound", 2),
                CollectorDiagnosticCounter::new("recordsRejected", 1),
            ],
            100,
        )
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
        let event = collector_diagnostic_event(
            SourceKey::ZCode,
            CollectionProjection::Session,
            DiagnosticSeverity::Warning,
            "zcode.collection_failed",
            "ZCode collection failed.",
            Some(CollectorFailureCode::SourceInvalidLocation),
            &[CollectorDiagnosticCounter::new("databasePath", 1)],
            100,
        );

        assert!(event.is_none());
    }

    #[test]
    fn rejects_unbounded_counter_keys() {
        let event = collector_diagnostic_event(
            SourceKey::ZCode,
            CollectionProjection::Session,
            DiagnosticSeverity::Warning,
            "zcode.collection_failed",
            "ZCode collection failed.",
            Some(CollectorFailureCode::SourceInvalidLocation),
            &[CollectorDiagnosticCounter::new("rows-rejected", 1)],
            100,
        );

        assert!(event.is_none());
    }
}
