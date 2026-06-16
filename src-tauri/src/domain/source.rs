#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum SourceKey {
    ClaudeCode,
    Codex,
    OpenCode,
    #[cfg(test)]
    TestUnsupported,
}

impl SourceKey {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude-code",
            Self::Codex => "codex",
            Self::OpenCode => "opencode",
            #[cfg(test)]
            Self::TestUnsupported => "test-unsupported",
        }
    }

    pub(crate) fn from_storage(value: &str) -> Option<Self> {
        match value {
            "claude-code" => Some(Self::ClaudeCode),
            "codex" => Some(Self::Codex),
            "opencode" => Some(Self::OpenCode),
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
    }
}
