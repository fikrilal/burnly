use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use chrono_tz::Tz;

use crate::application::collection::{
    CandidateProvenance, CollectionId, CollectionScope, CollectorKey,
};
use crate::domain::source::SourceKey;
use crate::domain::usage::DataQuality;

#[derive(Debug, Clone)]
pub(in crate::infrastructure::collectors) struct MappingIdentity {
    pub source: SourceKey,
    pub collector: CollectorKey,
    pub collector_version: String,
    pub profile_version: u16,
    pub collection_id: CollectionId,
    pub observed_at: DateTime<Utc>,
}

pub(in crate::infrastructure::collectors) fn provenance(
    identity: &MappingIdentity,
) -> CandidateProvenance {
    CandidateProvenance {
        source: identity.source,
        collector: identity.collector.clone(),
        collector_version: identity.collector_version.clone(),
        profile_version: identity.profile_version,
        collection_id: identity.collection_id.clone(),
        observed_at: identity.observed_at,
        data_quality: DataQuality::Complete,
        warnings: Vec::new(),
    }
}

pub(in crate::infrastructure::collectors) fn date_in_scope(
    usage_date: NaiveDate,
    scope: &CollectionScope,
) -> bool {
    match scope {
        CollectionScope::Full => true,
        CollectionScope::Incremental(scope) => {
            scope.start_date() <= usage_date && usage_date <= scope.end_date()
        }
    }
}

pub(in crate::infrastructure::collectors) fn utc_from_millis<E>(
    timestamp_ms: i64,
    error: E,
) -> Result<DateTime<Utc>, E> {
    Utc.timestamp_millis_opt(timestamp_ms).single().ok_or(error)
}

pub(in crate::infrastructure::collectors) fn local_date_from_millis<E>(
    timestamp_ms: i64,
    timezone: Tz,
    error: E,
) -> Result<NaiveDate, E> {
    Ok(utc_from_millis(timestamp_ms, error)?
        .with_timezone(&timezone)
        .date_naive())
}

pub(in crate::infrastructure::collectors) fn checked_add_u64<E>(
    left: u64,
    right: u64,
    error: E,
) -> Result<u64, E> {
    left.checked_add(right).ok_or(error)
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::*;
    use crate::application::collection::CollectionId;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Error {
        InvalidTimestamp,
        Overflow,
    }

    #[test]
    fn builds_complete_candidate_provenance() {
        let collector = CollectorKey::new("test-collector").expect("collector");
        let collection_id = CollectionId::new("collection-1").expect("collection");
        let observed_at = Utc::now();

        let result = provenance(&MappingIdentity {
            source: SourceKey::Cline,
            collector: collector.clone(),
            collector_version: "local".to_owned(),
            profile_version: 4,
            collection_id: collection_id.clone(),
            observed_at,
        });

        assert_eq!(result.source, SourceKey::Cline);
        assert_eq!(result.collector, collector);
        assert_eq!(result.collector_version, "local");
        assert_eq!(result.profile_version, 4);
        assert_eq!(result.collection_id, collection_id);
        assert_eq!(result.observed_at, observed_at);
        assert_eq!(result.data_quality, DataQuality::Complete);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn detects_dates_inside_incremental_scope() {
        let scope =
            CollectionScope::incremental(date(2026, 7, 2), date(2026, 7, 4)).expect("scope");

        assert!(date_in_scope(date(2026, 7, 2), &scope));
        assert!(date_in_scope(date(2026, 7, 3), &scope));
        assert!(date_in_scope(date(2026, 7, 4), &scope));
        assert!(!date_in_scope(date(2026, 7, 1), &scope));
        assert!(!date_in_scope(date(2026, 7, 5), &scope));
        assert!(date_in_scope(date(2020, 1, 1), &CollectionScope::Full));
    }

    #[test]
    fn converts_milliseconds_to_utc_and_local_date() {
        let timestamp_ms = 1_782_935_999_000;

        let timestamp =
            utc_from_millis(timestamp_ms, Error::InvalidTimestamp).expect("valid timestamp");
        let jakarta_date = local_date_from_millis(
            timestamp_ms,
            "Asia/Jakarta".parse::<Tz>().expect("timezone"),
            Error::InvalidTimestamp,
        )
        .expect("local date");

        assert_eq!(timestamp.timestamp_millis(), timestamp_ms);
        assert_eq!(jakarta_date, date(2026, 7, 2));
        assert_eq!(
            utc_from_millis(i64::MAX, Error::InvalidTimestamp).expect_err("invalid timestamp"),
            Error::InvalidTimestamp
        );
    }

    #[test]
    fn checked_add_reports_overflow() {
        assert_eq!(checked_add_u64(40, 2, Error::Overflow), Ok(42));
        assert_eq!(
            checked_add_u64(u64::MAX, 1, Error::Overflow).expect_err("overflow"),
            Error::Overflow
        );
    }

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("date")
    }
}
