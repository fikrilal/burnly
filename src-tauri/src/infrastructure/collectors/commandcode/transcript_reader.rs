//! Command Code transcript scanning.
//!
//! Walks `projects/**` for session transcripts, skips checkpoint files and
//! non-JSONL files, and parses each readable transcript. Unreadable or
//! unparseable files are skipped without failing the scan.

use std::fs;
use std::path::{Path, PathBuf};

use super::commandcode_home::projects_root;
use super::transcript_parser::{parse_transcript, ParsedTranscript, TranscriptKind};

/// One transcript file discovered under `projects/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TranscriptFile {
    /// Absolute path to the `.jsonl` transcript.
    pub(crate) path: PathBuf,
}

/// Result of scanning the projects root.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct TranscriptScanSummary {
    pub(crate) transcript_files_found: u32,
    pub(crate) new_format_with_usage: u32,
    pub(crate) new_format_no_usage: u32,
    pub(crate) legacy_transcripts: u32,
    pub(crate) unreadable_transcripts: u32,
    pub(crate) usage_records: u32,
}

pub(crate) struct TranscriptReader;

impl TranscriptReader {
    /// Scan `projects/**` and parse all readable transcripts.
    pub(crate) fn scan(
        commandcode_home: &Path,
    ) -> (
        Vec<TranscriptFile>,
        Vec<ParsedTranscript>,
        TranscriptScanSummary,
    ) {
        let projects = projects_root(commandcode_home);
        let mut files = Vec::new();
        let mut parsed = Vec::new();
        let mut summary = TranscriptScanSummary::default();

        for transcript in discover_transcripts(&projects) {
            summary.transcript_files_found += 1;
            let Ok(contents) = fs::read_to_string(&transcript) else {
                summary.unreadable_transcripts += 1;
                continue;
            };
            let (kind, maybe_parsed, parse_summary) = parse_transcript(&contents);
            summary.usage_records += parse_summary.usage_records;
            match kind {
                TranscriptKind::NewFormatWithUsage => {
                    summary.new_format_with_usage += 1;
                    files.push(TranscriptFile { path: transcript });
                    if let Some(parsed_transcript) = maybe_parsed {
                        parsed.push(parsed_transcript);
                    }
                }
                TranscriptKind::NewFormatNoUsage => summary.new_format_no_usage += 1,
                TranscriptKind::Legacy => summary.legacy_transcripts += 1,
            }
        }

        (files, parsed, summary)
    }
}

/// Discover `.jsonl` transcript paths under `projects/**`, skipping
/// checkpoint files.
fn discover_transcripts(projects: &Path) -> Vec<PathBuf> {
    let mut transcripts = Vec::new();
    let Ok(project_entries) = fs::read_dir(projects) else {
        return transcripts;
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
            transcripts.push(path);
        }
    }
    transcripts
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    const VALID_TRANSCRIPT: &str = r#"{"type":"session","version":3,"id":"sess-1","timestamp":"2026-08-04T10:00:00Z","cwd":"/tmp/proj"}
{"type":"message","id":"m1","parentId":null,"timestamp":"2026-08-04T10:00:02Z","message":{"role":"assistant","content":[{"type":"text","text":"redacted"}]},"usage":{"inputTokens":10,"outputTokens":2,"cacheReadTokens":3,"cacheWriteTokens":0,"costUsd":0.001},"model":"m","effort":"max"}"#;

    const LEGACY_TRANSCRIPT: &str = r#"{"id":"legacy-1","timestamp":"2026-05-07T03:23:01Z","sessionId":"sess-legacy","parentId":null,"role":"user","content":[{"type":"text","text":"redacted"}]}"#;

    fn write_transcript(project_dir: &Path, name: &str, contents: &str) {
        fs::create_dir_all(project_dir).expect("project dir");
        fs::write(project_dir.join(name), contents).expect("transcript");
    }

    #[test]
    fn scans_all_non_checkpoint_transcripts() {
        let temp = TempDir::new().expect("temp dir");
        let home = temp.path().join("home");
        write_transcript(
            &home.join("projects").join("proj-a"),
            "sess-1.jsonl",
            VALID_TRANSCRIPT,
        );
        write_transcript(
            &home.join("projects").join("proj-b"),
            "sess-2.jsonl",
            LEGACY_TRANSCRIPT,
        );
        write_transcript(
            &home.join("projects").join("proj-a"),
            "sess-1.checkpoints.jsonl",
            "{}",
        );

        let (files, parsed, summary) = TranscriptReader::scan(&home);

        assert_eq!(files.len(), 1);
        assert_eq!(parsed.len(), 1);
        assert_eq!(summary.transcript_files_found, 2);
        assert_eq!(summary.new_format_with_usage, 1);
        assert_eq!(summary.legacy_transcripts, 1);
        assert_eq!(summary.usage_records, 1);
    }

    #[test]
    fn ignores_unreadable_and_missing_projects_root() {
        let temp = TempDir::new().expect("temp dir");
        let missing = temp.path().join("missing-home");

        let (files, parsed, summary) = TranscriptReader::scan(&missing);

        assert!(files.is_empty());
        assert!(parsed.is_empty());
        assert_eq!(summary.transcript_files_found, 0);
    }

    #[test]
    fn counts_usage_records_across_multiple_transcripts() {
        let temp = TempDir::new().expect("temp dir");
        let home = temp.path().join("home");
        write_transcript(
            &home.join("projects").join("proj-a"),
            "sess-1.jsonl",
            VALID_TRANSCRIPT,
        );
        write_transcript(
            &home.join("projects").join("proj-a"),
            "sess-2.jsonl",
            &format!("{VALID_TRANSCRIPT}\n{VALID_TRANSCRIPT}"),
        );

        let (_, _, summary) = TranscriptReader::scan(&home);

        assert_eq!(summary.transcript_files_found, 2);
        assert_eq!(summary.new_format_with_usage, 2);
        assert_eq!(summary.usage_records, 3);
    }
}
