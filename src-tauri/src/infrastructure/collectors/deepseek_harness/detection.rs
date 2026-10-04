//! DeepSeek Harness session-root inspection and detection.
//!
//! Detection is read-only and filesystem-based. It identifies canonical
//! generation-addressed session logs without decompressing them or reading any
//! event content. The highest generation in each session directory is the only
//! one considered, matching the harness persistence backend's current-artifact
//! selection rule.
//!
//! Current support is limited to session format 4. Newer formats are reported
//! as unsupported rather than silently falling back to an older generation.

use std::fs;
use std::path::{Path, PathBuf};

use super::deepseek_home::{resolve_deepseek_harness_home, sessions_root};

const CURRENT_SESSION_FORMAT_VERSION: u32 = 4;

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

    let counts = scan_sessions(&sessions, sessions_root_readable);

    DeepSeekHarnessHomeInspection {
        home,
        home_exists,
        sessions_root_exists,
        sessions_root_readable,
        current_format_session_logs: counts.current_format,
        newer_format_session_logs: counts.newer_format,
        older_format_session_logs: counts.older_format,
        session_directories: counts.session_directories,
        unreadable_session_directories: counts.unreadable_session_directories,
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
struct SessionScanCounts {
    current_format: u32,
    newer_format: u32,
    older_format: u32,
    session_directories: u32,
    unreadable_session_directories: u32,
}

fn scan_sessions(sessions: &Path, readable: bool) -> SessionScanCounts {
    if !readable {
        return SessionScanCounts::default();
    }

    let mut counts = SessionScanCounts::default();
    let Ok(project_entries) = fs::read_dir(sessions) else {
        return counts;
    };

    for project_entry in project_entries.flatten() {
        if !project_entry
            .file_type()
            .map(|file_type| file_type.is_dir())
            .unwrap_or(false)
        {
            continue;
        }
        let Ok(session_entries) = fs::read_dir(project_entry.path()) else {
            continue;
        };
        for session_entry in session_entries.flatten() {
            if !session_entry
                .file_type()
                .map(|file_type| file_type.is_dir())
                .unwrap_or(false)
            {
                continue;
            }
            counts.session_directories = counts.session_directories.saturating_add(1);
            match highest_session_format(&session_entry.path()) {
                Ok(Some(version)) => {
                    if version == CURRENT_SESSION_FORMAT_VERSION {
                        counts.current_format = counts.current_format.saturating_add(1);
                    } else if version > CURRENT_SESSION_FORMAT_VERSION {
                        counts.newer_format = counts.newer_format.saturating_add(1);
                    } else {
                        counts.older_format = counts.older_format.saturating_add(1);
                    }
                }
                Ok(None) => {}
                Err(()) => {
                    counts.unreadable_session_directories =
                        counts.unreadable_session_directories.saturating_add(1);
                }
            }
        }
    }

    counts
}

fn highest_session_format(session_dir: &Path) -> Result<Option<u32>, ()> {
    let entries = fs::read_dir(session_dir).map_err(|_| ())?;
    let mut highest = None;

    for entry in entries.flatten() {
        if !entry
            .file_type()
            .map(|file_type| file_type.is_file())
            .unwrap_or(false)
        {
            continue;
        }
        let Some(version) = parse_session_log_filename(&entry.file_name().to_string_lossy()) else {
            continue;
        };
        highest = Some(highest.map_or(version, |current: u32| current.max(version)));
    }

    Ok(highest)
}

/// Parse a canonical DeepSeek Harness session-log basename.
///
/// Canonical generations are `session.jsonl` for version zero and
/// `session.vN.jsonl` for version `N >= 1`, each optionally carrying the
/// `.zstd` compression suffix. Leading zeros, uppercase `V`, and unrelated
/// suffixes are rejected.
pub(crate) fn parse_session_log_filename(filename: &str) -> Option<u32> {
    let uncompressed = filename.strip_suffix(".zstd").unwrap_or(filename);

    if uncompressed == "session.jsonl" {
        return Some(0);
    }

    let digits = uncompressed
        .strip_prefix("session.v")?
        .strip_suffix(".jsonl")?;
    if digits.is_empty()
        || digits.starts_with('0')
        || !digits.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }

    digits.parse::<u32>().ok()
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
    fn parses_canonical_session_log_filenames() {
        assert_eq!(parse_session_log_filename("session.jsonl"), Some(0));
        assert_eq!(parse_session_log_filename("session.jsonl.zstd"), Some(0));
        assert_eq!(parse_session_log_filename("session.v1.jsonl"), Some(1));
        assert_eq!(parse_session_log_filename("session.v4.jsonl.zstd"), Some(4));
        assert_eq!(parse_session_log_filename("session.v12.jsonl"), Some(12));
        assert_eq!(parse_session_log_filename("session.v0.jsonl"), None);
        assert_eq!(parse_session_log_filename("session.v01.jsonl"), None);
        assert_eq!(parse_session_log_filename("session.V4.jsonl"), None);
        assert_eq!(parse_session_log_filename("session.jsonl.gz"), None);
        assert_eq!(parse_session_log_filename("other.v4.jsonl"), None);
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
