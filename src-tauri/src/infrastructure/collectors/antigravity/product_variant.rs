#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(
    dead_code,
    reason = "variant metadata is introduced before runtime discovery uses it"
)]
pub(crate) enum AntigravityProductVariant {
    App,
    Ide,
    Cli,
}

impl AntigravityProductVariant {
    #[allow(
        dead_code,
        reason = "variant metadata is introduced before runtime discovery uses it"
    )]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::App => "antigravity",
            Self::Ide => "antigravity-ide",
            Self::Cli => "antigravity-cli",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variants_have_stable_metadata_values() {
        assert_eq!(AntigravityProductVariant::App.as_str(), "antigravity");
        assert_eq!(AntigravityProductVariant::Ide.as_str(), "antigravity-ide");
        assert_eq!(AntigravityProductVariant::Cli.as_str(), "antigravity-cli");
    }
}
