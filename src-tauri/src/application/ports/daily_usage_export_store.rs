//! Read port for scoped daily usage export (collect sync).

#![allow(
    dead_code,
    reason = "Port surface is for collect-sync composition across phase chunks"
)]

use chrono::NaiveDate;
use thiserror::Error;

use crate::application::collect_sync::{ExportedDailyFact, UploadScope};

/// Query parameters for exporting allowlisted daily facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DailyUsageExportQuery {
    pub scope: UploadScope,
    pub reporting_timezone: String,
}

impl DailyUsageExportQuery {
    pub(crate) fn full(reporting_timezone: impl Into<String>) -> Self {
        Self {
            scope: UploadScope::Full,
            reporting_timezone: reporting_timezone.into(),
        }
    }

    pub(crate) fn incremental(
        reporting_timezone: impl Into<String>,
        source_keys: impl IntoIterator<Item = String>,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<Self, DailyUsageExportStoreError> {
        let scope = UploadScope::incremental(source_keys, start_date, end_date)
            .map_err(|_| DailyUsageExportStoreError::InvalidScope)?;
        Ok(Self {
            scope,
            reporting_timezone: reporting_timezone.into(),
        })
    }
}

pub(crate) trait DailyUsageExportStore: Send + Sync {
    /// Loads allowlisted daily facts for the given scope.
    ///
    /// Must not read projects, sessions, diagnostics, credentials, or raw
    /// collector payload tables into the result.
    fn export_daily_facts(
        &self,
        query: &DailyUsageExportQuery,
    ) -> Result<Vec<ExportedDailyFact>, DailyUsageExportStoreError>;
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DailyUsageExportStoreError {
    #[error("the export query scope is invalid")]
    InvalidScope,
    #[allow(dead_code)] // reserved for integer conversion failures
    #[error("a usage value exceeded the supported integer range")]
    ValueOutOfRange,
    #[error("the export store backend failed")]
    Backend,
}
