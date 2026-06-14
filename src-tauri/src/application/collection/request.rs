use chrono::{DateTime, NaiveDate, Utc};
use thiserror::Error;

use crate::domain::source::SourceKey;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct CollectionId(String);

impl CollectionId {
    pub(crate) fn new(value: impl Into<String>) -> Result<Self, RequestValidationError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(RequestValidationError::EmptyCollectionId);
        }

        Ok(Self(value))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CollectionProjection {
    Daily,
    Session,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CollectionScope {
    Full,
    Incremental(IncrementalScope),
}

impl CollectionScope {
    pub(crate) fn incremental(
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<Self, RequestValidationError> {
        if start_date > end_date {
            return Err(RequestValidationError::InvalidDateRange);
        }

        Ok(Self::Incremental(IncrementalScope {
            start_date,
            end_date,
        }))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IncrementalScope {
    start_date: NaiveDate,
    end_date: NaiveDate,
}

impl IncrementalScope {
    pub(crate) const fn start_date(&self) -> NaiveDate {
        self.start_date
    }

    pub(crate) const fn end_date(&self) -> NaiveDate {
        self.end_date
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CollectionRequest {
    collection_id: CollectionId,
    source: SourceKey,
    projection: CollectionProjection,
    scope: CollectionScope,
    aggregation_timezone: Option<String>,
    settings: CollectionSettings,
    requested_at: DateTime<Utc>,
}

impl CollectionRequest {
    pub(crate) fn daily(
        collection_id: CollectionId,
        source: SourceKey,
        scope: CollectionScope,
        aggregation_timezone: impl Into<String>,
        requested_at: DateTime<Utc>,
    ) -> Result<Self, RequestValidationError> {
        let aggregation_timezone = aggregation_timezone.into();
        if aggregation_timezone.trim().is_empty() {
            return Err(RequestValidationError::MissingAggregationTimezone);
        }

        Ok(Self {
            collection_id,
            source,
            projection: CollectionProjection::Daily,
            scope,
            aggregation_timezone: Some(aggregation_timezone),
            settings: CollectionSettings::ClaudeCode,
            requested_at,
        })
    }

    pub(crate) fn session(
        collection_id: CollectionId,
        source: SourceKey,
        scope: CollectionScope,
        requested_at: DateTime<Utc>,
    ) -> Self {
        Self {
            collection_id,
            source,
            projection: CollectionProjection::Session,
            scope,
            aggregation_timezone: None,
            settings: CollectionSettings::ClaudeCode,
            requested_at,
        }
    }

    pub(crate) fn collection_id(&self) -> &CollectionId {
        &self.collection_id
    }

    pub(crate) const fn source(&self) -> SourceKey {
        self.source
    }

    pub(crate) const fn projection(&self) -> CollectionProjection {
        self.projection
    }

    pub(crate) fn scope(&self) -> &CollectionScope {
        &self.scope
    }

    pub(crate) fn aggregation_timezone(&self) -> Option<&str> {
        self.aggregation_timezone.as_deref()
    }

    pub(crate) const fn settings(&self) -> CollectionSettings {
        self.settings
    }

    pub(crate) fn requested_at(&self) -> &DateTime<Utc> {
        &self.requested_at
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CollectionSettings {
    ClaudeCode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DetectionReason {
    Startup,
    UserRequested,
    SettingsChanged,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DetectionRequest {
    pub source: SourceKey,
    pub reason: DetectionReason,
    pub requested_at: DateTime<Utc>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub(crate) enum RequestValidationError {
    #[error("collection id must not be empty")]
    EmptyCollectionId,

    #[error("incremental collection start date must not be after end date")]
    InvalidDateRange,

    #[error("daily collection requires an aggregation timezone")]
    MissingAggregationTimezone,
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    #[test]
    fn daily_request_requires_timezone_without_affecting_session_shape() {
        let collection_id = CollectionId::new("collection-1").expect("collection id");
        let requested_at = Utc
            .with_ymd_and_hms(2026, 6, 14, 7, 30, 0)
            .single()
            .expect("timestamp");

        assert_eq!(
            CollectionRequest::daily(
                collection_id.clone(),
                SourceKey::ClaudeCode,
                CollectionScope::Full,
                " ",
                requested_at,
            )
            .expect_err("missing timezone"),
            RequestValidationError::MissingAggregationTimezone
        );

        let session = CollectionRequest::session(
            collection_id,
            SourceKey::ClaudeCode,
            CollectionScope::Full,
            requested_at,
        );
        assert_eq!(session.aggregation_timezone(), None);
    }

    #[test]
    fn incremental_scope_rejects_reversed_dates() {
        let start = NaiveDate::from_ymd_opt(2026, 6, 15).expect("start date");
        let end = NaiveDate::from_ymd_opt(2026, 6, 14).expect("end date");

        assert_eq!(
            CollectionScope::incremental(start, end).expect_err("invalid range"),
            RequestValidationError::InvalidDateRange
        );
    }
}
