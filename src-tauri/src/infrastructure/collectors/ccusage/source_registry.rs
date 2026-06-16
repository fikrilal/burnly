use crate::application::collection::CollectorFailure;
use crate::domain::source::SourceKey;

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

const CODEX: SourceDescriptor = SourceDescriptor {
    source: SourceKey::Codex,
    display_name: "Codex",
    command_namespace: "codex",
    default_enabled: false,
    release_stage: ReleaseStage::Experimental,
    profile_version: 1,
};

pub(crate) fn source_descriptor(
    source: SourceKey,
) -> Result<&'static SourceDescriptor, CollectorFailure> {
    match source {
        SourceKey::ClaudeCode => Ok(&CLAUDE_CODE),
        SourceKey::Codex => Ok(&CODEX),
        #[cfg(test)]
        SourceKey::TestUnsupported => Err(CollectorFailure::new(
            crate::application::collection::CollectorFailureCode::UnsupportedSource,
            Some(source),
            None,
        )),
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
    fn codex_maps_to_reviewed_command_namespace() {
        let descriptor = source_descriptor(SourceKey::Codex).expect("source descriptor");

        assert_eq!(descriptor.command_namespace, "codex");
        assert_eq!(descriptor.release_stage, ReleaseStage::Experimental);
        assert!(!descriptor.default_enabled);
        assert_eq!(descriptor.profile_version, 1);
    }
}
