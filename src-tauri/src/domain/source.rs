#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum SourceKey {
    ClaudeCode,
    Codex,
}

impl SourceKey {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude-code",
            Self::Codex => "codex",
        }
    }

    pub(crate) fn from_storage(value: &str) -> Option<Self> {
        match value {
            "claude-code" => Some(Self::ClaudeCode),
            "codex" => Some(Self::Codex),
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
    }
}
