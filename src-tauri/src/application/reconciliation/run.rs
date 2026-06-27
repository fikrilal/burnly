//! Refresh-run and import-run lifecycle types.
//!
//! These describe one refresh attempt and the per-source imports it performs.
//! They are the durable audit trail and provide the import identity that
//! reconciliation attaches to canonical facts. They carry only stable codes and
//! bounded, redacted summaries, never raw collector output, paths, or session
//! identifiers.

#![allow(
    dead_code,
    reason = "Run lifecycle types are persisted now and driven by the Phase 4E refresh coordinator"
)]

use thiserror::Error;

use chrono::NaiveDate;

use crate::application::collection::{CollectionProjection, CollectionScope};
use crate::domain::source::SourceKey;

/// Maximum stored length, in characters, of a redacted run error summary.
const MAX_SUMMARY_CHARS: usize = 500;

/// Identifier of a persisted `sources` row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SourceId(i64);

impl SourceId {
    pub(crate) const fn new(value: i64) -> Self {
        Self(value)
    }

    pub(crate) const fn value(self) -> i64 {
        self.0
    }
}

/// Identifier of a persisted `refresh_runs` row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RefreshRunId(i64);

impl RefreshRunId {
    pub(crate) const fn new(value: i64) -> Self {
        Self(value)
    }

    pub(crate) const fn value(self) -> i64 {
        self.0
    }
}

/// Identifier of a persisted `import_runs` row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ImportRunId(i64);

impl ImportRunId {
    pub(crate) const fn new(value: i64) -> Self {
        Self(value)
    }

    pub(crate) const fn value(self) -> i64 {
        self.0
    }
}

/// Unique per-attempt key that prevents duplicate concurrent refresh runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JobKey(String);

impl JobKey {
    pub(crate) fn new(value: impl Into<String>) -> Result<Self, RunValidationError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(RunValidationError::EmptyJobKey);
        }

        Ok(Self(value))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

/// Why a refresh run was started.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RefreshTrigger {
    Launch,
    Manual,
    Scheduled,
    FileChange,
    Resume,
    Reconcile,
}

/// Terminal result of a refresh run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RefreshOutcome {
    Succeeded,
    Partial,
    Failed,
    Cancelled,
}

/// Terminal result of a single source/projection import.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ImportOutcome {
    Succeeded,
    Partial,
    Failed,
    Cancelled,
}

/// A redacted, bounded failure description suitable for durable storage.
///
/// The code is a stable diagnostic identifier; the summary is human-readable but
/// must already be free of raw collector output, file paths, and session
/// identifiers. The summary is bounded to keep run rows small.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RunError {
    code: String,
    summary: String,
}

impl RunError {
    pub(crate) fn new(
        code: impl Into<String>,
        summary: impl Into<String>,
    ) -> Result<Self, RunValidationError> {
        let code = code.into();
        if code.trim().is_empty() {
            return Err(RunValidationError::EmptyErrorCode);
        }

        let summary: String = summary.into().chars().take(MAX_SUMMARY_CHARS).collect();

        Ok(Self { code, summary })
    }

    pub(crate) fn code(&self) -> &str {
        &self.code
    }

    pub(crate) fn summary(&self) -> &str {
        &self.summary
    }
}

/// Inputs required to begin a refresh run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RefreshRunSpec {
    job_key: JobKey,
    trigger: RefreshTrigger,
    requested_by_app_version: String,
}

impl RefreshRunSpec {
    pub(crate) fn new(
        job_key: JobKey,
        trigger: RefreshTrigger,
        requested_by_app_version: impl Into<String>,
    ) -> Result<Self, RunValidationError> {
        let requested_by_app_version = requested_by_app_version.into();
        if requested_by_app_version.trim().is_empty() {
            return Err(RunValidationError::EmptyAppVersion);
        }

        Ok(Self {
            job_key,
            trigger,
            requested_by_app_version,
        })
    }

    pub(crate) fn job_key(&self) -> &JobKey {
        &self.job_key
    }

    pub(crate) const fn trigger(&self) -> RefreshTrigger {
        self.trigger
    }

    pub(crate) fn requested_by_app_version(&self) -> &str {
        &self.requested_by_app_version
    }
}

/// Terminal completion data for a refresh run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RefreshRunCompletion {
    pub outcome: RefreshOutcome,
    pub finished_at_ms: i64,
    pub error: Option<RunError>,
}

/// Collector identity recorded for an import run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ImportCollector {
    collector_key: String,
    collector_version: String,
    profile_version: u16,
}

impl ImportCollector {
    pub(crate) fn new(
        collector_key: impl Into<String>,
        collector_version: impl Into<String>,
        profile_version: u16,
    ) -> Result<Self, RunValidationError> {
        let collector_key = collector_key.into();
        if collector_key.trim().is_empty() {
            return Err(RunValidationError::EmptyCollectorKey);
        }

        let collector_version = collector_version.into();
        if collector_version.trim().is_empty() {
            return Err(RunValidationError::EmptyCollectorVersion);
        }

        if profile_version == 0 {
            return Err(RunValidationError::InvalidProfileVersion);
        }

        Ok(Self {
            collector_key,
            collector_version,
            profile_version,
        })
    }

    pub(crate) fn collector_key(&self) -> &str {
        &self.collector_key
    }

    pub(crate) fn collector_version(&self) -> &str {
        &self.collector_version
    }

    pub(crate) const fn profile_version(&self) -> u16 {
        self.profile_version
    }
}

/// Inputs required to begin a per-source import run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ImportRunSpec {
    refresh_run_id: RefreshRunId,
    source_id: SourceId,
    collector: ImportCollector,
    projection: CollectionProjection,
    scope: CollectionScope,
    aggregation_timezone: Option<String>,
}

impl ImportRunSpec {
    pub(crate) fn new(
        refresh_run_id: RefreshRunId,
        source_id: SourceId,
        collector: ImportCollector,
        projection: CollectionProjection,
        scope: CollectionScope,
        aggregation_timezone: Option<String>,
    ) -> Result<Self, RunValidationError> {
        let aggregation_timezone = match projection {
            CollectionProjection::Daily => {
                let timezone = aggregation_timezone
                    .filter(|timezone| !timezone.trim().is_empty())
                    .ok_or(RunValidationError::MissingDailyTimezone)?;
                Some(timezone)
            }
            CollectionProjection::Session => {
                aggregation_timezone.filter(|timezone| !timezone.trim().is_empty())
            }
        };

        Ok(Self {
            refresh_run_id,
            source_id,
            collector,
            projection,
            scope,
            aggregation_timezone,
        })
    }

    pub(crate) const fn refresh_run_id(&self) -> RefreshRunId {
        self.refresh_run_id
    }

    pub(crate) const fn source_id(&self) -> SourceId {
        self.source_id
    }

    pub(crate) fn collector_key(&self) -> &str {
        self.collector.collector_key()
    }

    pub(crate) fn collector_version(&self) -> &str {
        self.collector.collector_version()
    }

    pub(crate) const fn profile_version(&self) -> u16 {
        self.collector.profile_version()
    }

    pub(crate) const fn projection(&self) -> CollectionProjection {
        self.projection
    }

    pub(crate) const fn scope(&self) -> &CollectionScope {
        &self.scope
    }

    pub(crate) fn aggregation_timezone(&self) -> Option<&str> {
        self.aggregation_timezone.as_deref()
    }
}

/// Terminal completion data for an import run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ImportRunCompletion {
    pub outcome: ImportOutcome,
    pub records_seen: u32,
    pub records_rejected: u32,
    pub finished_at_ms: i64,
    pub error: Option<RunError>,
}

/// Query identity for finding the latest successful import for a refresh target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ImportRunLookup {
    source: SourceKey,
    projection: CollectionProjection,
    aggregation_timezone: Option<String>,
}

impl ImportRunLookup {
    pub(crate) fn new(
        source: SourceKey,
        projection: CollectionProjection,
        aggregation_timezone: Option<String>,
    ) -> Result<Self, RunValidationError> {
        let aggregation_timezone = match projection {
            CollectionProjection::Daily => {
                let timezone = aggregation_timezone
                    .filter(|timezone| !timezone.trim().is_empty())
                    .ok_or(RunValidationError::MissingDailyTimezone)?;
                Some(timezone)
            }
            CollectionProjection::Session => None,
        };

        Ok(Self {
            source,
            projection,
            aggregation_timezone,
        })
    }

    pub(crate) const fn source(&self) -> SourceKey {
        self.source
    }

    pub(crate) const fn projection(&self) -> CollectionProjection {
        self.projection
    }

    pub(crate) fn aggregation_timezone(&self) -> Option<&str> {
        self.aggregation_timezone.as_deref()
    }
}

/// Successful import state used by refresh policy planning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SuccessfulImportState {
    source: SourceKey,
    projection: CollectionProjection,
    scope: CollectionScope,
    finished_at_ms: i64,
}

impl SuccessfulImportState {
    pub(crate) const fn new(
        source: SourceKey,
        projection: CollectionProjection,
        scope: CollectionScope,
        finished_at_ms: i64,
    ) -> Self {
        Self {
            source,
            projection,
            scope,
            finished_at_ms,
        }
    }

    pub(crate) const fn source(&self) -> SourceKey {
        self.source
    }

    pub(crate) const fn projection(&self) -> CollectionProjection {
        self.projection
    }

    pub(crate) const fn scope(&self) -> &CollectionScope {
        &self.scope
    }

    pub(crate) const fn finished_at_ms(&self) -> i64 {
        self.finished_at_ms
    }

    pub(crate) const fn scope_end_date(&self) -> Option<NaiveDate> {
        match &self.scope {
            CollectionScope::Full => None,
            CollectionScope::Incremental(scope) => Some(scope.end_date()),
        }
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub(crate) enum RunValidationError {
    #[error("refresh run requires a non-empty job key")]
    EmptyJobKey,
    #[error("refresh run requires a non-empty requesting app version")]
    EmptyAppVersion,
    #[error("import run requires a non-empty collector key")]
    EmptyCollectorKey,
    #[error("import run requires a non-empty collector version")]
    EmptyCollectorVersion,
    #[error("import run requires a positive profile version")]
    InvalidProfileVersion,
    #[error("daily import run requires a non-empty aggregation timezone")]
    MissingDailyTimezone,
    #[error("run error requires a non-empty stable code")]
    EmptyErrorCode,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refresh_run_id() -> RefreshRunId {
        RefreshRunId::new(1)
    }

    fn source_id() -> SourceId {
        SourceId::new(1)
    }

    #[test]
    fn job_key_rejects_blank_values() {
        assert_eq!(
            JobKey::new("   ").expect_err("blank job key"),
            RunValidationError::EmptyJobKey
        );
        assert_eq!(
            JobKey::new("refresh-1").expect("job key").as_str(),
            "refresh-1"
        );
    }

    #[test]
    fn refresh_run_spec_requires_an_app_version() {
        let job_key = JobKey::new("refresh-1").expect("job key");
        assert_eq!(
            RefreshRunSpec::new(job_key, RefreshTrigger::Manual, " ")
                .expect_err("blank app version"),
            RunValidationError::EmptyAppVersion
        );
    }

    #[test]
    fn daily_import_run_requires_a_timezone_while_session_does_not() {
        assert_eq!(
            ImportRunSpec::new(
                refresh_run_id(),
                source_id(),
                collector(),
                CollectionProjection::Daily,
                CollectionScope::Full,
                None,
            )
            .expect_err("missing daily timezone"),
            RunValidationError::MissingDailyTimezone
        );

        let session = ImportRunSpec::new(
            refresh_run_id(),
            source_id(),
            collector(),
            CollectionProjection::Session,
            CollectionScope::Full,
            None,
        )
        .expect("session spec");
        assert_eq!(session.aggregation_timezone(), None);
    }

    #[test]
    fn latest_import_lookup_matches_daily_and_session_identity_rules() {
        assert_eq!(
            ImportRunLookup::new(SourceKey::ClaudeCode, CollectionProjection::Daily, None)
                .expect_err("missing daily timezone"),
            RunValidationError::MissingDailyTimezone
        );

        let daily = ImportRunLookup::new(
            SourceKey::ClaudeCode,
            CollectionProjection::Daily,
            Some("UTC".to_owned()),
        )
        .expect("daily lookup");
        assert_eq!(daily.source(), SourceKey::ClaudeCode);
        assert_eq!(daily.projection(), CollectionProjection::Daily);
        assert_eq!(daily.aggregation_timezone(), Some("UTC"));

        let session = ImportRunLookup::new(
            SourceKey::ClaudeCode,
            CollectionProjection::Session,
            Some("UTC".to_owned()),
        )
        .expect("session lookup");
        assert_eq!(session.aggregation_timezone(), None);
    }

    #[test]
    fn import_collector_validates_identity() {
        assert_eq!(
            ImportCollector::new(" ", "20.0.11", 1).expect_err("blank collector key"),
            RunValidationError::EmptyCollectorKey
        );
        assert_eq!(
            ImportCollector::new("fixture-collector", " ", 1).expect_err("blank collector version"),
            RunValidationError::EmptyCollectorVersion
        );
        assert_eq!(
            ImportCollector::new("fixture-collector", "20.0.11", 0)
                .expect_err("invalid profile version"),
            RunValidationError::InvalidProfileVersion
        );
    }

    fn collector() -> ImportCollector {
        ImportCollector::new("fixture-collector", "20.0.11", 1).expect("import collector")
    }

    #[test]
    fn run_error_requires_a_code_and_bounds_the_summary() {
        assert_eq!(
            RunError::new(" ", "summary").expect_err("blank code"),
            RunValidationError::EmptyErrorCode
        );

        let oversized = "x".repeat(MAX_SUMMARY_CHARS + 25);
        let error = RunError::new("collector.timeout", oversized).expect("run error");
        assert_eq!(error.code(), "collector.timeout");
        assert_eq!(error.summary().chars().count(), MAX_SUMMARY_CHARS);
    }
}
