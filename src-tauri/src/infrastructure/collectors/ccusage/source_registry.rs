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

const PI: SourceDescriptor = SourceDescriptor {
    source: SourceKey::Pi,
    display_name: "Pi",
    command_namespace: "pi",
    default_enabled: true,
    release_stage: ReleaseStage::Supported,
    profile_version: 1,
};

pub(crate) fn source_descriptor(
    source: SourceKey,
) -> Result<&'static SourceDescriptor, CollectorFailure> {
    match source {
        SourceKey::ClaudeCode => Ok(&CLAUDE_CODE),
        SourceKey::Codex => Ok(&CODEX),
        SourceKey::Pi => Ok(&PI),
        SourceKey::OpenCode
        | SourceKey::Cline
        | SourceKey::ZCode
        | SourceKey::Antigravity
        | SourceKey::GrokBuild
        | SourceKey::CommandCode
        | SourceKey::Zed => Err(CollectorFailure::new(
            crate::application::collection::CollectorFailureCode::UnsupportedSource,
            Some(source),
            None,
        )),
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

    #[test]
    fn pi_maps_to_reviewed_command_namespace() {
        let descriptor = source_descriptor(SourceKey::Pi).expect("source descriptor");

        assert_eq!(descriptor.command_namespace, "pi");
        assert_eq!(descriptor.display_name, "Pi");
        assert_eq!(descriptor.release_stage, ReleaseStage::Supported);
        assert!(descriptor.default_enabled);
        assert_eq!(descriptor.profile_version, 1);
    }

    #[test]
    fn native_sources_are_not_routed_through_ccusage() {
        let opencode = source_descriptor(SourceKey::OpenCode).expect_err("unsupported source");
        let cline = source_descriptor(SourceKey::Cline).expect_err("unsupported source");
        let zcode = source_descriptor(SourceKey::ZCode).expect_err("unsupported source");
        let antigravity =
            source_descriptor(SourceKey::Antigravity).expect_err("unsupported source");
        let grok_build = source_descriptor(SourceKey::GrokBuild).expect_err("unsupported source");
        let command_code =
            source_descriptor(SourceKey::CommandCode).expect_err("unsupported source");
        let zed = source_descriptor(SourceKey::Zed).expect_err("unsupported source");

        assert_eq!(
            opencode.code,
            crate::application::collection::CollectorFailureCode::UnsupportedSource
        );
        assert_eq!(
            cline.code,
            crate::application::collection::CollectorFailureCode::UnsupportedSource
        );
        assert_eq!(
            zcode.code,
            crate::application::collection::CollectorFailureCode::UnsupportedSource
        );
        assert_eq!(
            antigravity.code,
            crate::application::collection::CollectorFailureCode::UnsupportedSource
        );
        assert_eq!(
            grok_build.code,
            crate::application::collection::CollectorFailureCode::UnsupportedSource
        );
        assert_eq!(
            command_code.code,
            crate::application::collection::CollectorFailureCode::UnsupportedSource
        );
        assert_eq!(
            zed.code,
            crate::application::collection::CollectorFailureCode::UnsupportedSource
        );
    }
}
