use tauri::{Manager, Runtime};
use tauri_plugin_opener::OpenerExt;

use crate::application::ports::log_reveal::{
    LogRevealAvailability, LogRevealCapability, LogRevealError, LogRevealOutcome, LogRevealPort,
};

pub(crate) struct DesktopLogReveal<R: Runtime> {
    app: tauri::AppHandle<R>,
}

impl<R: Runtime> DesktopLogReveal<R> {
    pub(crate) fn new(app: tauri::AppHandle<R>) -> Self {
        Self { app }
    }
}

impl<R: Runtime> LogRevealPort for DesktopLogReveal<R> {
    fn capability(&self) -> LogRevealCapability {
        let status = match self.log_directory_exists() {
            Ok(true) => LogRevealAvailability::Available,
            Ok(false) => LogRevealAvailability::Missing,
            Err(_) => LogRevealAvailability::Unsupported,
        };
        LogRevealCapability {
            status,
            label: "Burnly logs".to_owned(),
        }
    }

    fn reveal_logs(&self) -> Result<LogRevealOutcome, LogRevealError> {
        let path = match self.app.path().app_log_dir() {
            Ok(path) => path,
            Err(_) => return Ok(LogRevealOutcome::Unsupported),
        };
        if !path.is_dir() {
            return Ok(LogRevealOutcome::Missing);
        }
        self.app
            .opener()
            .open_path(path.to_string_lossy().into_owned(), None::<String>)
            .map_err(|_| LogRevealError::Failed)?;
        Ok(LogRevealOutcome::Revealed)
    }
}

impl<R: Runtime> DesktopLogReveal<R> {
    fn log_directory_exists(&self) -> Result<bool, tauri::Error> {
        Ok(self.app.path().app_log_dir()?.is_dir())
    }
}
