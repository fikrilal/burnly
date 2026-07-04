use crate::application::collection::{
    CollectionProjection, CollectorDescriptor, CollectorFailure, CollectorFailureCode,
    CollectorIntegrity, CollectorKey, ProfileDescriptor,
};
use crate::domain::source::SourceKey;

#[derive(Debug, Clone, Copy)]
pub(in crate::infrastructure::collectors) struct CollectorIdentity {
    pub key: &'static str,
    pub display_name: &'static str,
    pub runtime_version: &'static str,
    pub adapter_version: u16,
    pub source: SourceKey,
    pub profile_version: u16,
}

pub(in crate::infrastructure::collectors) fn single_source_descriptor(
    identity: CollectorIdentity,
    supported_projections: Vec<CollectionProjection>,
    integrity: CollectorIntegrity,
) -> Result<CollectorDescriptor, CollectorFailure> {
    Ok(CollectorDescriptor {
        collector: collector_key(identity)?,
        display_name: identity.display_name.to_owned(),
        runtime_version: identity.runtime_version.to_owned(),
        expected_version: identity.runtime_version.to_owned(),
        adapter_version: identity.adapter_version,
        binary_target: std::env::consts::OS.to_owned(),
        integrity,
        profiles: vec![ProfileDescriptor {
            source: identity.source,
            profile_version: identity.profile_version,
            supported_projections,
        }],
    })
}

pub(in crate::infrastructure::collectors) fn daily_session_projections() -> Vec<CollectionProjection>
{
    vec![CollectionProjection::Daily, CollectionProjection::Session]
}

pub(in crate::infrastructure::collectors) fn collector_key(
    identity: CollectorIdentity,
) -> Result<CollectorKey, CollectorFailure> {
    CollectorKey::new(identity.key)
        .map_err(|_| CollectorFailure::new(CollectorFailureCode::Internal, None, None))
}

#[cfg(test)]
mod tests {
    use super::*;

    const IDENTITY: CollectorIdentity = CollectorIdentity {
        key: "test-source",
        display_name: "Test Source",
        runtime_version: "local",
        adapter_version: 7,
        source: SourceKey::ZCode,
        profile_version: 3,
    };

    #[test]
    fn builds_single_source_descriptor() {
        let descriptor = single_source_descriptor(
            IDENTITY,
            daily_session_projections(),
            CollectorIntegrity::UnverifiedDevelopment,
        )
        .expect("descriptor");

        assert_eq!(descriptor.collector.as_str(), "test-source");
        assert_eq!(descriptor.display_name, "Test Source");
        assert_eq!(descriptor.runtime_version, "local");
        assert_eq!(descriptor.expected_version, "local");
        assert_eq!(descriptor.adapter_version, 7);
        assert_eq!(descriptor.binary_target, std::env::consts::OS);
        assert_eq!(
            descriptor.integrity,
            CollectorIntegrity::UnverifiedDevelopment
        );
        assert_eq!(descriptor.profiles.len(), 1);
        assert_eq!(descriptor.profiles[0].source, SourceKey::ZCode);
        assert_eq!(descriptor.profiles[0].profile_version, 3);
        assert_eq!(
            descriptor.profiles[0].supported_projections,
            vec![CollectionProjection::Daily, CollectionProjection::Session]
        );
    }

    #[test]
    fn invalid_collector_key_maps_to_internal_failure() {
        let error = single_source_descriptor(
            CollectorIdentity {
                key: " ",
                ..IDENTITY
            },
            daily_session_projections(),
            CollectorIntegrity::UnverifiedDevelopment,
        )
        .expect_err("invalid key");

        assert_eq!(error.code, CollectorFailureCode::Internal);
        assert_eq!(error.source_key, None);
        assert_eq!(error.projection, None);
    }
}
