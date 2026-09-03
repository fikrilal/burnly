#![allow(
    dead_code,
    reason = "chunk 01 defines baseline store contracts consumed in subsequent chunks"
)]

use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum AntigravityBaselineVariant {
    App,
    Ide,
    Cli,
}

impl AntigravityBaselineVariant {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::App => "antigravity",
            Self::Ide => "antigravity-ide",
            Self::Cli => "antigravity-cli",
        }
    }

    pub(crate) fn from_str(s: &str) -> Option<Self> {
        match s {
            "antigravity" => Some(Self::App),
            "antigravity-ide" => Some(Self::Ide),
            "antigravity-cli" => Some(Self::Cli),
            _ => None,
        }
    }

    pub(crate) const fn all() -> [Self; 3] {
        [Self::App, Self::Ide, Self::Cli]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AntigravityBaselineStatus {
    Pending,
    Complete,
}

impl AntigravityBaselineStatus {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Complete => "complete",
        }
    }

    pub(crate) fn from_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Self::Pending),
            "complete" => Some(Self::Complete),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AntigravityBaselineRecord {
    pub(crate) variant: AntigravityBaselineVariant,
    pub(crate) status: AntigravityBaselineStatus,
    pub(crate) started_at_ms: i64,
    pub(crate) completed_at_ms: Option<i64>,
    pub(crate) updated_at_ms: i64,
}

#[derive(Debug, Error)]
pub(crate) enum AntigravityBaselineStoreError {
    #[error("baseline store database error: {0}")]
    Database(String),
}

pub(crate) trait AntigravityBaselineStore: Send + Sync {
    fn get_status(
        &self,
        variant: AntigravityBaselineVariant,
    ) -> Result<Option<AntigravityBaselineStatus>, AntigravityBaselineStoreError>;

    fn begin_baseline(
        &self,
        variant: AntigravityBaselineVariant,
        started_at_ms: i64,
    ) -> Result<(), AntigravityBaselineStoreError>;

    fn complete_baseline(
        &self,
        variant: AntigravityBaselineVariant,
        completed_at_ms: i64,
    ) -> Result<(), AntigravityBaselineStoreError>;

    fn complete_all_variants(
        &self,
        completed_at_ms: i64,
    ) -> Result<(), AntigravityBaselineStoreError> {
        for variant in AntigravityBaselineVariant::all() {
            self.complete_baseline(variant, completed_at_ms)?;
        }
        Ok(())
    }

    fn list_statuses(
        &self,
    ) -> Result<Vec<AntigravityBaselineRecord>, AntigravityBaselineStoreError>;
}

#[derive(Debug, Default)]
pub(crate) struct NoopAntigravityBaselineStore;

impl AntigravityBaselineStore for NoopAntigravityBaselineStore {
    fn get_status(
        &self,
        _variant: AntigravityBaselineVariant,
    ) -> Result<Option<AntigravityBaselineStatus>, AntigravityBaselineStoreError> {
        Ok(Some(AntigravityBaselineStatus::Complete))
    }

    fn begin_baseline(
        &self,
        _variant: AntigravityBaselineVariant,
        _started_at_ms: i64,
    ) -> Result<(), AntigravityBaselineStoreError> {
        Ok(())
    }

    fn complete_baseline(
        &self,
        _variant: AntigravityBaselineVariant,
        _completed_at_ms: i64,
    ) -> Result<(), AntigravityBaselineStoreError> {
        Ok(())
    }

    fn list_statuses(
        &self,
    ) -> Result<Vec<AntigravityBaselineRecord>, AntigravityBaselineStoreError> {
        Ok(Vec::new())
    }
}
