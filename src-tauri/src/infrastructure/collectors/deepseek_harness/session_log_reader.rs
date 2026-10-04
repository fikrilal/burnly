//! Bounded DeepSeek Harness session-log decoding.
//!
//! The reader opens one discovered log, decodes concatenated Zstandard frames
//! or plain JSONL, keeps complete decoded data when the final frame is torn by
//! a live write, and validates the format-4 session header. Event parsing is
//! deliberately left to the next collector chunk.

#![allow(
    dead_code,
    reason = "session-log reader is wired into the collector in a later chunk"
)]

use std::fs::File;
use std::io::{Cursor, Read};
use std::path::Path;

use serde::Deserialize;
use thiserror::Error;

use super::discovery::{SessionLogCompression, SessionLogFile, CURRENT_SESSION_FORMAT_VERSION};

/// Upper bound on one compressed session log read from disk.
const MAX_ENCODED_BYTES: usize = 64 * 1024 * 1024;
/// Upper bound on decompressed or plain session-log bytes retained in memory.
const MAX_DECOMPRESSED_BYTES: usize = 128 * 1024 * 1024;

const ZSTD_MAGIC: [u8; 4] = [0x28, 0xB5, 0x2F, 0xFD];

/// Validated DeepSeek Harness session header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionLogHeader {
    pub(crate) version: u32,
    pub(crate) id: String,
    pub(crate) created_at_ms: i64,
    pub(crate) cwd: Option<String>,
    pub(crate) is_seeded: bool,
    pub(crate) origin: Option<String>,
    pub(crate) parent_session: Option<String>,
    pub(crate) delegation_depth: Option<u32>,
    pub(crate) agent_preset: Option<String>,
}

/// One decoded session log, with the validated header and post-header JSONL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DecodedSessionLog {
    pub(crate) file: SessionLogFile,
    pub(crate) header: SessionLogHeader,
    pub(crate) events_jsonl: Vec<u8>,
    pub(crate) truncated_tail: bool,
}

#[derive(Debug, Error)]
pub(crate) enum SessionLogReadError {
    #[error("DeepSeek Harness session log could not be read")]
    Io(#[source] std::io::Error),

    #[error("DeepSeek Harness session log is empty")]
    Empty,

    #[error("DeepSeek Harness session log exceeds the supported size")]
    TooLarge,

    #[error("DeepSeek Harness session log is not a Zstandard stream")]
    NotZstd,

    #[error("DeepSeek Harness session log could not be decompressed")]
    Decode(#[source] std::io::Error),

    #[error("DeepSeek Harness session header is malformed")]
    MalformedHeader,

    #[error(
        "DeepSeek Harness session header version {actual} does not match file generation {expected}"
    )]
    HeaderVersionMismatch { expected: u32, actual: u32 },

    #[error("DeepSeek Harness session format version {0} is unsupported")]
    UnsupportedFormatVersion(u32),
}

/// Read, decode, and validate one discovered session log.
pub(crate) fn read_session_log(
    file: &SessionLogFile,
) -> Result<DecodedSessionLog, SessionLogReadError> {
    let bytes = match file.compression {
        SessionLogCompression::Zstd => read_bounded(&file.path, MAX_ENCODED_BYTES)?,
        SessionLogCompression::Plain => read_bounded(&file.path, MAX_DECOMPRESSED_BYTES)?,
    };
    if bytes.is_empty() {
        return Err(SessionLogReadError::Empty);
    }

    let (decoded, mut truncated_tail) = match file.compression {
        SessionLogCompression::Zstd => decode_zstd(&bytes)?,
        SessionLogCompression::Plain => (bytes, false),
    };
    if decoded.is_empty() {
        return Err(SessionLogReadError::MalformedHeader);
    }
    if matches!(file.compression, SessionLogCompression::Plain) && !decoded.ends_with(b"\n") {
        truncated_tail = true;
    }

    let (header, events_jsonl) = split_header(decoded, file)?;
    Ok(DecodedSessionLog {
        file: file.clone(),
        header,
        events_jsonl,
        truncated_tail,
    })
}

fn read_bounded(path: &Path, limit: usize) -> Result<Vec<u8>, SessionLogReadError> {
    let file = File::open(path).map_err(SessionLogReadError::Io)?;
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(SessionLogReadError::Io)?;
    if bytes.len() > limit {
        return Err(SessionLogReadError::TooLarge);
    }
    Ok(bytes)
}

fn decode_zstd(bytes: &[u8]) -> Result<(Vec<u8>, bool), SessionLogReadError> {
    decode_zstd_with_limit(bytes, MAX_DECOMPRESSED_BYTES)
}

fn decode_zstd_with_limit(
    bytes: &[u8],
    decoded_limit: usize,
) -> Result<(Vec<u8>, bool), SessionLogReadError> {
    if bytes.len() < ZSTD_MAGIC.len() || bytes[..ZSTD_MAGIC.len()] != ZSTD_MAGIC {
        return Err(SessionLogReadError::NotZstd);
    }

    let mut decoder = zstd::stream::read::Decoder::new(Cursor::new(bytes))
        .map_err(SessionLogReadError::Decode)?;
    let mut decoded = Vec::new();
    let mut chunk = [0_u8; 64 * 1024];
    let mut truncated_tail = false;

    loop {
        match decoder.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) => {
                let next_len = decoded
                    .len()
                    .checked_add(read)
                    .ok_or(SessionLogReadError::TooLarge)?;
                if next_len > decoded_limit {
                    return Err(SessionLogReadError::TooLarge);
                }
                decoded.extend_from_slice(&chunk[..read]);
            }
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => {
                // A live writer can leave a torn final frame. Complete frames
                // and complete decoded bytes already produced remain usable;
                // the tail is marked so the collector can surface it.
                truncated_tail = true;
                break;
            }
            Err(error) => return Err(SessionLogReadError::Decode(error)),
        }
    }

    Ok((decoded, truncated_tail))
}

fn split_header(
    mut decoded: Vec<u8>,
    file: &SessionLogFile,
) -> Result<(SessionLogHeader, Vec<u8>), SessionLogReadError> {
    let header_end = decoded.iter().position(|byte| *byte == b'\n');
    let header_line = match header_end {
        Some(index) => &decoded[..index],
        None => decoded.as_slice(),
    };
    if header_line.is_empty() {
        return Err(SessionLogReadError::MalformedHeader);
    }

    let raw: RawSessionHeader =
        serde_json::from_slice(header_line).map_err(|_| SessionLogReadError::MalformedHeader)?;
    let header = raw.into_header(file)?;

    match header_end {
        Some(index) => {
            decoded.drain(..=index);
            Ok((header, decoded))
        }
        None => Ok((header, Vec::new())),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawSessionHeader {
    #[serde(rename = "type")]
    kind: String,
    version: u32,
    id: String,
    created_at: i64,
    #[serde(default)]
    cwd: Option<String>,
    is_seeded: bool,
    #[serde(default)]
    origin: Option<String>,
    #[serde(default)]
    parent_session: Option<String>,
    #[serde(default)]
    delegation_depth: Option<u32>,
    #[serde(default)]
    agent_preset: Option<String>,
}

impl RawSessionHeader {
    fn into_header(self, file: &SessionLogFile) -> Result<SessionLogHeader, SessionLogReadError> {
        if self.kind != "session" || self.id.trim().is_empty() {
            return Err(SessionLogReadError::MalformedHeader);
        }
        if self.version != file.format_version {
            return Err(SessionLogReadError::HeaderVersionMismatch {
                expected: file.format_version,
                actual: self.version,
            });
        }
        if self.version != CURRENT_SESSION_FORMAT_VERSION {
            return Err(SessionLogReadError::UnsupportedFormatVersion(self.version));
        }
        if self.created_at < 0 {
            return Err(SessionLogReadError::MalformedHeader);
        }

        Ok(SessionLogHeader {
            version: self.version,
            id: self.id,
            created_at_ms: self.created_at,
            cwd: self.cwd,
            is_seeded: self.is_seeded,
            origin: self.origin,
            parent_session: self.parent_session,
            delegation_depth: self.delegation_depth,
            agent_preset: self.agent_preset,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use tempfile::TempDir;

    use super::*;

    const VALID_ROOT: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/collectors/deepseek-harness/sessions/valid-root-session.jsonl"
    ));
    const HEADER_ONLY: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/collectors/deepseek-harness/sessions/header-only-session.jsonl"
    ));
    const MALFORMED_HEADER: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/collectors/deepseek-harness/sessions/malformed-header-session.jsonl"
    ));
    const EVENT_ONLY: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/collectors/deepseek-harness/sessions/event-only-session.jsonl"
    ));
    const VERSION_MISMATCH: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/collectors/deepseek-harness/sessions/version-mismatch-session.jsonl"
    ));
    const UNSUPPORTED_V5: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/collectors/deepseek-harness/sessions/unsupported-v5-session.jsonl"
    ));
    const PARTIAL_TRAILING_LINE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/collectors/deepseek-harness/sessions/partial-trailing-line-session.jsonl"
    ));

    fn split_fixture(input: &str) -> (&str, &str) {
        input.split_once('\n').expect("fixture header")
    }

    fn write_file(temp: &TempDir, name: &str, bytes: &[u8]) -> PathBuf {
        let path = temp.path().join(name);
        fs::write(&path, bytes).expect("write session log");
        path
    }

    fn session_file(
        path: PathBuf,
        format_version: u32,
        compression: SessionLogCompression,
    ) -> SessionLogFile {
        SessionLogFile {
            path,
            format_version,
            compression,
        }
    }

    fn zstd_frame(payload: &[u8]) -> Vec<u8> {
        zstd::stream::encode_all(payload, 3).expect("compress frame")
    }

    fn concatenate(parts: &[Vec<u8>]) -> Vec<u8> {
        parts.iter().flatten().copied().collect()
    }

    #[test]
    fn reads_concatenated_compressed_frames() {
        let temp = TempDir::new().expect("temp dir");
        let (header, events) = split_fixture(VALID_ROOT);
        let frames = [
            zstd_frame(format!("{header}\n").as_bytes()),
            zstd_frame(events.as_bytes()),
        ];
        let path = write_file(&temp, "session.v4.jsonl.zstd", &concatenate(&frames));
        let file = session_file(path, 4, SessionLogCompression::Zstd);

        let decoded = read_session_log(&file).expect("decode");

        assert_eq!(
            decoded.header.id,
            "session-00000000-0000-0000-0000-000000000000"
        );
        assert_eq!(decoded.header.created_at_ms, 1_790_000_000_000);
        assert_eq!(decoded.header.cwd.as_deref(), Some("/redacted/project"));
        assert_eq!(decoded.events_jsonl, events.as_bytes());
        assert!(!decoded.truncated_tail);
    }

    #[test]
    fn reads_header_only_compressed_log_as_empty_events() {
        let temp = TempDir::new().expect("temp dir");
        let path = write_file(
            &temp,
            "session.v4.jsonl.zstd",
            &zstd_frame(HEADER_ONLY.as_bytes()),
        );
        let file = session_file(path, 4, SessionLogCompression::Zstd);

        let decoded = read_session_log(&file).expect("decode");

        assert!(decoded.events_jsonl.is_empty());
        assert!(!decoded.truncated_tail);
    }

    #[test]
    fn reads_plain_jsonl_when_compression_is_disabled() {
        let temp = TempDir::new().expect("temp dir");
        let (_, events) = split_fixture(VALID_ROOT);
        let path = write_file(&temp, "session.v4.jsonl", VALID_ROOT.as_bytes());
        let file = session_file(path, 4, SessionLogCompression::Plain);

        let decoded = read_session_log(&file).expect("decode");

        assert_eq!(decoded.events_jsonl, events.as_bytes());
        assert!(!decoded.truncated_tail);
    }

    #[test]
    fn keeps_complete_prefix_when_final_frame_is_torn() {
        let temp = TempDir::new().expect("temp dir");
        let (header, events) = split_fixture(VALID_ROOT);
        let complete = [
            zstd_frame(format!("{header}\n").as_bytes()),
            zstd_frame(events.as_bytes()),
        ];
        let partial = zstd_frame(b"{\"type\":\"partial\"}\n");
        let mut bytes = concatenate(&complete);
        bytes.extend_from_slice(&partial[..partial.len() - 1]);
        let path = write_file(&temp, "session.v4.jsonl.zstd", &bytes);
        let file = session_file(path, 4, SessionLogCompression::Zstd);

        let decoded = read_session_log(&file).expect("torn prefix");

        assert!(decoded.truncated_tail);
        let events_jsonl = String::from_utf8(decoded.events_jsonl).expect("utf8");
        assert!(events_jsonl.contains("\"type\":\"assistant/message\""));
    }

    #[test]
    fn rejects_corrupt_complete_compressed_frame() {
        let temp = TempDir::new().expect("temp dir");
        let mut bytes = zstd_frame(HEADER_ONLY.as_bytes());
        let last = bytes.len() - 1;
        bytes[last] ^= 0xFF;
        let path = write_file(&temp, "session.v4.jsonl.zstd", &bytes);
        let file = session_file(path, 4, SessionLogCompression::Zstd);

        let error = read_session_log(&file).expect_err("corrupt frame");

        assert!(matches!(error, SessionLogReadError::Decode(_)));
    }

    #[test]
    fn rejects_input_above_byte_limit() {
        let temp = TempDir::new().expect("temp dir");
        let path = write_file(&temp, "session.v4.jsonl", b"123456");

        let error = read_bounded(&path, 5).expect_err("limit");

        assert!(matches!(error, SessionLogReadError::TooLarge));
    }

    #[test]
    fn rejects_decompressed_output_above_limit() {
        let frame = zstd_frame(b"0123456789");

        let error = decode_zstd_with_limit(&frame, 4).expect_err("limit");

        assert!(matches!(error, SessionLogReadError::TooLarge));
    }

    #[test]
    fn discovers_and_reads_selected_session_log() {
        let temp = TempDir::new().expect("temp dir");
        let sessions = temp.path().join("sessions");
        let session_dir = sessions.join("--project--").join("session-a");
        fs::create_dir_all(&session_dir).expect("session dir");
        let (header, events) = split_fixture(VALID_ROOT);
        let frames = [
            zstd_frame(format!("{header}\n").as_bytes()),
            zstd_frame(events.as_bytes()),
        ];
        fs::write(
            session_dir.join("session.v4.jsonl.zstd"),
            concatenate(&frames),
        )
        .expect("session log");

        let discovered = super::super::discovery::inspect_session_directories(&sessions)
            .expect("discovery")
            .files
            .into_iter()
            .next()
            .expect("selected session log");
        let decoded = read_session_log(&discovered).expect("decode");

        assert_eq!(discovered.format_version, 4);
        assert_eq!(
            decoded.header.id,
            "session-00000000-0000-0000-0000-000000000000"
        );
        assert_eq!(decoded.events_jsonl, events.as_bytes());
    }

    #[test]
    fn rejects_empty_file() {
        let temp = TempDir::new().expect("temp dir");
        let path = write_file(&temp, "session.v4.jsonl", b"");
        let file = session_file(path, 4, SessionLogCompression::Plain);

        let error = read_session_log(&file).expect_err("empty");

        assert!(matches!(error, SessionLogReadError::Empty));
    }

    #[test]
    fn rejects_non_zstd_bytes_for_compressed_logs() {
        let temp = TempDir::new().expect("temp dir");
        let path = write_file(&temp, "session.v4.jsonl.zstd", b"not a zstd frame");
        let file = session_file(path, 4, SessionLogCompression::Zstd);

        let error = read_session_log(&file).expect_err("not zstd");

        assert!(matches!(error, SessionLogReadError::NotZstd));
    }

    #[test]
    fn rejects_malformed_header() {
        let temp = TempDir::new().expect("temp dir");
        let path = write_file(&temp, "session.v4.jsonl", MALFORMED_HEADER.as_bytes());
        let file = session_file(path, 4, SessionLogCompression::Plain);

        let error = read_session_log(&file).expect_err("malformed");

        assert!(matches!(error, SessionLogReadError::MalformedHeader));
    }

    #[test]
    fn rejects_log_that_starts_with_an_event() {
        let temp = TempDir::new().expect("temp dir");
        let path = write_file(&temp, "session.v4.jsonl", EVENT_ONLY.as_bytes());
        let file = session_file(path, 4, SessionLogCompression::Plain);

        let error = read_session_log(&file).expect_err("event first");

        assert!(matches!(error, SessionLogReadError::MalformedHeader));
    }

    #[test]
    fn rejects_header_version_that_does_not_match_file_generation() {
        let temp = TempDir::new().expect("temp dir");
        let path = write_file(&temp, "session.v4.jsonl", VERSION_MISMATCH.as_bytes());
        let file = session_file(path, 4, SessionLogCompression::Plain);

        let error = read_session_log(&file).expect_err("version mismatch");

        assert!(matches!(
            error,
            SessionLogReadError::HeaderVersionMismatch {
                expected: 4,
                actual: 3
            }
        ));
    }

    #[test]
    fn rejects_unsupported_format_version() {
        let temp = TempDir::new().expect("temp dir");
        let path = write_file(&temp, "session.v5.jsonl", UNSUPPORTED_V5.as_bytes());
        let file = session_file(path, 5, SessionLogCompression::Plain);

        let error = read_session_log(&file).expect_err("unsupported version");

        assert!(matches!(
            error,
            SessionLogReadError::UnsupportedFormatVersion(5)
        ));
    }

    #[test]
    fn marks_plain_log_without_trailing_newline_as_truncated_tail() {
        let temp = TempDir::new().expect("temp dir");
        let path = write_file(&temp, "session.v4.jsonl", PARTIAL_TRAILING_LINE.as_bytes());
        let file = session_file(path, 4, SessionLogCompression::Plain);

        let decoded = read_session_log(&file).expect("partial tail");

        assert!(decoded.truncated_tail);
        let events_jsonl = String::from_utf8(decoded.events_jsonl).expect("utf8");
        assert!(events_jsonl.contains("\"seq\":1"));
    }
}
