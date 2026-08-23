use std::path::Path;

use chrono::{DateTime, Utc};
use rusqlite::Connection;
use thiserror::Error;

use crate::infrastructure::collectors::support::open_external_read_only;

use super::mapper::ConversationUsage;
use super::product_variant::AntigravityProductVariant;
use super::protobuf_usage::{
    parse_gen_metadata_rows, parse_trajectory_created_ms, GenMetadataRow, ProtobufUsageError,
};
use super::ConversationDatabase;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct CliSqliteCollectionReport {
    pub(crate) records_extracted: u32,
    pub(crate) records_rejected: u32,
    pub(crate) conversations_parsed: u32,
    pub(crate) conversations_failed: u32,
}

#[derive(Debug, Error)]
pub(crate) enum CliSqliteReaderError {
    #[error("antigravity cli sqlite database could not be opened")]
    Open(#[source] rusqlite::Error),
    #[error("antigravity cli sqlite query failed")]
    Query(#[source] rusqlite::Error),
    #[error("antigravity cli sqlite usage parse failed")]
    Parse(#[source] ProtobufUsageError),
}

pub(crate) fn collect_cli_sqlite_usage(
    conversations: &[ConversationDatabase],
) -> Result<(Vec<ConversationUsage>, CliSqliteCollectionReport), CliSqliteReaderError> {
    let mut usage = Vec::new();
    let mut report = CliSqliteCollectionReport::default();

    for conversation in conversations
        .iter()
        .filter(|conversation| conversation.variant == AntigravityProductVariant::Cli)
    {
        match read_cli_conversation(conversation) {
            Ok(records) => {
                report.conversations_parsed = report.conversations_parsed.saturating_add(1);
                report.records_extracted = report
                    .records_extracted
                    .saturating_add(records.len().try_into().unwrap_or(u32::MAX));
                if !records.is_empty() {
                    usage.push(ConversationUsage {
                        database: conversation.clone(),
                        records,
                    });
                }
            }
            Err(CliSqliteReaderError::Parse(_)) => {
                report.conversations_failed = report.conversations_failed.saturating_add(1);
                report.records_rejected = report.records_rejected.saturating_add(1);
            }
            Err(CliSqliteReaderError::Open(_) | CliSqliteReaderError::Query(_)) => {
                report.conversations_failed = report.conversations_failed.saturating_add(1);
            }
        }
    }

    Ok((usage, report))
}

fn read_cli_conversation(
    conversation: &ConversationDatabase,
) -> Result<Vec<super::AntigravityUsageRecord>, CliSqliteReaderError> {
    read_conversation_gen_metadata_usage(conversation).map_err(Into::into)
}

pub(crate) fn read_conversation_gen_metadata_usage(
    conversation: &ConversationDatabase,
) -> Result<Vec<super::AntigravityUsageRecord>, ConversationSqliteReaderError> {
    let connection =
        open_external_read_only(&conversation.path).map_err(ConversationSqliteReaderError::Open)?;
    let session_timestamp_ms = read_session_timestamp_ms(&connection, &conversation.path)?;
    let rows = read_gen_metadata_rows(&connection)?;
    parse_gen_metadata_rows(
        conversation.variant,
        &conversation.conversation_id,
        &rows,
        session_timestamp_ms,
    )
    .map_err(ConversationSqliteReaderError::Parse)
}

#[derive(Debug, Error)]
pub(crate) enum ConversationSqliteReaderError {
    #[error("antigravity conversation sqlite database could not be opened")]
    Open(#[source] rusqlite::Error),
    #[error("antigravity conversation sqlite query failed")]
    Query(#[source] rusqlite::Error),
    #[error("antigravity conversation sqlite usage parse failed")]
    Parse(#[source] ProtobufUsageError),
}

impl From<ConversationSqliteReaderError> for CliSqliteReaderError {
    fn from(error: ConversationSqliteReaderError) -> Self {
        match error {
            ConversationSqliteReaderError::Open(source) => Self::Open(source),
            ConversationSqliteReaderError::Query(source) => Self::Query(source),
            ConversationSqliteReaderError::Parse(source) => Self::Parse(source),
        }
    }
}

fn read_session_timestamp_ms(
    connection: &Connection,
    path: &Path,
) -> Result<i64, ConversationSqliteReaderError> {
    let blob: Option<Vec<u8>> = connection
        .query_row(
            "SELECT data FROM trajectory_metadata_blob LIMIT 1",
            [],
            |row| row.get(0),
        )
        .ok();

    if let Some(blob) = blob {
        if let Some(timestamp) = parse_trajectory_created_ms(&blob) {
            return Ok(timestamp);
        }
    }

    Ok(file_modified_ms(path).unwrap_or_else(|| Utc::now().timestamp_millis()))
}

pub(crate) fn read_gen_metadata_rows(
    connection: &Connection,
) -> Result<Vec<GenMetadataRow>, ConversationSqliteReaderError> {
    let mut statement = connection
        .prepare("SELECT idx, data FROM gen_metadata ORDER BY idx")
        .map_err(ConversationSqliteReaderError::Query)?;
    let rows = statement
        .query_map([], |row| {
            Ok(GenMetadataRow {
                index: row.get(0)?,
                data: row.get(1)?,
            })
        })
        .map_err(ConversationSqliteReaderError::Query)?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(ConversationSqliteReaderError::Query)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GenMetadataSchemaValidation {
    Valid,
    Missing,
    Mismatch,
}

pub(crate) fn validate_gen_metadata_schema(connection: &Connection) -> GenMetadataSchemaValidation {
    let has_table = connection
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'gen_metadata' LIMIT 1",
            [],
            |row| row.get::<_, i32>(0),
        )
        .is_ok();
    if !has_table {
        return GenMetadataSchemaValidation::Missing;
    }

    let Ok(mut statement) = connection.prepare("PRAGMA table_info(gen_metadata)") else {
        return GenMetadataSchemaValidation::Mismatch;
    };
    let Ok(columns) = statement
        .query_map([], |row| row.get::<_, String>(1))
        .and_then(|rows| rows.collect::<Result<Vec<_>, _>>())
    else {
        return GenMetadataSchemaValidation::Mismatch;
    };

    let has_idx = columns.iter().any(|column| column == "idx");
    let has_data = columns.iter().any(|column| column == "data");
    if has_idx && has_data {
        GenMetadataSchemaValidation::Valid
    } else {
        GenMetadataSchemaValidation::Mismatch
    }
}

fn file_modified_ms(path: &Path) -> Option<i64> {
    std::fs::metadata(path)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .map(|time| DateTime::<Utc>::from(time).timestamp_millis())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use chrono::TimeZone;
    use rusqlite::params;
    use rusqlite::Connection;
    use tempfile::TempDir;

    use super::*;
    use crate::infrastructure::collectors::antigravity::protobuf_usage::tests::{
        sample_gen_metadata_blob, sample_trajectory_metadata_blob,
    };

    fn write_cli_database(path: &Path, rows: &[Vec<u8>], trajectory_blob: Option<&[u8]>) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent dir");
        }
        let connection = Connection::open(path).expect("open database");
        connection
            .execute_batch(
                "CREATE TABLE gen_metadata (idx integer, data blob, size integer);
                 CREATE TABLE trajectory_metadata_blob (id text, data blob);",
            )
            .expect("schema");
        for (index, row) in rows.iter().enumerate() {
            connection
                .execute(
                    "INSERT INTO gen_metadata (idx, data, size) VALUES (?1, ?2, 0)",
                    params![i64::try_from(index).expect("index"), row],
                )
                .expect("insert gen_metadata");
        }
        if let Some(blob) = trajectory_blob {
            connection
                .execute(
                    "INSERT INTO trajectory_metadata_blob (id, data) VALUES ('main', ?1)",
                    params![blob],
                )
                .expect("insert trajectory metadata");
        }
    }

    fn conversation(path: &Path) -> ConversationDatabase {
        ConversationDatabase {
            variant: AntigravityProductVariant::Cli,
            conversation_id: path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .expect("stem")
                .to_owned(),
            path: path.to_path_buf(),
            modified_at: Utc.with_ymd_and_hms(2026, 7, 2, 8, 0, 0).unwrap(),
        }
    }

    #[test]
    fn reads_usage_records_from_synthetic_cli_database() {
        let directory = TempDir::new().expect("tempdir");
        let path = directory
            .path()
            .join("antigravity-cli/conversations/session-a.db");
        write_cli_database(
            &path,
            &[sample_gen_metadata_blob("response-1")],
            Some(&sample_trajectory_metadata_blob()),
        );

        let (usage, report) = collect_cli_sqlite_usage(&[conversation(&path)]).expect("usage");

        assert_eq!(report.conversations_parsed, 1);
        assert_eq!(usage.len(), 1);
        assert_eq!(usage[0].records[0].input_tokens, 150);
        assert_eq!(
            usage[0].records[0].response_id.as_deref(),
            Some("response-1")
        );
    }

    #[test]
    fn tolerates_missing_trajectory_metadata_table() {
        let directory = TempDir::new().expect("tempdir");
        let path = directory.path().join("session-b.db");
        {
            let connection = Connection::open(&path).expect("open");
            connection
                .execute_batch("CREATE TABLE gen_metadata (idx integer, data blob, size integer);")
                .expect("schema");
            connection
                .execute(
                    "INSERT INTO gen_metadata (idx, data, size) VALUES (0, ?1, 0)",
                    params![sample_gen_metadata_blob("response-2")],
                )
                .expect("insert");
        }

        let (usage, _) = collect_cli_sqlite_usage(&[conversation(&path)]).expect("usage");
        assert_eq!(usage.len(), 1);
    }

    #[test]
    fn missing_database_fails_soft_per_conversation() {
        let directory = TempDir::new().expect("tempdir");
        let path = directory.path().join("missing.db");
        let conversation = conversation(&path);
        let (_, report) = collect_cli_sqlite_usage(&[conversation]).expect("collection");
        assert_eq!(report.conversations_failed, 1);
        assert_eq!(report.conversations_parsed, 0);
    }
}
