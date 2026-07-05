use std::sync::Mutex;

use chrono::{DateTime, NaiveDate, TimeZone, Utc};

use crate::application::collection::{
    CollectionId, CollectionRequest, CollectionScope, DetectionReason, DetectionRequest,
};
use crate::application::diagnostics::DiagnosticEvent;
use crate::application::ports::collector::CancellationSignal;
use crate::application::ports::diagnostic_recorder::DiagnosticRecorder;
use crate::domain::source::SourceKey;

pub(in crate::infrastructure::collectors) struct NeverCancelled;

impl CancellationSignal for NeverCancelled {
    fn is_cancelled(&self) -> bool {
        false
    }
}

#[derive(Default)]
pub(in crate::infrastructure::collectors) struct RecordingDiagnostics {
    events: Mutex<Vec<DiagnosticEvent>>,
}

impl RecordingDiagnostics {
    pub(in crate::infrastructure::collectors) fn events(&self) -> Vec<DiagnosticEvent> {
        self.events.lock().expect("diagnostics").clone()
    }
}

impl DiagnosticRecorder for RecordingDiagnostics {
    fn record(&self, event: DiagnosticEvent) {
        self.events.lock().expect("diagnostics").push(event);
    }
}

pub(in crate::infrastructure::collectors) fn detection_request(
    source: SourceKey,
    requested_at: DateTime<Utc>,
) -> DetectionRequest {
    DetectionRequest {
        source,
        reason: DetectionReason::Startup,
        requested_at,
    }
}

pub(in crate::infrastructure::collectors) fn daily_request(
    collection_id: &str,
    source: SourceKey,
    scope: CollectionScope,
    aggregation_timezone: &str,
    requested_at: DateTime<Utc>,
) -> CollectionRequest {
    CollectionRequest::daily(
        CollectionId::new(collection_id).expect("collection id"),
        source,
        scope,
        aggregation_timezone,
        requested_at,
    )
    .expect("daily request")
}

pub(in crate::infrastructure::collectors) fn session_request(
    collection_id: &str,
    source: SourceKey,
    requested_at: DateTime<Utc>,
) -> CollectionRequest {
    CollectionRequest::session(
        CollectionId::new(collection_id).expect("collection id"),
        source,
        CollectionScope::Full,
        requested_at,
    )
}

pub(in crate::infrastructure::collectors) fn fixed_timestamp(
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(year, month, day, hour, minute, second)
        .single()
        .expect("timestamp")
}

pub(in crate::infrastructure::collectors) fn utc_millis(value: i64) -> DateTime<Utc> {
    Utc.timestamp_millis_opt(value).single().expect("timestamp")
}

pub(in crate::infrastructure::collectors) fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).expect("date")
}
