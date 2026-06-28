//! Application-owned port for persisting refresh-run and import-run lifecycle.
//!
//! The application records run lifecycle through this contract without knowing
//! the storage engine. Implementations live in infrastructure.

#![allow(
    dead_code,
    reason = "The run store contract is implemented now and called by the Phase 4E refresh coordinator"
)]

use thiserror::Error;

use crate::application::reconciliation::{
    ImportRunCompletion, ImportRunId, ImportRunLookup, ImportRunSpec, RefreshRunCompletion,
    RefreshRunId, RefreshRunSpec, SourceId, SuccessfulImportState,
};
use crate::domain::source::SourceKey;

pub(crate) trait RunStore: Send + Sync {
    /// Returns the persisted source row for `source`, creating a minimal row if
    /// none exists. Detection state is owned elsewhere and is not asserted here.
    fn resolve_source(&self, source: SourceKey, now_ms: i64) -> Result<SourceId, RunStoreError>;

    /// Inserts a running refresh run and returns its identifier.
    fn begin_refresh_run(
        &self,
        spec: RefreshRunSpec,
        now_ms: i64,
    ) -> Result<RefreshRunId, RunStoreError>;

    /// Transitions a refresh run to a terminal status.
    fn complete_refresh_run(
        &self,
        id: RefreshRunId,
        completion: RefreshRunCompletion,
    ) -> Result<(), RunStoreError>;

    /// Inserts a running import run bound to a refresh run and source.
    fn begin_import_run(
        &self,
        spec: ImportRunSpec,
        started_at_ms: i64,
    ) -> Result<ImportRunId, RunStoreError>;

    /// Transitions an import run to a terminal status with record counts.
    fn complete_import_run(
        &self,
        id: ImportRunId,
        completion: ImportRunCompletion,
    ) -> Result<(), RunStoreError>;

    /// Returns the latest successful import state for a source/projection
    /// identity, if one exists.
    fn latest_successful_import(
        &self,
        lookup: ImportRunLookup,
    ) -> Result<Option<SuccessfulImportState>, RunStoreError>;
}

/// Failure categories surfaced by the run store, independent of the engine.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RunStoreError {
    #[error("a refresh run with the same job key already exists")]
    DuplicateJobKey,
    #[error("the target run record does not exist")]
    RunNotFound,
    #[error("the run store backend failed")]
    Backend,
}
