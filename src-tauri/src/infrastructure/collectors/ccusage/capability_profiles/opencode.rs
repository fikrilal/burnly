use crate::application::collection::CollectionProjection;
use crate::domain::source::SourceKey;

use super::{
    CapabilityProfile, CapabilityState, CostCapability, CostProvenance, DateFilterBehavior,
    EmptyOutputBehavior, MissingPricingStrategy, ProjectIdentityCapability, ReportProfile,
    TokenCapabilities,
};

const SUPPORTED_PROJECTIONS: &[CollectionProjection] =
    &[CollectionProjection::Daily, CollectionProjection::Session];

pub(crate) const OPENCODE_PROFILE: CapabilityProfile = CapabilityProfile {
    source: SourceKey::OpenCode,
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
            cache_creation: CapabilityState::Unsupported,
            cache_read: CapabilityState::Unsupported,
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
            cache_creation: CapabilityState::Unsupported,
            cache_read: CapabilityState::Unsupported,
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
