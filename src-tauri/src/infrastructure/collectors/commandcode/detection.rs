//! Command Code data-root inspection and detection.
//!
//! Detection is read-only and filesystem-based. A source is considered
//! available when the projects root contains at least one new-format session
//! transcript (a `type: session` record plus at least one message carrying a
//! `usage` block). Legacy transcripts (flat records without a `type` field)
//! carry no usage data and are reported separately.

use std::fs;
use std::path::{Path, PathBuf};

use super::commandcode_home::{projects_root, resolve_commandcode_home};

/// Snapshot of a Command Code data root used by detection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CommandCodeHomeInspection {
    pub(crate) commandcode_home: PathBuf,
    pub(crate) commandcode_home_exists: bool,
    pub(crate) projects_root_exists: bool,
    pub(crate) projects_root_readable: bool,
    /// Number of new-format session transcripts found under `projects/`.
    pub(crate) new_format_transcripts: u32,
    /// Number of legacy-format transcripts found under `projects/`.
    pub(crate) legacy_transcripts: u32,
    /// True when at least one new-format transcript carries usage.
    pub(crate) has_usage_transcripts: bool,
}

pub(crate) fn inspect_commandcode_home(override_path: Option<&Path>) -> CommandCodeHomeInspection {
    let commandcode_home = resolve_commandcode_home(override_path);
    let commandcode_home_exists = commandcode_home.is_dir();
    let projects = projects_root(&commandcode_home);
    let projects_root_exists = projects.is_dir();
    let projects_root_readable = fs::read_dir(&projects).is_ok();

    let (new_format_transcripts, legacy_transcripts, has_usage_transcripts) =
        scan_projects(&projects, projects_root_readable);

    CommandCodeHomeInspection {
        commandcode_home,
        commandcode_home_exists,
        projects_root_exists,
        projects_root_readable,
        new_format_transcripts,
        legacy_transcripts,
        has_usage_transcripts,
    }
}

fn scan_projects(projects: &Path, readable: bool) -> (u32, u32, bool) {
    if !readable {
        return (0, 0, false);
    }

    let mut new_format = 0_u32;
    let mut legacy = 0_u32;
    let mut has_usage = false;

    let Ok(project_entries) = fs::read_dir(projects) else {
        return (0, 0, false);
    };
    for project_entry in project_entries.flatten() {
        if !project_entry
            .file_type()
            .map(|file_type| file_type.is_dir())
            .unwrap_or(false)
        {
            continue;
        }
        let Ok(transcript_entries) = fs::read_dir(project_entry.path()) else {
            continue;
        };
        for transcript_entry in transcript_entries.flatten() {
            let path = transcript_entry.path();
            if !path.extension().map(|ext| ext == "jsonl").unwrap_or(false) {
                continue;
            }
            if path
                .file_name()
                .map(|name| name.to_string_lossy().contains(".checkpoints."))
                .unwrap_or(false)
            {
                continue;
            }
            match classify_transcript(&path) {
                TranscriptKind::NewFormatWithUsage => {
                    new_format += 1;
                    has_usage = true;
                }
                TranscriptKind::NewFormatNoUsage => new_format += 1,
                TranscriptKind::Legacy => legacy += 1,
                TranscriptKind::UnreadableOrEmpty => {}
            }
        }
    }

    (new_format, legacy, has_usage)
}

enum TranscriptKind {
    NewFormatWithUsage,
    NewFormatNoUsage,
    Legacy,
    UnreadableOrEmpty,
}

fn classify_transcript(path: &Path) -> TranscriptKind {
    let Ok(lines) = read_transcript_lines(path) else {
        return TranscriptKind::UnreadableOrEmpty;
    };

    let mut saw_type_field = false;
    let mut saw_session_record = false;
    let mut saw_usage = false;

    for line in lines {
        // A trailing partial line from a live append is not a failure; treat
        // it as unparseable and skip it.
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        let Some(obj) = value.as_object() else {
            continue;
        };
        if obj.contains_key("type") {
            saw_type_field = true;
        }
        if obj.get("type").and_then(|t| t.as_str()) == Some("session") {
            saw_session_record = true;
        }
        if obj.contains_key("usage") {
            saw_usage = true;
        }
    }

    match (saw_type_field, saw_session_record, saw_usage) {
        (true, true, true) => TranscriptKind::NewFormatWithUsage,
        (true, _, _) => TranscriptKind::NewFormatNoUsage,
        // Flat records without a `type` field are the pre-1.11 legacy schema.
        (false, _, _) => TranscriptKind::Legacy,
    }
}

fn read_transcript_lines(path: &Path) -> std::io::Result<Vec<String>> {
    fs::read_to_string(path)
        .map(|contents| contents.lines().map(str::to_owned).collect::<Vec<String>>())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    const NEW_FORMAT_WITH_USAGE: &str = r#"{"type":"session","version":3,"id":"sess-1","timestamp":"2026-08-04T10:00:00Z","cwd":"/tmp/proj"}
{"type":"message","id":"m1","parentId":null,"timestamp":"2026-08-04T10:00:01Z","message":{"role":"user","content":[{"type":"text","text":"hi"}]}}
{"type":"message","id":"m2","parentId":"m1","timestamp":"2026-08-04T10:00:02Z","message":{"role":"assistant","content":[{"type":"text","text":"hello"}]},"usage":{"inputTokens":10,"outputTokens":2,"cacheReadTokens":3,"cacheWriteTokens":0,"costUsd":0.001},"model":"deepseek/deepseek-v4-flash","effort":"max"}"#;

    const NEW_FORMAT_NO_USAGE: &str = r#"{"type":"session","version":3,"id":"sess-2","timestamp":"2026-08-04T10:00:00Z","cwd":"/tmp/proj"}
{"type":"message","id":"m1","parentId":null,"timestamp":"2026-08-04T10:00:01Z","message":{"role":"user","content":[{"type":"text","text":"hi"}]}}"#;

    const LEGACY_FORMAT: &str = r#"{"id":"legacy-1","timestamp":"2026-05-07T03:23:01Z","sessionId":"sess-legacy","parentId":null,"role":"user","content":[{"type":"text","text":"hi"}]}"#;

    fn write_transcript(project_dir: &Path, name: &str, contents: &str) {
        fs::create_dir_all(project_dir).expect("project dir");
        fs::write(project_dir.join(name), contents).expect("transcript");
    }

    #[test]
    fn inspects_commandcode_home_from_environment_override() {
        let temp = TempDir::new().expect("temp dir");
        let commandcode_home = temp.path().join("commandcode-home");
        fs::create_dir_all(commandcode_home.join("projects").join("proj-a")).expect("projects dir");
        write_transcript(
            &commandcode_home.join("projects").join("proj-a"),
            "sess-1.jsonl",
            NEW_FORMAT_WITH_USAGE,
        );

        let previous = std::env::var_os("COMMANDCODE_HOME");
        std::env::set_var("COMMANDCODE_HOME", &commandcode_home);
        let inspection = inspect_commandcode_home(None);
        restore_env("COMMANDCODE_HOME", previous);

        assert_eq!(inspection.commandcode_home, commandcode_home);
        assert!(inspection.commandcode_home_exists);
        assert!(inspection.projects_root_exists);
        assert!(inspection.projects_root_readable);
        assert_eq!(inspection.new_format_transcripts, 1);
        assert_eq!(inspection.legacy_transcripts, 0);
        assert!(inspection.has_usage_transcripts);
    }

    #[test]
    fn reports_not_found_when_projects_root_is_missing() {
        let temp = TempDir::new().expect("temp dir");
        let commandcode_home = temp.path().join("missing");

        let inspection = inspect_commandcode_home(Some(&commandcode_home));

        assert!(!inspection.commandcode_home_exists);
        assert!(!inspection.projects_root_exists);
        assert_eq!(inspection.new_format_transcripts, 0);
        assert!(!inspection.has_usage_transcripts);
    }

    #[test]
    fn reports_available_no_data_when_only_legacy_transcripts_exist() {
        let temp = TempDir::new().expect("temp dir");
        let commandcode_home = temp.path().join("commandcode-home");
        write_transcript(
            &commandcode_home.join("projects").join("proj-legacy"),
            "sess-legacy.jsonl",
            LEGACY_FORMAT,
        );

        let inspection = inspect_commandcode_home(Some(&commandcode_home));

        assert_eq!(inspection.new_format_transcripts, 0);
        assert_eq!(inspection.legacy_transcripts, 1);
        assert!(!inspection.has_usage_transcripts);
    }

    #[test]
    fn counts_new_format_transcripts_with_and_without_usage() {
        let temp = TempDir::new().expect("temp dir");
        let commandcode_home = temp.path().join("commandcode-home");
        write_transcript(
            &commandcode_home.join("projects").join("proj-a"),
            "sess-1.jsonl",
            NEW_FORMAT_WITH_USAGE,
        );
        write_transcript(
            &commandcode_home.join("projects").join("proj-b"),
            "sess-2.jsonl",
            NEW_FORMAT_NO_USAGE,
        );

        let inspection = inspect_commandcode_home(Some(&commandcode_home));

        assert_eq!(inspection.new_format_transcripts, 2);
        assert!(inspection.has_usage_transcripts);
    }

    #[test]
    fn skips_checkpoint_files_when_scanning() {
        let temp = TempDir::new().expect("temp dir");
        let commandcode_home = temp.path().join("commandcode-home");
        write_transcript(
            &commandcode_home.join("projects").join("proj-a"),
            "sess-1.jsonl",
            NEW_FORMAT_WITH_USAGE,
        );
        write_transcript(
            &commandcode_home.join("projects").join("proj-a"),
            "sess-1.checkpoints.jsonl",
            r#"{"id":"cp-1","messageId":"cp-1","turnNumber":1,"createdAt":"2026-08-04T10:00:00Z","prompt":"secret"}"#,
        );

        let inspection = inspect_commandcode_home(Some(&commandcode_home));

        assert_eq!(inspection.new_format_transcripts, 1);
        assert_eq!(inspection.legacy_transcripts, 0);
    }

    #[test]
    fn tolerates_partial_trailing_line_in_new_format() {
        let temp = TempDir::new().expect("temp dir");
        let commandcode_home = temp.path().join("commandcode-home");
        write_transcript(
            &commandcode_home.join("projects").join("proj-a"),
            "sess-1.jsonl",
            &format!("{NEW_FORMAT_WITH_USAGE}\n{{\"type\":\"message\""),
        );

        let inspection = inspect_commandcode_home(Some(&commandcode_home));

        assert_eq!(inspection.new_format_transcripts, 1);
        assert!(inspection.has_usage_transcripts);
    }

    fn restore_env(key: &str, value: Option<std::ffi::OsString>) {
        match value {
            Some(value) => std::env::set_var(key, value),
            None => std::env::remove_var(key),
        }
    }
}
