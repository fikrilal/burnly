use std::str::FromStr;

use chrono_tz::Tz;

const MIN_REFRESH_INTERVAL_MINUTES: i64 = 5;
const MAX_REFRESH_INTERVAL_MINUTES: i64 = 1_440;

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
    reporting_timezone: String,
    background_refresh_enabled: bool,
    refresh_interval_minutes: i64,
    launch_at_login: bool,
    close_behavior: CloseBehavior,
    notifications_enabled: bool,
    store_project_paths: bool,
}

impl Settings {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        reporting_timezone: String,
        background_refresh_enabled: bool,
        refresh_interval_minutes: i64,
        launch_at_login: bool,
        close_behavior: &str,
        notifications_enabled: bool,
        store_project_paths: bool,
    ) -> Result<Self, SettingsValidationError> {
        let reporting_timezone = reporting_timezone.trim().to_owned();
        Tz::from_str(&reporting_timezone)
            .map_err(|_| SettingsValidationError::ReportingTimezone)?;
        if !(MIN_REFRESH_INTERVAL_MINUTES..=MAX_REFRESH_INTERVAL_MINUTES)
            .contains(&refresh_interval_minutes)
        {
            return Err(SettingsValidationError::RefreshInterval);
        }

        Ok(Self {
            reporting_timezone,
            background_refresh_enabled,
            refresh_interval_minutes,
            launch_at_login,
            close_behavior: CloseBehavior::parse(close_behavior)?,
            notifications_enabled,
            store_project_paths,
        })
    }

    pub(crate) fn reporting_timezone(&self) -> &str {
        &self.reporting_timezone
    }

    pub(crate) const fn background_refresh_enabled(&self) -> bool {
        self.background_refresh_enabled
    }

    pub(crate) const fn refresh_interval_minutes(&self) -> i64 {
        self.refresh_interval_minutes
    }

    pub(crate) const fn launch_at_login(&self) -> bool {
        self.launch_at_login
    }

    pub(crate) const fn close_behavior(&self) -> CloseBehavior {
        self.close_behavior
    }

    pub(crate) const fn notifications_enabled(&self) -> bool {
        self.notifications_enabled
    }

    pub(crate) const fn store_project_paths(&self) -> bool {
        self.store_project_paths
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
    ReportingTimezone,
    RefreshInterval,
    CloseBehavior,
    Revision,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_settings() -> Settings {
        Settings::new(
            "Asia/Jakarta".to_owned(),
            true,
            30,
            false,
            "hide",
            false,
            false,
        )
        .expect("valid settings")
    }

    #[test]
    fn validates_timezone_interval_and_close_behavior() {
        assert_eq!(valid_settings().reporting_timezone(), "Asia/Jakarta");
        assert_eq!(
            Settings::new(
                "Not/AZone".to_owned(),
                false,
                15,
                false,
                "quit",
                false,
                false
            ),
            Err(SettingsValidationError::ReportingTimezone)
        );
        assert_eq!(
            Settings::new("UTC".to_owned(), false, 1, false, "quit", false, false),
            Err(SettingsValidationError::RefreshInterval)
        );
        assert_eq!(
            Settings::new("UTC".to_owned(), false, 15, false, "close", false, false),
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
