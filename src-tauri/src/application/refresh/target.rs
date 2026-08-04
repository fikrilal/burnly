//! Refresh target catalog and target-level helper functions.
//!
//! Defines the supported source/projection pairs that the coordinator refreshes
//! and provides deterministic helpers for collection IDs, import timezones, and
//! record counting. These helpers have no threading or persistence side effects.

use std::str::FromStr;

use chrono::{DateTime, NaiveDate, Utc};
use chrono_tz::Tz;

use crate::application::collection::{CollectionProjection, CollectionResult};
use crate::application::reconciliation::ImportRunLookup;
use crate::domain::source::SourceKey;

use super::outcome::clamp_count;
use super::planner::RefreshPlanTarget;

/// A single source/projection pair that the coordinator refreshes.
#[derive(Debug, Clone, Copy)]
pub(super) struct RefreshTarget {
    pub(super) source: SourceKey,
    pub(super) projection: CollectionProjection,
}

impl RefreshTarget {
    pub(super) fn plan_target(self, aggregation_timezone: &str) -> RefreshPlanTarget {
        match self.projection {
            CollectionProjection::Daily => {
                RefreshPlanTarget::daily(self.source, aggregation_timezone.to_owned())
            }
            CollectionProjection::Session => RefreshPlanTarget::session(self.source),
        }
    }

    pub(super) fn import_lookup(
        self,
        aggregation_timezone: &str,
    ) -> Result<ImportRunLookup, crate::application::reconciliation::RunValidationError> {
        ImportRunLookup::new(
            self.source,
            self.projection,
            match self.projection {
                CollectionProjection::Daily => Some(aggregation_timezone.to_owned()),
                CollectionProjection::Session => None,
            },
        )
    }
}

/// All supported source/projection pairs refreshed by the coordinator.
pub(super) const fn refresh_targets() -> [RefreshTarget; 16] {
    [
        RefreshTarget {
            source: SourceKey::ClaudeCode,
            projection: CollectionProjection::Daily,
        },
        RefreshTarget {
            source: SourceKey::ClaudeCode,
            projection: CollectionProjection::Session,
        },
        RefreshTarget {
            source: SourceKey::Codex,
            projection: CollectionProjection::Daily,
        },
        RefreshTarget {
            source: SourceKey::Codex,
            projection: CollectionProjection::Session,
        },
        RefreshTarget {
            source: SourceKey::OpenCode,
            projection: CollectionProjection::Daily,
        },
        RefreshTarget {
            source: SourceKey::OpenCode,
            projection: CollectionProjection::Session,
        },
        RefreshTarget {
            source: SourceKey::Pi,
            projection: CollectionProjection::Daily,
        },
        RefreshTarget {
            source: SourceKey::Pi,
            projection: CollectionProjection::Session,
        },
        RefreshTarget {
            source: SourceKey::Cline,
            projection: CollectionProjection::Daily,
        },
        RefreshTarget {
            source: SourceKey::Cline,
            projection: CollectionProjection::Session,
        },
        RefreshTarget {
            source: SourceKey::ZCode,
            projection: CollectionProjection::Daily,
        },
        RefreshTarget {
            source: SourceKey::ZCode,
            projection: CollectionProjection::Session,
        },
        RefreshTarget {
            source: SourceKey::Antigravity,
            projection: CollectionProjection::Daily,
        },
        RefreshTarget {
            source: SourceKey::Antigravity,
            projection: CollectionProjection::Session,
        },
        RefreshTarget {
            source: SourceKey::GrokBuild,
            projection: CollectionProjection::Daily,
        },
        RefreshTarget {
            source: SourceKey::GrokBuild,
            projection: CollectionProjection::Session,
        },
    ]
}

pub(super) const fn projection_label(projection: CollectionProjection) -> &'static str {
    match projection {
        CollectionProjection::Daily => "daily",
        CollectionProjection::Session => "session",
    }
}

pub(super) fn import_timezone(projection: CollectionProjection, timezone: &str) -> Option<String> {
    match projection {
        CollectionProjection::Daily => Some(timezone.to_owned()),
        CollectionProjection::Session => None,
    }
}

pub(super) fn local_date(
    requested_at: DateTime<Utc>,
    aggregation_timezone: &str,
) -> Result<NaiveDate, ()> {
    let timezone = Tz::from_str(aggregation_timezone).map_err(|_| ())?;
    Ok(requested_at.with_timezone(&timezone).date_naive())
}

pub(super) fn records_seen(collection: &CollectionResult) -> u32 {
    let count = match collection.projection() {
        CollectionProjection::Daily => collection.daily_candidates().len(),
        CollectionProjection::Session => collection.session_candidates().len(),
    };
    clamp_count(count)
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    #[test]
    fn target_catalog_contains_each_supported_source_projection_pair() {
        let targets = refresh_targets();

        assert_eq!(targets.len(), 16);
        assert_eq!(
            targets
                .iter()
                .filter(|target| target.projection == CollectionProjection::Daily)
                .count(),
            8
        );
        assert_eq!(
            targets
                .iter()
                .filter(|target| target.projection == CollectionProjection::Session)
                .count(),
            8
        );

        for source in [
            SourceKey::ClaudeCode,
            SourceKey::Codex,
            SourceKey::OpenCode,
            SourceKey::Pi,
            SourceKey::Cline,
            SourceKey::ZCode,
            SourceKey::Antigravity,
            SourceKey::GrokBuild,
        ] {
            assert!(targets.iter().any(|target| target.source == source
                && target.projection == CollectionProjection::Daily));
            assert!(targets.iter().any(|target| target.source == source
                && target.projection == CollectionProjection::Session));
        }
    }

    #[test]
    fn command_code_is_not_yet_a_refresh_target() {
        let targets = refresh_targets();

        assert_eq!(targets.len(), 16);
        assert!(!targets
            .iter()
            .any(|target| target.source == SourceKey::CommandCode));
    }

    #[test]
    fn import_timezone_is_only_stored_for_daily_targets() {
        assert_eq!(
            import_timezone(CollectionProjection::Daily, "Asia/Jakarta"),
            Some("Asia/Jakarta".to_owned())
        );
        assert_eq!(
            import_timezone(CollectionProjection::Session, "Asia/Jakarta"),
            None
        );
    }

    #[test]
    fn import_lookup_uses_timezone_only_for_daily_targets() {
        let daily = RefreshTarget {
            source: SourceKey::Codex,
            projection: CollectionProjection::Daily,
        }
        .import_lookup("Asia/Jakarta")
        .expect("daily import lookup");
        assert_eq!(daily.aggregation_timezone(), Some("Asia/Jakarta"));

        let session = RefreshTarget {
            source: SourceKey::Codex,
            projection: CollectionProjection::Session,
        }
        .import_lookup("Asia/Jakarta")
        .expect("session import lookup");
        assert_eq!(session.aggregation_timezone(), None);
    }

    #[test]
    fn local_date_uses_requested_aggregation_timezone() {
        let requested_at = Utc
            .with_ymd_and_hms(2026, 7, 4, 18, 0, 0)
            .single()
            .expect("timestamp");

        assert_eq!(
            local_date(requested_at, "UTC").expect("utc date"),
            NaiveDate::from_ymd_opt(2026, 7, 4).expect("utc expected date")
        );
        assert_eq!(
            local_date(requested_at, "Asia/Jakarta").expect("jakarta date"),
            NaiveDate::from_ymd_opt(2026, 7, 5).expect("jakarta expected date")
        );
    }

    #[test]
    fn local_date_rejects_invalid_timezone() {
        let requested_at = Utc
            .with_ymd_and_hms(2026, 7, 4, 18, 0, 0)
            .single()
            .expect("timestamp");

        assert!(local_date(requested_at, "not-a-timezone").is_err());
    }
}
