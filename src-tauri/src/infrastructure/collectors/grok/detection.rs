use std::path::{Path, PathBuf};

use super::grok_home::{resolve_grok_home, unified_log_path};
use super::session_index::GrokSessionIndex;
use super::unified_log_reader::UnifiedLogReader;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GrokHomeInspection {
    pub(crate) grok_home: PathBuf,
    pub(crate) grok_home_exists: bool,
    pub(crate) unified_log_exists: bool,
    pub(crate) unified_log_has_inference_events: bool,
    pub(crate) session_summaries_found: u32,
    pub(crate) signals_files_found: u32,
}

pub(crate) fn inspect_grok_home(grok_home_override: Option<&Path>) -> GrokHomeInspection {
    let grok_home = resolve_grok_home(grok_home_override);
    let grok_home_exists = grok_home.is_dir();
    let unified_log = unified_log_path(&grok_home);
    let unified_log_exists = unified_log.is_file();
    let unified_log_has_inference_events =
        unified_log_exists && unified_log_contains_inference_events(&unified_log);
    let session_summaries_found = GrokSessionIndex::from_grok_home(&grok_home)
        .scan()
        .map(|summaries| summaries.len() as u32)
        .unwrap_or(0);
    let signals_files_found = count_signals_files(&grok_home);

    GrokHomeInspection {
        grok_home,
        grok_home_exists,
        unified_log_exists,
        unified_log_has_inference_events,
        session_summaries_found,
        signals_files_found,
    }
}

fn unified_log_contains_inference_events(path: &Path) -> bool {
    UnifiedLogReader::read_from_path(path)
        .ok()
        .is_some_and(|(rows, _)| !rows.is_empty())
}

fn count_signals_files(grok_home: &Path) -> u32 {
    let sessions_root = grok_home.join("sessions");
    if !sessions_root.is_dir() {
        return 0;
    }

    let mut count = 0_u32;
    let Ok(cwd_entries) = std::fs::read_dir(&sessions_root) else {
        return 0;
    };
    for cwd_entry in cwd_entries.flatten() {
        if !cwd_entry
            .file_type()
            .map(|file_type| file_type.is_dir())
            .unwrap_or(false)
        {
            continue;
        }
        let Ok(session_entries) = std::fs::read_dir(cwd_entry.path()) else {
            continue;
        };
        for session_entry in session_entries.flatten() {
            if session_entry.path().join("signals.json").is_file() {
                count += 1;
            }
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::super::unified_log_reader::INFERENCE_DONE_MESSAGE;
    use super::*;

    #[test]
    fn inspects_grok_home_from_environment_override() {
        let temp = TempDir::new().expect("temp dir");
        let grok_home = temp.path().join("grok-home");
        fs::create_dir_all(grok_home.join("logs")).expect("logs dir");
        fs::write(
            unified_log_path(&grok_home),
            format!(
                "{{\"ts\":\"2026-07-06T10:00:00Z\",\"pid\":1,\"sid\":\"sid-1\",\"msg\":\"{INFERENCE_DONE_MESSAGE}\",\"ctx\":{{\"loop_index\":1,\"prompt_tokens\":10,\"cached_prompt_tokens\":0,\"completion_tokens\":1,\"reasoning_tokens\":0}}}}"
            ),
        )
        .expect("unified log");

        let previous = std::env::var_os("GROK_HOME");
        std::env::set_var("GROK_HOME", &grok_home);
        let inspection = inspect_grok_home(None);
        restore_env("GROK_HOME", previous);

        assert_eq!(inspection.grok_home, grok_home);
        assert!(inspection.grok_home_exists);
        assert!(inspection.unified_log_exists);
        assert!(inspection.unified_log_has_inference_events);
    }

    fn restore_env(key: &str, value: Option<std::ffi::OsString>) {
        match value {
            Some(value) => std::env::set_var(key, value),
            None => std::env::remove_var(key),
        }
    }
}
