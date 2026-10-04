//! Canonical DeepSeek Harness session-log discovery.
//!
//! Discovery is read-only and metadata-only: it enumerates expected session
//! directories, parses generation-addressed filenames, and selects the
//! numerically highest generation per session directory. It never opens or
//! decompresses a session log.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Current logical session format generation supported by Burnly.
pub(crate) const CURRENT_SESSION_FORMAT_VERSION: u32 = 4;

/// Physical encoding of one canonical session-log generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SessionLogCompression {
    Plain,
    Zstd,
}

impl SessionLogCompression {
    /// Deterministic preference when a mixed root accidentally contains both
    /// encodings for the same generation. The DSH backend does not publish
    /// mixed roots, so this is only a tie-breaker for damaged input.
    const fn rank(self) -> u8 {
        match self {
            Self::Zstd => 1,
            Self::Plain => 0,
        }
    }
}

/// One discovered session log selected from a session-owned directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionLogFile {
    pub(crate) path: PathBuf,
    pub(crate) format_version: u32,
    pub(crate) compression: SessionLogCompression,
}

/// Result of walking a DeepSeek Harness sessions root.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct SessionDirectoryInspection {
    #[allow(
        dead_code,
        reason = "discovered files are consumed by later collector chunks"
    )]
    pub(crate) files: Vec<SessionLogFile>,
    pub(crate) session_directories: u32,
    pub(crate) current_format_directories: u32,
    pub(crate) newer_format_directories: u32,
    pub(crate) older_format_directories: u32,
    pub(crate) unreadable_session_directories: u32,
}

/// Walk `sessions/<project>/<session>/` and select one log per session.
pub(crate) fn inspect_session_directories(
    sessions_root: &Path,
) -> io::Result<SessionDirectoryInspection> {
    let mut inspection = SessionDirectoryInspection::default();
    let project_entries = fs::read_dir(sessions_root)?;

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
            inspection.session_directories = inspection.session_directories.saturating_add(1);
            match canonical_session_file(&session_entry.path()) {
                Ok(Some(file)) => {
                    inspect_selected_file(&mut inspection, &file);
                    inspection.files.push(file);
                }
                Ok(None) => {}
                Err(_) => {
                    inspection.unreadable_session_directories =
                        inspection.unreadable_session_directories.saturating_add(1);
                }
            }
        }
    }

    Ok(inspection)
}

fn inspect_selected_file(inspection: &mut SessionDirectoryInspection, file: &SessionLogFile) {
    if file.format_version == CURRENT_SESSION_FORMAT_VERSION {
        inspection.current_format_directories =
            inspection.current_format_directories.saturating_add(1);
    } else if file.format_version > CURRENT_SESSION_FORMAT_VERSION {
        inspection.newer_format_directories = inspection.newer_format_directories.saturating_add(1);
    } else {
        inspection.older_format_directories = inspection.older_format_directories.saturating_add(1);
    }
}

/// Select the highest canonical generation in one session directory.
pub(crate) fn canonical_session_file(
    session_directory: &Path,
) -> io::Result<Option<SessionLogFile>> {
    let entries = fs::read_dir(session_directory)?;
    let mut selected: Option<SessionLogFile> = None;

    for entry in entries.flatten() {
        if !entry
            .file_type()
            .map(|file_type| file_type.is_file())
            .unwrap_or(false)
        {
            continue;
        }
        let Some((format_version, compression)) =
            parse_session_log_filename(&entry.file_name().to_string_lossy())
        else {
            continue;
        };
        let candidate = SessionLogFile {
            path: entry.path(),
            format_version,
            compression,
        };
        if selected
            .as_ref()
            .is_none_or(|current| is_preferred(&candidate, current))
        {
            selected = Some(candidate);
        }
    }

    Ok(selected)
}

fn is_preferred(candidate: &SessionLogFile, current: &SessionLogFile) -> bool {
    candidate.format_version > current.format_version
        || (candidate.format_version == current.format_version
            && candidate.compression.rank() > current.compression.rank())
}

/// Parse a canonical DeepSeek Harness session-log basename.
///
/// Canonical generations are `session.jsonl` for version zero and
/// `session.vN.jsonl` for version `N >= 1`, each optionally carrying the
/// `.zstd` compression suffix. Leading zeros, uppercase `V`, and unrelated
/// suffixes are rejected.
pub(crate) fn parse_session_log_filename(filename: &str) -> Option<(u32, SessionLogCompression)> {
    let (uncompressed, compression) = match filename.strip_suffix(".zstd") {
        Some(uncompressed) => (uncompressed, SessionLogCompression::Zstd),
        None => (filename, SessionLogCompression::Plain),
    };

    if uncompressed == "session.jsonl" {
        return Some((0, compression));
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

    digits
        .parse::<u32>()
        .ok()
        .map(|version| (version, compression))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn parses_canonical_session_log_filenames() {
        assert_eq!(
            parse_session_log_filename("session.jsonl"),
            Some((0, SessionLogCompression::Plain))
        );
        assert_eq!(
            parse_session_log_filename("session.jsonl.zstd"),
            Some((0, SessionLogCompression::Zstd))
        );
        assert_eq!(
            parse_session_log_filename("session.v1.jsonl"),
            Some((1, SessionLogCompression::Plain))
        );
        assert_eq!(
            parse_session_log_filename("session.v4.jsonl.zstd"),
            Some((4, SessionLogCompression::Zstd))
        );
        assert_eq!(
            parse_session_log_filename("session.v12.jsonl"),
            Some((12, SessionLogCompression::Plain))
        );
        assert_eq!(parse_session_log_filename("session.v0.jsonl"), None);
        assert_eq!(parse_session_log_filename("session.v01.jsonl"), None);
        assert_eq!(parse_session_log_filename("session.V4.jsonl"), None);
        assert_eq!(parse_session_log_filename("session.jsonl.gz"), None);
        assert_eq!(parse_session_log_filename("other.v4.jsonl"), None);
    }

    #[test]
    fn selects_highest_generation_per_session_directory() {
        let temp = TempDir::new().expect("temp dir");
        let session_dir = temp.path().join("session-a");
        fs::create_dir_all(&session_dir).expect("session dir");
        fs::write(session_dir.join("session.v3.jsonl.zstd"), b"{}").expect("v3");
        fs::write(session_dir.join("session.v4.jsonl"), b"{}").expect("v4");
        fs::write(session_dir.join("session.v5.jsonl.zstd"), b"{}").expect("v5");

        let selected = canonical_session_file(&session_dir)
            .expect("select")
            .expect("selected file");

        assert_eq!(selected.format_version, 5);
        assert_eq!(selected.compression, SessionLogCompression::Zstd);
        assert_eq!(selected.path, session_dir.join("session.v5.jsonl.zstd"));
    }

    #[test]
    fn prefers_zstd_when_the_same_generation_has_both_encodings() {
        let temp = TempDir::new().expect("temp dir");
        let session_dir = temp.path().join("session-a");
        fs::create_dir_all(&session_dir).expect("session dir");
        fs::write(session_dir.join("session.v4.jsonl"), b"{}").expect("plain");
        fs::write(session_dir.join("session.v4.jsonl.zstd"), b"{}").expect("zstd");

        let selected = canonical_session_file(&session_dir)
            .expect("select")
            .expect("selected file");

        assert_eq!(selected.format_version, 4);
        assert_eq!(selected.compression, SessionLogCompression::Zstd);
    }

    #[test]
    fn ignores_non_canonical_and_non_file_entries() {
        let temp = TempDir::new().expect("temp dir");
        let session_dir = temp.path().join("session-a");
        fs::create_dir_all(&session_dir).expect("session dir");
        fs::write(session_dir.join("session.v4.jsonl.zstd"), b"{}").expect("valid");
        fs::write(session_dir.join("session.v4.jsonl.bak"), b"{}").expect("backup");
        fs::create_dir_all(session_dir.join("nested")).expect("nested dir");

        let selected = canonical_session_file(&session_dir)
            .expect("select")
            .expect("selected file");

        assert_eq!(selected.format_version, 4);
        assert_eq!(selected.path, session_dir.join("session.v4.jsonl.zstd"));
    }

    #[test]
    fn counts_versions_across_session_directories() {
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

        let inspection = inspect_session_directories(&sessions).expect("inspect");

        assert_eq!(inspection.session_directories, 4);
        assert_eq!(inspection.current_format_directories, 1);
        assert_eq!(inspection.newer_format_directories, 1);
        assert_eq!(inspection.older_format_directories, 2);
        assert_eq!(inspection.files.len(), 4);
    }

    fn write_session_log(sessions: &Path, project: &str, session: &str, filename: &str) {
        let session_dir = sessions.join(project).join(session);
        fs::create_dir_all(&session_dir).expect("session dir");
        fs::write(session_dir.join(filename), b"{}").expect("session log");
    }
}
