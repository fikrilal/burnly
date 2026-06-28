#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CloseBehavior {
    Hide,
    Quit,
}

impl CloseBehavior {
    pub(crate) fn parse(value: &str) -> Result<Self, SettingsValidationError> {
        match value {
            "hide" => Ok(Self::Hide),
            "quit" => Ok(Self::Quit),
            _ => Err(SettingsValidationError::CloseBehavior),
        }
    }

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Hide => "hide",
            Self::Quit => "quit",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Settings {
    launch_at_login: bool,
    close_behavior: CloseBehavior,
}

impl Settings {
    pub(crate) fn new(
        launch_at_login: bool,
        close_behavior: &str,
    ) -> Result<Self, SettingsValidationError> {
        Ok(Self {
            launch_at_login,
            close_behavior: CloseBehavior::parse(close_behavior)?,
        })
    }

    pub(crate) const fn launch_at_login(&self) -> bool {
        self.launch_at_login
    }

    pub(crate) const fn close_behavior(&self) -> CloseBehavior {
        self.close_behavior
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SettingsDocument {
    settings: Settings,
    revision: i64,
}

impl SettingsDocument {
    pub(crate) fn new(settings: Settings, revision: i64) -> Result<Self, SettingsValidationError> {
        if revision <= 0 {
            return Err(SettingsValidationError::Revision);
        }
        Ok(Self { settings, revision })
    }

    pub(crate) const fn settings(&self) -> &Settings {
        &self.settings
    }

    pub(crate) const fn revision(&self) -> i64 {
        self.revision
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SettingsValidationError {
    CloseBehavior,
    Revision,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_settings() -> Settings {
        Settings::new(false, "hide").expect("valid settings")
    }

    #[test]
    fn validates_close_behavior() {
        assert_eq!(
            Settings::new(false, "close"),
            Err(SettingsValidationError::CloseBehavior)
        );
    }

    #[test]
    fn settings_document_requires_positive_revision() {
        assert_eq!(
            SettingsDocument::new(valid_settings(), 0),
            Err(SettingsValidationError::Revision)
        );
    }
}
