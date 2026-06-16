//! Session reconciliation request and summary types.
//!
//! These describe one transactional reconciliation of validated session candidates
//! into canonical facts. They carry the import identity and declared scope so the
//! store can attach provenance and so later chunks can compute absences.

#![allow(
    dead_code,
    reason = "Session reconciliation is wired into the Phase 4E refresh coordinator"
)]

use crate::application::collection::{CollectionOutcome, CollectionScope, SessionUsageCandidate};
use crate::application::reconciliation::{ImportRunId, SourceId};

/// One transactional reconciliation of session candidates for a single source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionReconciliationRequest {
    source_id: SourceId,
    import_run_id: ImportRunId,
    scope: CollectionScope,
    outcome: CollectionOutcome,
    observed_at_ms: i64,
    candidates: Vec<SessionUsageCandidate>,
}

impl SessionReconciliationRequest {
    pub(crate) fn new(
        source_id: SourceId,
        import_run_id: ImportRunId,
        scope: CollectionScope,
        outcome: CollectionOutcome,
        observed_at_ms: i64,
        candidates: Vec<SessionUsageCandidate>,
    ) -> Self {
        Self {
            source_id,
            import_run_id,
            scope,
            outcome,
            observed_at_ms,
            candidates,
        }
    }

    pub(crate) const fn source_id(&self) -> SourceId {
        self.source_id
    }

    pub(crate) const fn import_run_id(&self) -> ImportRunId {
        self.import_run_id
    }

    pub(crate) const fn scope(&self) -> &CollectionScope {
        &self.scope
    }

    pub(crate) const fn outcome(&self) -> CollectionOutcome {
        self.outcome
    }

    pub(crate) const fn observed_at_ms(&self) -> i64 {
        self.observed_at_ms
    }

    pub(crate) fn candidates(&self) -> &[SessionUsageCandidate] {
        &self.candidates
    }
}

/// Outcome of a successful session reconciliation transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionReconciliationSummary {
    upserted_sessions: u32,
    observed_source_keys: Vec<String>,
}

impl SessionReconciliationSummary {
    pub(crate) fn new(upserted_sessions: u32, observed_source_keys: Vec<String>) -> Self {
        Self {
            upserted_sessions,
            observed_source_keys,
        }
    }

    pub(crate) const fn upserted_sessions(&self) -> u32 {
        self.upserted_sessions
    }

    pub(crate) fn observed_source_keys(&self) -> &[String] {
        &self.observed_source_keys
    }
}
