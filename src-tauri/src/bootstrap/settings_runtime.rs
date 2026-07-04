use tauri::Runtime;

use crate::application::bootstrap::{Capability, RuntimeCapabilities, RuntimeSettings};
use crate::application::settings::{RuntimeSettingError, SettingsRuntime};
use crate::domain::settings::Settings;

pub(super) struct DesktopSettingsRuntime<R: Runtime> {
    app: tauri::AppHandle<R>,
    runtime_settings: RuntimeSettings,
}

impl<R: Runtime> DesktopSettingsRuntime<R> {
    pub(super) fn new(app: tauri::AppHandle<R>, runtime_settings: RuntimeSettings) -> Self {
        Self {
            app,
            runtime_settings,
        }
    }

    pub(super) fn reconcile_launch_at_login_on_startup(
        &self,
        enabled: bool,
    ) -> Result<(), RuntimeSettingError> {
        if !should_reconcile_launch_at_login_on_startup(enabled, launch_at_login_supported()) {
            return Ok(());
        }

        self.apply_launch_at_login(true)
    }

    fn apply_launch_at_login(&self, enabled: bool) -> Result<(), RuntimeSettingError> {
        use tauri_plugin_autostart::ManagerExt;

        if enabled && !launch_at_login_supported() {
            return Err(RuntimeSettingError::LaunchAtLoginUnavailable);
        }

        let autostart = self.app.autolaunch();
        let result = if enabled {
            autostart.enable()
        } else {
            autostart.disable()
        };

        result.map_err(|_| RuntimeSettingError::LaunchAtLoginApplyFailed)
    }
}

impl<R: Runtime> SettingsRuntime for DesktopSettingsRuntime<R> {
    fn validate(&self, current: &Settings, proposed: &Settings) -> Result<(), RuntimeSettingError> {
        if proposed.launch_at_login() && !current.launch_at_login() && !launch_at_login_supported()
        {
            return Err(RuntimeSettingError::LaunchAtLoginUnavailable);
        }

        Ok(())
    }

    fn prepare_update(
        &self,
        current: &Settings,
        proposed: &Settings,
    ) -> Result<(), RuntimeSettingError> {
        if current.launch_at_login() != proposed.launch_at_login() {
            self.apply_launch_at_login(proposed.launch_at_login())?;
        }

        Ok(())
    }

    fn rollback_update(&self, current: &Settings) -> Result<(), RuntimeSettingError> {
        self.apply_launch_at_login(current.launch_at_login())
    }

    fn commit_update(&self, settings: &Settings) {
        self.runtime_settings.update(settings);
    }
}

fn should_reconcile_launch_at_login_on_startup(persisted_enabled: bool, supported: bool) -> bool {
    persisted_enabled && supported
}

fn launch_at_login_supported() -> bool {
    !cfg!(debug_assertions)
}

pub(super) fn launch_at_login_capability() -> Capability {
    if launch_at_login_supported() {
        RuntimeCapabilities::launch_at_login_available()
    } else {
        RuntimeCapabilities::launch_at_login_not_implemented()
    }
}

#[cfg(test)]
mod tests {
    use super::should_reconcile_launch_at_login_on_startup;

    #[test]
    fn launch_at_login_startup_reconciliation_policy_requires_enabled_and_supported() {
        assert!(should_reconcile_launch_at_login_on_startup(true, true));
        assert!(!should_reconcile_launch_at_login_on_startup(true, false));
        assert!(!should_reconcile_launch_at_login_on_startup(false, true));
        assert!(!should_reconcile_launch_at_login_on_startup(false, false));
    }
}
