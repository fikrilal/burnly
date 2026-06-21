use std::fs;

use tauri::{AppHandle, Runtime};
use tauri_plugin_dialog::DialogExt;

use crate::application::ports::export_writer::{
    ExportWriteOutcome, ExportWriter, ExportWriterError,
};

pub(crate) struct DesktopExportWriter<R: Runtime> {
    app: AppHandle<R>,
}

impl<R: Runtime> DesktopExportWriter<R> {
    pub(crate) fn new(app: AppHandle<R>) -> Self {
        Self { app }
    }
}

impl<R: Runtime> ExportWriter for DesktopExportWriter<R> {
    fn write_csv(
        &self,
        suggested_name: &str,
        contents: &[u8],
    ) -> Result<ExportWriteOutcome, ExportWriterError> {
        let selected = self
            .app
            .dialog()
            .file()
            .add_filter("CSV", &["csv"])
            .set_file_name(suggested_name)
            .blocking_save_file();
        let Some(selected) = selected else {
            return Ok(ExportWriteOutcome::Cancelled);
        };
        let path = selected
            .into_path()
            .map_err(|_| ExportWriterError::DestinationUnavailable)?;
        fs::write(path, contents).map_err(|_| ExportWriterError::WriteFailed)?;
        Ok(ExportWriteOutcome::Written)
    }
}
