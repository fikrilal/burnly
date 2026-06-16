use crate::domain::source::SourceKey;
use thiserror::Error;

use super::CollectionProjection;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct CollectorKey(String);

impl CollectorKey {
    pub(crate) fn new(value: impl Into<String>) -> Result<Self, DescriptorValidationError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(DescriptorValidationError::EmptyCollectorKey);
        }

        Ok(Self(value))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CollectorDescriptor {
    pub collector: CollectorKey,
    pub display_name: String,
    pub runtime_version: String,
    pub expected_version: String,
    pub adapter_version: u16,
    pub binary_target: String,
    pub integrity: CollectorIntegrity,
    pub profiles: Vec<ProfileDescriptor>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CollectorIntegrity {
    Verified,
    Mismatch,
    UnverifiedDevelopment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProfileDescriptor {
    pub source: SourceKey,
    pub profile_version: u16,
    pub supported_projections: Vec<CollectionProjection>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub(crate) enum DescriptorValidationError {
    #[error("collector key must not be empty")]
    EmptyCollectorKey,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collector_key_is_opaque_and_non_empty() {
        let key = CollectorKey::new("fixture-collector").expect("collector key");

        assert_eq!(key.as_str(), "fixture-collector");
        assert_eq!(
            CollectorKey::new(" ").expect_err("empty collector key"),
            DescriptorValidationError::EmptyCollectorKey
        );
    }
}
