use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::Deserialize;
use thiserror::Error;

use super::grok_home::sessions_root;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GrokSessionSummary {
    pub(crate) session_id: String,
    pub(crate) cwd: String,
    pub(crate) current_model_id: Option<String>,
    pub(crate) agent_name: Option<String>,
    pub(crate) git_root_dir: Option<String>,
    pub(crate) head_branch: Option<String>,
    pub(crate) created_at: Option<DateTime<Utc>>,
    pub(crate) updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Error)]
pub(crate) enum SessionIndexError {
    #[error("grok sessions root could not be read")]
    UnreadableRoot(#[source] std::io::Error),
    #[error("grok session summary is incompatible")]
    IncompatibleSummary,
}

#[derive(Debug, Clone)]
pub(crate) struct GrokSessionIndex {
    sessions_root: PathBuf,
}

impl GrokSessionIndex {
    pub(crate) fn from_grok_home(grok_home: &Path) -> Self {
        Self {
            sessions_root: sessions_root(grok_home),
        }
    }

    pub(crate) fn scan(&self) -> Result<Vec<GrokSessionSummary>, SessionIndexError> {
        if !self.sessions_root.is_dir() {
            return Ok(Vec::new());
        }

        let mut summaries = Vec::new();
        let cwd_entries =
            fs::read_dir(&self.sessions_root).map_err(SessionIndexError::UnreadableRoot)?;
        for cwd_entry in cwd_entries {
            let cwd_entry = cwd_entry.map_err(SessionIndexError::UnreadableRoot)?;
            if !cwd_entry
                .file_type()
                .map_err(SessionIndexError::UnreadableRoot)?
                .is_dir()
            {
                continue;
            }

            let session_entries =
                fs::read_dir(cwd_entry.path()).map_err(SessionIndexError::UnreadableRoot)?;
            for session_entry in session_entries {
                let session_entry = session_entry.map_err(SessionIndexError::UnreadableRoot)?;
                let summary_path = session_entry.path().join("summary.json");
                if !summary_path.is_file() {
                    continue;
                }
                let contents =
                    fs::read_to_string(&summary_path).map_err(SessionIndexError::UnreadableRoot)?;
                if let Ok(summary) = parse_summary(&contents) {
                    summaries.push(summary);
                }
            }
        }

        summaries.sort_by(|left, right| {
            left.session_id
                .cmp(&right.session_id)
                .then_with(|| left.cwd.cmp(&right.cwd))
        });
        Ok(summaries)
    }
}

fn parse_summary(contents: &str) -> Result<GrokSessionSummary, SessionIndexError> {
    let raw: RawSummary =
        serde_json::from_str(contents).map_err(|_| SessionIndexError::IncompatibleSummary)?;
    let session_id = raw
        .info
        .as_ref()
        .and_then(|info| info.id.clone())
        .or(raw.id)
        .filter(|value| !value.trim().is_empty())
        .ok_or(SessionIndexError::IncompatibleSummary)?;
    let cwd = raw
        .info
        .and_then(|info| info.cwd)
        .or(raw.cwd)
        .filter(|value| !value.trim().is_empty())
        .ok_or(SessionIndexError::IncompatibleSummary)?;

    Ok(GrokSessionSummary {
        session_id,
        cwd,
        current_model_id: raw.current_model_id,
        agent_name: raw.agent_name,
        git_root_dir: raw.git_root_dir,
        head_branch: raw.head_branch,
        created_at: parse_timestamp(raw.created_at.as_deref())?,
        updated_at: parse_timestamp(raw.updated_at.as_deref())?,
    })
}

fn parse_timestamp(value: Option<&str>) -> Result<Option<DateTime<Utc>>, SessionIndexError> {
    let Some(value) = value else {
        return Ok(None);
    };
    DateTime::parse_from_rfc3339(value)
        .map(|timestamp| Some(timestamp.with_timezone(&Utc)))
        .map_err(|_| SessionIndexError::IncompatibleSummary)
}

#[derive(Debug, Deserialize)]
struct RawSummary {
    info: Option<RawSummaryInfo>,
    id: Option<String>,
    cwd: Option<String>,
    current_model_id: Option<String>,
    agent_name: Option<String>,
    git_root_dir: Option<String>,
    head_branch: Option<String>,
    created_at: Option<String>,
    updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawSummaryInfo {
    id: Option<String>,
    cwd: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_summary_without_session_identity() {
        let error = parse_summary(r#"{"num_messages": 0}"#).expect_err("missing identity");

        assert!(matches!(error, SessionIndexError::IncompatibleSummary));
    }
}
