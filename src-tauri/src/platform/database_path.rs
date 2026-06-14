use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager, Runtime};
use thiserror::Error;

const DATABASE_FILE_NAME: &str = "burnly.sqlite3";

#[derive(Debug, Error)]
pub enum DatabasePathError {
    #[error("application data directory is unavailable")]
    AppDataUnavailable(#[source] tauri::Error),
}

pub fn resolve<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, DatabasePathError> {
    app.path()
        .app_data_dir()
        .map(|directory| from_app_data_directory(&directory))
        .map_err(DatabasePathError::AppDataUnavailable)
}

pub fn from_app_data_directory(directory: &Path) -> PathBuf {
    directory.join(DATABASE_FILE_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_stable_database_file_name() {
        let app_data_directory = Path::new("root").join("app-data");

        let database_path = from_app_data_directory(&app_data_directory);

        assert_eq!(database_path, app_data_directory.join("burnly.sqlite3"));
    }
}
