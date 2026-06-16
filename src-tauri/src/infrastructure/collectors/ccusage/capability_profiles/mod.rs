use crate::application::collection::{
    CollectionProjection, CollectorFailure, CollectorFailureCode,
};
use crate::domain::source::SourceKey;

mod claude_daily;
mod codex;
mod opencode;

pub(crate) use claude_daily::CLAUDE_DAILY_PROFILE;
pub(crate) use codex::CODEX_PROFILE;
pub(crate) use opencode::OPENCODE_PROFILE;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CapabilityProfile {
    pub source: SourceKey,
    pub profile_version: u16,
    pub supported_projections: &'static [CollectionProjection],
    pub daily: Option<ReportProfile>,
    pub session: Option<ReportProfile>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ReportProfile {
    pub report_name: &'static str,
    pub envelope_key: &'static str,
    pub date_filter: DateFilterBehavior,
    pub aggregation_timezone: CapabilityState,
    pub model_identity: CapabilityState,
    pub project_identity: ProjectIdentityCapability,
    pub token_categories: TokenCapabilities,
    pub cost: CostCapability,
    pub empty_output: EmptyOutputBehavior,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CapabilityState {
    Supported,
    Unsupported,
    Conditional,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DateFilterBehavior {
    InclusiveCalendarDates,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProjectIdentityCapability {
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TokenCapabilities {
    pub input: CapabilityState,
    pub output: CapabilityState,
    pub cache_creation: CapabilityState,
    pub cache_read: CapabilityState,
    pub reasoning_output: CapabilityState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CostCapability {
    pub state: CapabilityState,
    pub provenance: CostProvenance,
    pub missing_pricing: MissingPricingStrategy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CostProvenance {
    CollectorCalculatedOffline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MissingPricingStrategy {
    PositiveUsageWithZeroCostIsUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EmptyOutputBehavior {
    ValidEmptyCollection,
}

pub(crate) const fn profiles() -> &'static [CapabilityProfile] {
    &[CLAUDE_DAILY_PROFILE, CODEX_PROFILE, OPENCODE_PROFILE]
}

pub(crate) fn profile_for(
    source: SourceKey,
    projection: CollectionProjection,
) -> Result<&'static CapabilityProfile, CollectorFailure> {
    let profile = profiles().iter().find(|profile| profile.source == source);
    let Some(profile) = profile else {
        return Err(unsupported_source(source));
    };

    if !profile.supported_projections.contains(&projection) {
        return Err(CollectorFailure::new(
            CollectorFailureCode::UnsupportedProjection,
            Some(source),
            Some(projection),
        ));
    }

    Ok(profile)
}

pub(super) fn unsupported_source(source: SourceKey) -> CollectorFailure {
    CollectorFailure::new(CollectorFailureCode::UnsupportedSource, Some(source), None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supports_claude_daily_and_session() {
        assert!(profile_for(SourceKey::ClaudeCode, CollectionProjection::Daily).is_ok());
        assert!(profile_for(SourceKey::ClaudeCode, CollectionProjection::Session).is_ok());
    }

    #[test]
    fn supports_codex_daily_and_session() {
        assert!(profile_for(SourceKey::Codex, CollectionProjection::Daily).is_ok());
        assert!(profile_for(SourceKey::Codex, CollectionProjection::Session).is_ok());
    }

    #[test]
    fn supports_opencode_daily_and_session() {
        assert!(profile_for(SourceKey::OpenCode, CollectionProjection::Daily).is_ok());
        assert!(profile_for(SourceKey::OpenCode, CollectionProjection::Session).is_ok());
    }
}
