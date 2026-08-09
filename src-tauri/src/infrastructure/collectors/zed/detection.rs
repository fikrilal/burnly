//! Zed agent data-root resolution and detection.

use std::path::{Path, PathBuf};

pub(crate) fn default_zed_data_dir() -> PathBuf {
    home_directory()
        .map(|directory| directory.join(".local").join("share").join("zed"))
        .unwrap_or_else(|| PathBuf::from(".local/share/zed"))
}

pub(crate) fn threads_db_path(zed_data_dir: &Path) -> PathBuf {
    zed_data_dir.join("threads").join("threads.db")
}

fn home_directory() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_threads_db_under_zed_data_dir() {
        assert_eq!(
            threads_db_path(Path::new("/tmp/zed")),
            PathBuf::from("/tmp/zed/threads/threads.db")
        );
    }
}
