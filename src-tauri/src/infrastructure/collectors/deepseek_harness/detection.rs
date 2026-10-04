//! DeepSeek Harness session-root inspection and detection.
//!
//! Detection is read-only and metadata-only. It resolves the harness home,
//! delegates canonical generation selection to `discovery`, and publishes
//! counts for the adapter. It never opens or decompresses a session log.

use std::fs;
use std::path::{Path, PathBuf};

use super::deepseek_home::{resolve_deepseek_harness_home, sessions_root};
use super::discovery::inspect_session_directories;

/// Snapshot of a DeepSeek Harness data root used by detection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DeepSeekHarnessHomeInspection {
    pub(crate) home: PathBuf,
    pub(crate) home_exists: bool,
    pub(crate) sessions_root_exists: bool,
    pub(crate) sessions_root_readable: bool,
    /// Number of session directories whose highest canonical generation is 4.
    pub(crate) current_format_session_logs: u32,
    /// Number of session directories whose highest canonical generation is > 4.
    pub(crate) newer_format_session_logs: u32,
    /// Number of session directories whose highest canonical generation is < 4.
    pub(crate) older_format_session_logs: u32,
    /// Number of session directories inspected.
    pub(crate) session_directories: u32,
    /// Number of session directories whose contents could not be enumerated.
    pub(crate) unreadable_session_directories: u32,
}

impl DeepSeekHarnessHomeInspection {
    pub(crate) const fn has_supported_session_logs(&self) -> bool {
        self.current_format_session_logs > 0
    }
}

pub(crate) fn inspect_deepseek_harness_home(
    override_path: Option<&Path>,
) -> DeepSeekHarnessHomeInspection {
    let home = resolve_deepseek_harness_home(override_path);
    let home_exists = home.is_dir();
    let sessions = sessions_root(&home);
    let sessions_root_exists = sessions.is_dir();
    let sessions_root_readable = fs::read_dir(&sessions).is_ok();
    let discovered = inspect_session_directories(&sessions).unwrap_or_default();

    DeepSeekHarnessHomeInspection {
        home,
        home_exists,
        sessions_root_exists,
        sessions_root_readable,
        current_format_session_logs: discovered.current_format_directories,
        newer_format_session_logs: discovered.newer_format_directories,
        older_format_session_logs: discovered.older_format_directories,
        session_directories: discovered.session_directories,
        unreadable_session_directories: discovered.unreadable_session_directories,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    fn write_session_log(sessions: &Path, project: &str, session: &str, filename: &str) {
        let session_dir = sessions.join(project).join(session);
        fs::create_dir_all(&session_dir).expect("session dir");
        fs::write(session_dir.join(filename), b"{}").expect("session log");
    }

    #[test]
    fn counts_highest_generation_per_session_directory() {
        let temp = TempDir::new().expect("temp dir");
        let sessions = temp.path().join("sessions");
        write_session_log(
            &sessions,
            "--project--",
            "session-a",
            "session.v3.jsonl.zstd",
        );
        write_session_log(
            &sessions,
            "--project--",
            "session-a",
            "session.v4.jsonl.zstd",
        );
        write_session_log(
            &sessions,
            "--project--",
            "session-b",
            "session.v5.jsonl.zstd",
        );
        write_session_log(
            &sessions,
            "--project--",
            "session-c",
            "session.v2.jsonl.zstd",
        );
        write_session_log(&sessions, "--project--", "session-d", "session.jsonl");

        let inspection = inspect_deepseek_harness_home(Some(temp.path()));

        assert!(inspection.home_exists);
        assert!(inspection.sessions_root_exists);
        assert!(inspection.sessions_root_readable);
        assert_eq!(inspection.session_directories, 4);
        assert_eq!(inspection.current_format_session_logs, 1);
        assert_eq!(inspection.newer_format_session_logs, 1);
        assert_eq!(inspection.older_format_session_logs, 2);
        assert!(inspection.has_supported_session_logs());
    }

    #[test]
    fn reports_missing_home_and_sessions_root() {
        let temp = TempDir::new().expect("temp dir");
        let missing = temp.path().join("missing-home");

        let inspection = inspect_deepseek_harness_home(Some(&missing));

        assert!(!inspection.home_exists);
        assert!(!inspection.sessions_root_exists);
        assert!(!inspection.sessions_root_readable);
        assert_eq!(inspection.session_directories, 0);
        assert_eq!(inspection.current_format_session_logs, 0);
        assert!(!inspection.has_supported_session_logs());
    }

    #[test]
    fn ignores_non_canonical_and_non_file_entries() {
        let temp = TempDir::new().expect("temp dir");
        let sessions = temp.path().join("sessions");
        write_session_log(
            &sessions,
            "--project--",
            "session-a",
            "session.v4.jsonl.zstd",
        );
        let session_dir = sessions.join("--project--").join("session-a");
        fs::write(session_dir.join("session.v4.jsonl.bak"), b"{}").expect("backup");
        fs::create_dir_all(session_dir.join("nested")).expect("nested dir");

        let inspection = inspect_deepseek_harness_home(Some(temp.path()));

        assert_eq!(inspection.session_directories, 1);
        assert_eq!(inspection.current_format_session_logs, 1);
        assert_eq!(inspection.newer_format_session_logs, 0);
        assert_eq!(inspection.older_format_session_logs, 0);
    }
}
