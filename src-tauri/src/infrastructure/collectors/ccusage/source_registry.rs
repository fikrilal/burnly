use crate::application::collection::CollectorFailure;
use crate::domain::source::SourceKey;

use super::capability_profiles::unsupported_source;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SourceDescriptor {
    pub source: SourceKey,
    pub display_name: &'static str,
    pub command_namespace: &'static str,
    pub default_enabled: bool,
    pub release_stage: ReleaseStage,
    pub profile_version: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReleaseStage {
    Supported,
    Experimental,
    Disabled,
}

const CLAUDE_CODE: SourceDescriptor = SourceDescriptor {
    source: SourceKey::ClaudeCode,
    display_name: "Claude Code",
    command_namespace: "claude",
    default_enabled: true,
    release_stage: ReleaseStage::Supported,
    profile_version: 1,
};

pub(crate) fn source_descriptor(
    source: SourceKey,
) -> Result<&'static SourceDescriptor, CollectorFailure> {
    match source {
        SourceKey::ClaudeCode => Ok(&CLAUDE_CODE),
        SourceKey::Codex => Err(unsupported_source(source)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_code_maps_to_reviewed_command_namespace() {
        let descriptor = source_descriptor(SourceKey::ClaudeCode).expect("source descriptor");

        assert_eq!(descriptor.command_namespace, "claude");
        assert_eq!(descriptor.release_stage, ReleaseStage::Supported);
        assert!(descriptor.default_enabled);
        assert_eq!(descriptor.profile_version, 1);
    }

    #[test]
    fn known_unregistered_source_is_rejected() {
        let error = source_descriptor(SourceKey::Codex).expect_err("codex is not registered");

        assert_eq!(
            error.code,
            crate::application::collection::CollectorFailureCode::UnsupportedSource
        );
    }
}
