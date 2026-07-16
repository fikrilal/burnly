//! Upload scope values and pending-scope merge rules.

use std::collections::BTreeSet;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// Application upload scope derived from product policy / refresh outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UploadScope {
    Full,
    Incremental {
        /// Successful daily source keys included in this scope.
        source_keys: BTreeSet<String>,
        start_date: NaiveDate,
        end_date: NaiveDate,
    },
}

/// Durable JSON shape for pending scope (avoids chrono serde dependency).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum StoredUploadScope {
    Full,
    Incremental {
        source_keys: Vec<String>,
        start_date: String,
        end_date: String,
    },
}

impl From<&UploadScope> for StoredUploadScope {
    fn from(value: &UploadScope) -> Self {
        match value {
            UploadScope::Full => Self::Full,
            UploadScope::Incremental {
                source_keys,
                start_date,
                end_date,
            } => Self::Incremental {
                source_keys: source_keys.iter().cloned().collect(),
                start_date: start_date.to_string(),
                end_date: end_date.to_string(),
            },
        }
    }
}

impl TryFrom<StoredUploadScope> for UploadScope {
    type Error = ScopeError;

    fn try_from(value: StoredUploadScope) -> Result<Self, Self::Error> {
        match value {
            StoredUploadScope::Full => Ok(Self::Full),
            StoredUploadScope::Incremental {
                source_keys,
                start_date,
                end_date,
            } => {
                let start_date = NaiveDate::parse_from_str(&start_date, "%Y-%m-%d")
                    .map_err(|_| ScopeError::InvalidStoredDate)?;
                let end_date = NaiveDate::parse_from_str(&end_date, "%Y-%m-%d")
                    .map_err(|_| ScopeError::InvalidStoredDate)?;
                Self::incremental(source_keys, start_date, end_date)
            }
        }
    }
}

impl UploadScope {
    pub(crate) fn incremental(
        source_keys: impl IntoIterator<Item = String>,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<Self, ScopeError> {
        if end_date < start_date {
            return Err(ScopeError::InvertedDateRange);
        }
        let source_keys: BTreeSet<String> = source_keys
            .into_iter()
            .map(|key| key.trim().to_owned())
            .filter(|key| !key.is_empty())
            .collect();
        if source_keys.is_empty() {
            return Err(ScopeError::EmptySourceKeys);
        }
        Ok(Self::Incremental {
            source_keys,
            start_date,
            end_date,
        })
    }

    #[allow(dead_code)] // used by later orchestration chunks
    pub(crate) const fn is_full(&self) -> bool {
        matches!(self, Self::Full)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum ScopeError {
    #[error("incremental upload scope requires at least one source key")]
    EmptySourceKeys,
    #[error("incremental upload scope end date must not precede start date")]
    InvertedDateRange,
    #[error("stored upload scope date is invalid")]
    InvalidStoredDate,
}

/// Merge a new scope into durable pending scope.
///
/// `Full` replaces any narrower scope. Incremental scopes union source keys and
/// expand the inclusive date range.
pub(crate) fn merge_upload_scopes(
    existing: Option<UploadScope>,
    incoming: UploadScope,
) -> UploadScope {
    match (existing, incoming) {
        (_, UploadScope::Full) | (Some(UploadScope::Full), _) => UploadScope::Full,
        (None, other) => other,
        (
            Some(UploadScope::Incremental {
                source_keys: mut existing_keys,
                start_date: existing_start,
                end_date: existing_end,
            }),
            UploadScope::Incremental {
                source_keys: incoming_keys,
                start_date: incoming_start,
                end_date: incoming_end,
            },
        ) => {
            existing_keys.extend(incoming_keys);
            UploadScope::Incremental {
                source_keys: existing_keys,
                start_date: existing_start.min(incoming_start),
                end_date: existing_end.max(incoming_end),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("date")
    }

    #[test]
    fn full_dominates_incremental() {
        let pending = UploadScope::incremental(
            ["claude-code".to_owned()],
            date(2026, 7, 1),
            date(2026, 7, 10),
        )
        .expect("scope");
        assert_eq!(
            merge_upload_scopes(Some(pending), UploadScope::Full),
            UploadScope::Full
        );
        assert_eq!(
            merge_upload_scopes(Some(UploadScope::Full), pending_scope()),
            UploadScope::Full
        );
    }

    #[test]
    fn incremental_scopes_merge_sources_and_dates() {
        let first = UploadScope::incremental(
            ["claude-code".to_owned()],
            date(2026, 7, 5),
            date(2026, 7, 8),
        )
        .expect("first");
        let second = UploadScope::incremental(
            ["codex".to_owned(), "claude-code".to_owned()],
            date(2026, 7, 1),
            date(2026, 7, 6),
        )
        .expect("second");
        let merged = merge_upload_scopes(Some(first), second);
        assert_eq!(
            merged,
            UploadScope::Incremental {
                source_keys: BTreeSet::from(["claude-code".to_owned(), "codex".to_owned()]),
                start_date: date(2026, 7, 1),
                end_date: date(2026, 7, 8),
            }
        );
    }

    fn pending_scope() -> UploadScope {
        UploadScope::incremental(
            ["claude-code".to_owned()],
            date(2026, 7, 1),
            date(2026, 7, 2),
        )
        .expect("scope")
    }
}
