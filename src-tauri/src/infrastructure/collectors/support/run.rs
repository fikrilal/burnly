use std::time::Instant;

use chrono::{DateTime, Utc};

use crate::application::collection::{
    CollectionMetadata, CollectionPeriod, CollectionProjection, CollectionRequest,
    CollectionResult, CollectorFailure, ProcessSummary,
};

use super::descriptor::{collector_key, CollectorIdentity};
use super::failure::validation_failure_as_internal;

#[derive(Debug)]
pub(in crate::infrastructure::collectors) struct LocalCollectionRun {
    started: Instant,
    started_at: DateTime<Utc>,
}

impl LocalCollectionRun {
    pub(in crate::infrastructure::collectors) fn start() -> Self {
        Self {
            started: Instant::now(),
            started_at: Utc::now(),
        }
    }

    pub(in crate::infrastructure::collectors) fn started_at(&self) -> DateTime<Utc> {
        self.started_at
    }

    pub(in crate::infrastructure::collectors) fn process_summary(&self) -> ProcessSummary {
        ProcessSummary {
            runtime_ms: self
                .started
                .elapsed()
                .as_millis()
                .try_into()
                .unwrap_or(u64::MAX),
            stdout_bytes: 0,
            stderr_bytes: 0,
            exit_code: None,
        }
    }
}

pub(in crate::infrastructure::collectors) fn collection_metadata(
    identity: CollectorIdentity,
    request: &CollectionRequest,
    started_at: DateTime<Utc>,
    finished_at: DateTime<Utc>,
) -> Result<CollectionMetadata, CollectorFailure> {
    CollectionMetadata::new(
        request.collection_id().clone(),
        collector_key(identity)?,
        identity.runtime_version.to_owned(),
        identity.source,
        request.scope().clone(),
        identity.profile_version,
        CollectionPeriod {
            started_at,
            finished_at,
        },
    )
    .map_err(|error| validation_failure_as_internal(request, error))
}

pub(in crate::infrastructure::collectors) fn empty_collection_result(
    identity: CollectorIdentity,
    request: &CollectionRequest,
    run: &LocalCollectionRun,
) -> Result<CollectionResult, CollectorFailure> {
    let metadata = collection_metadata(identity, request, run.started_at(), Utc::now())?;
    match request.projection() {
        CollectionProjection::Daily => CollectionResult::daily(
            metadata,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            run.process_summary(),
        ),
        CollectionProjection::Session => CollectionResult::session(
            metadata,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            run.process_summary(),
        ),
    }
    .map_err(|error| validation_failure_as_internal(request, error))
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::*;
    use crate::application::collection::{
        CollectionId, CollectionOutcome, CollectionScope, CollectorFailureCode,
    };
    use crate::domain::source::SourceKey;

    const IDENTITY: CollectorIdentity = CollectorIdentity {
        key: "test-source",
        display_name: "Test Source",
        runtime_version: "local",
        adapter_version: 1,
        source: SourceKey::ZCode,
        profile_version: 7,
    };

    #[test]
    fn builds_metadata_from_identity_and_request() {
        let request = daily_request();
        let started_at = Utc::now();
        let finished_at = started_at;

        let metadata =
            collection_metadata(IDENTITY, &request, started_at, finished_at).expect("metadata");

        assert_eq!(metadata.collector().as_str(), "test-source");
        assert_eq!(metadata.collector_version(), "local");
        assert_eq!(metadata.profile_version(), 7);
        assert_eq!(metadata.effective_scope(), request.scope());
    }

    #[test]
    fn invalid_identity_key_maps_to_internal_failure() {
        let request = daily_request();
        let now = Utc::now();

        let error = collection_metadata(
            CollectorIdentity {
                key: " ",
                ..IDENTITY
            },
            &request,
            now,
            now,
        )
        .expect_err("invalid key");

        assert_eq!(error.code, CollectorFailureCode::Internal);
        assert_eq!(error.source_key, None);
        assert_eq!(error.projection, None);
    }

    #[test]
    fn builds_empty_daily_and_session_results_with_local_process_summary() {
        let run = LocalCollectionRun::start();

        let daily =
            empty_collection_result(IDENTITY, &daily_request(), &run).expect("daily result");
        let session =
            empty_collection_result(IDENTITY, &session_request(), &run).expect("session result");

        assert_eq!(daily.projection(), CollectionProjection::Daily);
        assert_eq!(daily.outcome(), CollectionOutcome::Empty);
        assert_eq!(daily.process_summary().stdout_bytes, 0);
        assert_eq!(daily.process_summary().stderr_bytes, 0);
        assert_eq!(daily.process_summary().exit_code, None);
        assert_eq!(session.projection(), CollectionProjection::Session);
        assert_eq!(session.outcome(), CollectionOutcome::Empty);
    }

    fn daily_request() -> CollectionRequest {
        CollectionRequest::daily(
            CollectionId::new("collection-1").expect("collection id"),
            SourceKey::ZCode,
            CollectionScope::incremental(date(), date()).expect("scope"),
            "UTC",
            Utc::now(),
        )
        .expect("request")
    }

    fn session_request() -> CollectionRequest {
        CollectionRequest::session(
            CollectionId::new("collection-1").expect("collection id"),
            SourceKey::ZCode,
            CollectionScope::Full,
            Utc::now(),
        )
    }

    fn date() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 7, 4).expect("date")
    }
}
