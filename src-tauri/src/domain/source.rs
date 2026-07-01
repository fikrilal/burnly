#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum SourceKey {
    ClaudeCode,
    Codex,
    OpenCode,
    Pi,
    Cline,
    #[cfg(test)]
    TestUnsupported,
}

impl SourceKey {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude-code",
            Self::Codex => "codex",
            Self::OpenCode => "opencode",
            Self::Pi => "pi",
            Self::Cline => "cline",
            #[cfg(test)]
            Self::TestUnsupported => "test-unsupported",
        }
    }

    pub(crate) fn from_storage(value: &str) -> Option<Self> {
        match value {
            "claude-code" => Some(Self::ClaudeCode),
            "codex" => Some(Self::Codex),
            "opencode" => Some(Self::OpenCode),
            "pi" => Some(Self::Pi),
            "cline" => Some(Self::Cline),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_key_has_stable_product_identity() {
        assert_eq!(SourceKey::ClaudeCode.as_str(), "claude-code");
        assert_eq!(SourceKey::Codex.as_str(), "codex");
        assert_eq!(SourceKey::OpenCode.as_str(), "opencode");
        assert_eq!(SourceKey::Pi.as_str(), "pi");
        assert_eq!(SourceKey::Cline.as_str(), "cline");
    }

    #[test]
    fn source_key_round_trips_from_storage() {
        assert_eq!(
            SourceKey::from_storage(SourceKey::ClaudeCode.as_str()),
            Some(SourceKey::ClaudeCode)
        );
        assert_eq!(
            SourceKey::from_storage(SourceKey::Codex.as_str()),
            Some(SourceKey::Codex)
        );
        assert_eq!(
            SourceKey::from_storage(SourceKey::OpenCode.as_str()),
            Some(SourceKey::OpenCode)
        );
        assert_eq!(
            SourceKey::from_storage(SourceKey::Pi.as_str()),
            Some(SourceKey::Pi)
        );
        assert_eq!(
            SourceKey::from_storage(SourceKey::Cline.as_str()),
            Some(SourceKey::Cline)
        );
        assert_eq!(SourceKey::from_storage("unknown"), None);
    }
}
