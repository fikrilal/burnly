#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

    pub(crate) const fn data_dir_name(self) -> &'static str {
        self.as_str()
    }

    pub(crate) const fn all() -> [Self; 3] {
        [Self::App, Self::Ide, Self::Cli]
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

    #[test]
    fn variants_have_local_data_directory_names() {
        let names = AntigravityProductVariant::all().map(AntigravityProductVariant::data_dir_name);

        assert_eq!(names, ["antigravity", "antigravity-ide", "antigravity-cli"]);
    }
}
