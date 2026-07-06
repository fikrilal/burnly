use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::Path;

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

use chrono::{DateTime, Utc};
use serde::Deserialize;
use thiserror::Error;

pub(crate) const INFERENCE_DONE_MESSAGE: &str = "shell.turn.inference_done";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct UnifiedLogFileMetadata {
    pub(crate) file_inode: Option<u64>,
    pub(crate) file_size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GrokInferenceUsage {
    pub(crate) session_id: String,
    pub(crate) observed_at: DateTime<Utc>,
    pub(crate) pid: u64,
    pub(crate) loop_index: u32,
    pub(crate) prompt_tokens: u64,
    pub(crate) cached_prompt_tokens: u64,
    pub(crate) completion_tokens: u64,
    pub(crate) reasoning_tokens: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct UnifiedLogReadSummary {
    pub(crate) lines_read: u32,
    pub(crate) inference_rows_accepted: u32,
    pub(crate) lines_skipped: u32,
}

#[derive(Debug, Error)]
pub(crate) enum UnifiedLogReadError {
    #[error("grok unified log could not be read")]
    Read(#[source] std::io::Error),
}

#[derive(Debug)]
pub(crate) struct UnifiedLogReader;

impl UnifiedLogReader {
    pub(crate) fn read_file_metadata(
        path: &Path,
    ) -> Result<UnifiedLogFileMetadata, UnifiedLogReadError> {
        let metadata = fs::metadata(path).map_err(UnifiedLogReadError::Read)?;
        Ok(UnifiedLogFileMetadata {
            file_inode: file_inode(&metadata),
            file_size: metadata.len(),
        })
    }

    pub(crate) fn read_from_path(
        path: &Path,
    ) -> Result<(Vec<GrokInferenceUsage>, UnifiedLogReadSummary), UnifiedLogReadError> {
        let file = File::open(path).map_err(UnifiedLogReadError::Read)?;
        let reader = BufReader::new(file);
        let mut rows = Vec::new();
        let mut summary = UnifiedLogReadSummary::default();

        for line in reader.lines() {
            let line = line.map_err(UnifiedLogReadError::Read)?;
            summary.lines_read += 1;
            match parse_inference_line(&line) {
                ParseOutcome::Accepted(row) => {
                    summary.inference_rows_accepted += 1;
                    rows.push(row);
                }
                ParseOutcome::Skipped => summary.lines_skipped += 1,
            }
        }

        Ok((rows, summary))
    }
}

enum ParseOutcome {
    Accepted(GrokInferenceUsage),
    Skipped,
}

fn parse_inference_line(line: &str) -> ParseOutcome {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return ParseOutcome::Skipped;
    }

    let envelope = match serde_json::from_str::<UnifiedLogEnvelope>(trimmed) {
        Ok(value) => value,
        Err(_) => return ParseOutcome::Skipped,
    };
    if envelope.msg != INFERENCE_DONE_MESSAGE {
        return ParseOutcome::Skipped;
    }

    let Some(session_id) = envelope.sid.filter(|value| !value.trim().is_empty()) else {
        return ParseOutcome::Skipped;
    };
    let Some(ctx) = envelope.ctx else {
        return ParseOutcome::Skipped;
    };

    match GrokInferenceUsage::try_from_parts(&session_id, &envelope.ts, envelope.pid, ctx) {
        Ok(row) => ParseOutcome::Accepted(row),
        Err(_) => ParseOutcome::Skipped,
    }
}

impl GrokInferenceUsage {
    pub(crate) fn dedupe_key(&self) -> String {
        format!(
            "{}:{}:{}:{}:{}:{}",
            self.session_id,
            self.observed_at.timestamp_millis(),
            self.loop_index,
            self.prompt_tokens,
            self.completion_tokens,
            self.pid
        )
    }

    fn try_from_parts(
        session_id: &str,
        observed_at: &str,
        pid: u64,
        ctx: InferenceContext,
    ) -> Result<Self, InferenceRowError> {
        let observed_at = DateTime::parse_from_rfc3339(observed_at)
            .map_err(|_| InferenceRowError::InvalidTimestamp)?
            .with_timezone(&Utc);
        let loop_index = u32::try_from(ctx.loop_index).map_err(|_| InferenceRowError::LoopIndex)?;
        let prompt_tokens = non_negative_u64(ctx.prompt_tokens)?;
        let cached_prompt_tokens = non_negative_u64(ctx.cached_prompt_tokens)?;
        let completion_tokens = non_negative_u64(ctx.completion_tokens)?;
        let reasoning_tokens = non_negative_u64(ctx.reasoning_tokens)?;

        if cached_prompt_tokens > prompt_tokens {
            return Err(InferenceRowError::CachedExceedsPrompt);
        }

        Ok(Self {
            session_id: session_id.to_owned(),
            observed_at,
            pid,
            loop_index,
            prompt_tokens,
            cached_prompt_tokens,
            completion_tokens,
            reasoning_tokens,
        })
    }
}

fn non_negative_u64(value: i64) -> Result<u64, InferenceRowError> {
    u64::try_from(value).map_err(|_| InferenceRowError::NegativeTokenCount)
}

#[derive(Debug)]
enum InferenceRowError {
    InvalidTimestamp,
    LoopIndex,
    NegativeTokenCount,
    CachedExceedsPrompt,
}

#[derive(Debug, Deserialize)]
struct UnifiedLogEnvelope {
    ts: String,
    pid: u64,
    sid: Option<String>,
    msg: String,
    ctx: Option<InferenceContext>,
}

fn file_inode(metadata: &fs::Metadata) -> Option<u64> {
    #[cfg(unix)]
    {
        Some(metadata.ino())
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        None
    }
}

#[derive(Debug, Deserialize)]
struct InferenceContext {
    loop_index: u64,
    prompt_tokens: i64,
    cached_prompt_tokens: i64,
    completion_tokens: i64,
    reasoning_tokens: i64,
}
