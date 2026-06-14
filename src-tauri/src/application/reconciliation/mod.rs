//! Canonical reconciliation: run lifecycle, identity, and transactional writes.
//!
//! Reconciliation is the only path that mutates imported usage facts. This phase
//! establishes the run lifecycle records that later chunks attach facts to.

mod run;

#[allow(
    unused_imports,
    reason = "reconciliation re-exports the run lifecycle contract for callers and adapters"
)]
pub(crate) use run::{
    ImportCollector, ImportOutcome, ImportRunCompletion, ImportRunId, ImportRunSpec, JobKey,
    RefreshOutcome, RefreshRunCompletion, RefreshRunId, RefreshRunSpec, RefreshTrigger, RunError,
    RunValidationError, SourceId,
};
