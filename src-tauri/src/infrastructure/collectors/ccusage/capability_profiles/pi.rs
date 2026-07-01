use crate::application::collection::CollectionProjection;
use crate::domain::source::SourceKey;

use super::{
    CapabilityProfile, CapabilityState, CostCapability, CostProvenance, DateFilterBehavior,
    EmptyOutputBehavior, MissingPricingStrategy, ProjectIdentityCapability, ReportProfile,
    TokenCapabilities,
};

const SUPPORTED_PROJECTIONS: &[CollectionProjection] =
    &[CollectionProjection::Daily, CollectionProjection::Session];

/// Pi shares the OpenCode-family token/cost shape but its ccusage reports emit
/// `cacheCreationTokens` and `cacheReadTokens`, so both cache categories are
/// Supported here. Reasoning is excluded from ccusage's aggregate `totalTokens`
/// for Pi, so `reasoning_output` stays Unsupported.
pub(crate) const PI_PROFILE: CapabilityProfile = CapabilityProfile {
    source: SourceKey::Pi,
    profile_version: 1,
    supported_projections: SUPPORTED_PROJECTIONS,
    daily: Some(ReportProfile {
        report_name: "daily",
        envelope_key: "daily",
        date_filter: DateFilterBehavior::InclusiveCalendarDates,
        aggregation_timezone: CapabilityState::Supported,
        model_identity: CapabilityState::Supported,
        project_identity: ProjectIdentityCapability::Unavailable,
        token_categories: TokenCapabilities {
            input: CapabilityState::Supported,
            output: CapabilityState::Supported,
            cache_creation: CapabilityState::Supported,
            cache_read: CapabilityState::Supported,
            reasoning_output: CapabilityState::Unsupported,
        },
        cost: CostCapability {
            state: CapabilityState::Supported,
            provenance: CostProvenance::CollectorCalculatedOffline,
            missing_pricing: MissingPricingStrategy::PositiveUsageWithZeroCostIsUnavailable,
        },
        empty_output: EmptyOutputBehavior::ValidEmptyCollection,
    }),
    session: Some(ReportProfile {
        report_name: "session",
        envelope_key: "sessions",
        date_filter: DateFilterBehavior::InclusiveCalendarDates,
        aggregation_timezone: CapabilityState::Supported,
        model_identity: CapabilityState::Supported,
        project_identity: ProjectIdentityCapability::Unavailable,
        token_categories: TokenCapabilities {
            input: CapabilityState::Supported,
            output: CapabilityState::Supported,
            cache_creation: CapabilityState::Supported,
            cache_read: CapabilityState::Supported,
            reasoning_output: CapabilityState::Unsupported,
        },
        cost: CostCapability {
            state: CapabilityState::Supported,
            provenance: CostProvenance::CollectorCalculatedOffline,
            missing_pricing: MissingPricingStrategy::PositiveUsageWithZeroCostIsUnavailable,
        },
        empty_output: EmptyOutputBehavior::ValidEmptyCollection,
    }),
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_records_reviewed_pi_semantics() {
        let daily = PI_PROFILE.daily.unwrap();

        assert_eq!(daily.report_name, "daily");
        assert_eq!(daily.envelope_key, "daily");
        assert_eq!(
            daily.token_categories.cache_read,
            CapabilityState::Supported
        );
        assert_eq!(
            daily.token_categories.reasoning_output,
            CapabilityState::Unsupported
        );

        let session = PI_PROFILE.session.unwrap();
        assert_eq!(session.report_name, "session");
        assert_eq!(session.envelope_key, "sessions");
    }
}
