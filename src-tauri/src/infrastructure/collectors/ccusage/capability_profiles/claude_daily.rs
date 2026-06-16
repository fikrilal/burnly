use crate::application::collection::CollectionProjection;
use crate::domain::source::SourceKey;

use super::{
    CapabilityProfile, CapabilityState, CostCapability, CostProvenance, DateFilterBehavior,
    EmptyOutputBehavior, MissingPricingStrategy, ProjectIdentityCapability, ReportProfile,
    TokenCapabilities,
};

const SUPPORTED_PROJECTIONS: &[CollectionProjection] =
    &[CollectionProjection::Daily, CollectionProjection::Session];

pub(crate) const CLAUDE_DAILY_PROFILE: CapabilityProfile = CapabilityProfile {
    source: SourceKey::ClaudeCode,
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
    fn profile_records_reviewed_claude_daily_semantics() {
        let daily = CLAUDE_DAILY_PROFILE.daily.unwrap();

        assert_eq!(daily.report_name, "daily");
        assert_eq!(daily.envelope_key, "daily");
        assert_eq!(daily.aggregation_timezone, CapabilityState::Supported);
        assert_eq!(
            daily.project_identity,
            ProjectIdentityCapability::Unavailable
        );
        assert_eq!(
            daily.token_categories.reasoning_output,
            CapabilityState::Unsupported
        );
        assert_eq!(
            daily.cost.provenance,
            CostProvenance::CollectorCalculatedOffline
        );
        assert_eq!(
            daily.empty_output,
            EmptyOutputBehavior::ValidEmptyCollection
        );
    }
}
