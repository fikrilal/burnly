//! Zed agent thread store.
//!
//! Reads `~/.local/share/zed/threads/threads.db` read-only, decompresses the
//! zstd `data` BLOB, and parses the thread JSON into usage-only structs.
//! Message content is never deserialized.

use std::io::Read;
use std::path::Path;

use chrono::{DateTime, Utc};
use rusqlite::Connection;
use serde::Deserialize;
use thiserror::Error;

use super::super::support::open_external_read_only;

const ZSTD_MAGIC: &[u8] = &[0x28, 0xB5, 0x2F, 0xFD];
/// Upper bound for a decompressed thread payload (10 MB).
const MAX_DECOMPRESSED_BYTES: usize = 10 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ZedThreadUsage {
    pub(crate) thread_id: String,
    pub(crate) title: String,
    pub(crate) model_provider: String,
    pub(crate) model_id: String,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
    pub(crate) tokens: ZedTokenUsage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct ZedTokenUsage {
    pub(crate) input_tokens: u64,
    pub(crate) output_tokens: u64,
    pub(crate) cache_read_tokens: u64,
    pub(crate) cache_creation_tokens: u64,
}

#[derive(Debug, Error)]
pub(crate) enum ZedThreadStoreError {
    #[error("zed threads database could not be opened")]
    Open(#[source] rusqlite::Error),
    #[error("zed threads table is missing")]
    MissingThreadsTable,
    #[error("zed thread row could not be read")]
    Query(#[source] rusqlite::Error),
    #[error("zed thread payload is not zstd")]
    NotZstd,
    #[error("zed thread payload could not be decompressed")]
    Decompress,
    #[error("zed thread payload exceeds the size limit")]
    TooLarge,
    #[error("zed thread JSON is incompatible")]
    Incompatible,
}

#[derive(Debug)]
pub(crate) struct ZedThreadStore {
    connection: Connection,
}

impl ZedThreadStore {
    pub(crate) fn open_read_only(path: impl AsRef<Path>) -> Result<Self, ZedThreadStoreError> {
        let connection = open_external_read_only(path).map_err(ZedThreadStoreError::Open)?;
        verify_threads_table(&connection)?;
        Ok(Self { connection })
    }

    pub(crate) fn read_threads(&self) -> Result<Vec<ZedThreadUsage>, ZedThreadStoreError> {
        let mut statement = self
            .connection
            .prepare("SELECT id, data, data_type FROM threads")
            .map_err(ZedThreadStoreError::Query)?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(ZedThreadStoreError::Query)?;

        let mut threads = Vec::new();
        for row in rows {
            let (thread_id, data, data_type) = row.map_err(ZedThreadStoreError::Query)?;
            if let Ok(usage) = parse_thread_payload(&thread_id, &data, &data_type) {
                threads.push(usage);
            }
            // Unparseable threads are skipped (non-fatal) so one bad row does
            // not fail the whole store read.
        }
        Ok(threads)
    }
}

fn verify_threads_table(connection: &Connection) -> Result<(), ZedThreadStoreError> {
    let has_threads = connection
        .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='threads'")
        .map_err(ZedThreadStoreError::Query)?
        .exists([])
        .map_err(ZedThreadStoreError::Query)?;
    if has_threads {
        Ok(())
    } else {
        Err(ZedThreadStoreError::MissingThreadsTable)
    }
}

fn parse_thread_payload(
    thread_id: &str,
    data: &[u8],
    data_type: &str,
) -> Result<ZedThreadUsage, ZedThreadStoreError> {
    if data_type != "zstd" || !data.starts_with(ZSTD_MAGIC) {
        return Err(ZedThreadStoreError::NotZstd);
    }
    // Bound the decompressed stream while decoding so a decompression bomb
    // cannot exhaust memory before the size check runs.
    let mut json = Vec::new();
    zstd::stream::read::Decoder::new(data)
        .map_err(|_| ZedThreadStoreError::Decompress)?
        .take((MAX_DECOMPRESSED_BYTES + 1) as u64)
        .read_to_end(&mut json)
        .map_err(|_| ZedThreadStoreError::Decompress)?;
    if json.len() > MAX_DECOMPRESSED_BYTES {
        return Err(ZedThreadStoreError::TooLarge);
    }
    let thread: ZedThreadJson =
        serde_json::from_slice(&json).map_err(|_| ZedThreadStoreError::Incompatible)?;

    let tokens = thread.cumulative_token_usage.unwrap_or_default();
    Ok(ZedThreadUsage {
        thread_id: thread_id.to_owned(),
        title: thread.title,
        model_provider: thread
            .model
            .as_ref()
            .map(|m| m.provider.clone())
            .unwrap_or_default(),
        model_id: thread.model.map(|m| m.model).unwrap_or_default(),
        created_at: thread.created_at.unwrap_or(thread.updated_at),
        updated_at: thread.updated_at,
        tokens: ZedTokenUsage {
            input_tokens: tokens.input_tokens,
            output_tokens: tokens.output_tokens,
            cache_read_tokens: tokens.cache_read_input_tokens.unwrap_or(0),
            cache_creation_tokens: tokens.cache_creation_input_tokens.unwrap_or(0),
        },
    })
}

/// Usage-only view of the decompressed Zed thread JSON. `messages` and other
/// content-bearing fields are deliberately absent.
#[derive(Debug, Deserialize)]
struct ZedThreadJson {
    #[serde(default)]
    title: String,
    #[serde(default)]
    updated_at: DateTime<Utc>,
    #[serde(default)]
    created_at: Option<DateTime<Utc>>,
    #[serde(default)]
    cumulative_token_usage: Option<CumulativeTokenUsage>,
    #[serde(default)]
    model: Option<ModelRef>,
}

#[derive(Debug, Default, Deserialize)]
struct CumulativeTokenUsage {
    input_tokens: u64,
    output_tokens: u64,
    #[serde(default)]
    cache_read_input_tokens: Option<u64>,
    #[serde(default)]
    cache_creation_input_tokens: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct ModelRef {
    provider: String,
    model: String,
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    fn zstd_compress(payload: &str) -> Vec<u8> {
        zstd::stream::encode_all(payload.as_bytes(), 3).expect("compress")
    }

    fn write_threads_db(dir: &TempDir, rows: &[(&str, &[u8], &str)]) -> std::path::PathBuf {
        let path = dir.path().join("threads.db");
        let conn = Connection::open(&path).expect("open db");
        conn.execute(
            "CREATE TABLE threads (id TEXT PRIMARY KEY, summary TEXT NOT NULL, updated_at TEXT NOT NULL, data_type TEXT NOT NULL, data BLOB NOT NULL, parent_id TEXT, worktree_branch TEXT, folder_paths TEXT, folder_paths_order TEXT, created_at TEXT)",
            [],
        )
        .expect("create table");
        for (id, data, data_type) in rows {
            conn.execute(
                "INSERT INTO threads (id, summary, updated_at, data_type, data) VALUES (?1, '', '2026-08-09T00:00:00Z', ?3, ?2)",
                rusqlite::params![id, data, data_type],
            )
            .expect("insert");
        }
        path
    }

    const VALID_THREAD: &str = r#"{
        "title": "Exploration",
        "updated_at": "2026-08-09T03:49:28.634198070Z",
        "created_at": "2026-08-09T03:42:58.149142710Z",
        "cumulative_token_usage": {"input_tokens": 138468, "output_tokens": 9644, "cache_read_input_tokens": 1586296},
        "model": {"provider": "zed.dev", "model": "gpt-5.6-luna"},
        "messages": [{"User": {"id": "u1", "content": [{"Text": "secret prompt"}]}}]
    }"#;

    #[test]
    fn opens_readonly_and_rejects_writes() {
        let dir = TempDir::new().expect("dir");
        let path = write_threads_db(&dir, &[("t1", &zstd_compress(VALID_THREAD), "zstd")]);
        let store = ZedThreadStore::open_read_only(&path).expect("open");
        let error = store
            .connection
            .execute("INSERT INTO threads (id, summary, updated_at, data_type, data) VALUES ('x','','','','')", [])
            .expect_err("readonly write rejected");
        assert_eq!(
            error.sqlite_error_code(),
            Some(rusqlite::ErrorCode::ReadOnly)
        );
    }

    #[test]
    fn parses_valid_thread_with_cumulative_usage() {
        let dir = TempDir::new().expect("dir");
        let path = write_threads_db(&dir, &[("t1", &zstd_compress(VALID_THREAD), "zstd")]);
        let store = ZedThreadStore::open_read_only(&path).expect("open");

        let threads = store.read_threads().expect("read");
        assert_eq!(threads.len(), 1);
        let t = &threads[0];
        assert_eq!(t.thread_id, "t1");
        assert_eq!(t.model_provider, "zed.dev");
        assert_eq!(t.model_id, "gpt-5.6-luna");
        assert_eq!(t.tokens.input_tokens, 138468);
        assert_eq!(t.tokens.output_tokens, 9644);
        assert_eq!(t.tokens.cache_read_tokens, 1586296);
        assert_eq!(t.tokens.cache_creation_tokens, 0);
        assert_eq!(
            t.created_at.to_rfc3339(),
            "2026-08-09T03:42:58.149142710+00:00"
        );
    }

    #[test]
    fn missing_cache_fields_default_to_zero() {
        let json = r#"{
            "title": "Gemini",
            "updated_at": "2026-08-09T03:57:23Z",
            "cumulative_token_usage": {"input_tokens": 873218, "output_tokens": 2418},
            "model": {"provider": "zed.dev", "model": "gemini-3.5-flash"}
        }"#;
        let dir = TempDir::new().expect("dir");
        let path = write_threads_db(&dir, &[("t2", &zstd_compress(json), "zstd")]);
        let store = ZedThreadStore::open_read_only(&path).expect("open");

        let threads = store.read_threads().expect("read");
        assert_eq!(threads[0].tokens.input_tokens, 873218);
        assert_eq!(threads[0].tokens.cache_read_tokens, 0);
        assert_eq!(threads[0].tokens.cache_creation_tokens, 0);
    }

    #[test]
    fn non_zstd_payload_is_rejected_and_skipped() {
        let dir = TempDir::new().expect("dir");
        let path = write_threads_db(&dir, &[("t1", b"not-zstd-data", "zstd")]);
        let store = ZedThreadStore::open_read_only(&path).expect("open");

        let threads = store.read_threads().expect("read");
        assert!(threads.is_empty());
    }

    #[test]
    fn decompression_bomb_is_bounded_and_skipped() {
        // A small zstd payload that expands far beyond the limit must be
        // rejected as TooLarge instead of exhausting memory.
        let bomb: Vec<u8> = vec![b'x'; MAX_DECOMPRESSED_BYTES * 4];
        let compressed = zstd::stream::encode_all(bomb.as_slice(), 1).expect("compress");
        let dir = TempDir::new().expect("dir");
        let path = write_threads_db(&dir, &[("t1", &compressed, "zstd")]);
        let store = ZedThreadStore::open_read_only(&path).expect("open");

        let threads = store.read_threads().expect("read");
        assert!(threads.is_empty());
    }

    #[test]
    fn incompatible_data_type_is_skipped() {
        let dir = TempDir::new().expect("dir");
        let path = write_threads_db(&dir, &[("t1", &zstd_compress(VALID_THREAD), "plain")]);
        let store = ZedThreadStore::open_read_only(&path).expect("open");

        let threads = store.read_threads().expect("read");
        assert!(threads.is_empty());
    }

    #[test]
    fn missing_threads_table_is_an_error() {
        let dir = TempDir::new().expect("dir");
        let path = dir.path().join("empty.db");
        let conn = Connection::open(&path).expect("open");
        conn.execute("CREATE TABLE other (id INTEGER)", [])
            .expect("table");
        drop(conn);

        let error = ZedThreadStore::open_read_only(&path).expect_err("missing table");
        assert!(matches!(error, ZedThreadStoreError::MissingThreadsTable));
    }

    #[test]
    fn reads_sanitized_fixture_file() {
        let fixture = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tests/fixtures/collectors/zed/threads/thread-luna.json"
        ));
        let compressed = zstd_compress(fixture);
        let parsed = parse_thread_payload("fixture-id", &compressed, "zstd").expect("parse");
        assert_eq!(parsed.tokens.input_tokens, 138468);
        assert_eq!(parsed.model_id, "gpt-5.6-luna");
    }
}
