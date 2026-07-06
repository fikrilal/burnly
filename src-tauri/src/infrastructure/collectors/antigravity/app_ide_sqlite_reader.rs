use std::collections::BTreeSet;

use crate::infrastructure::collectors::support::open_external_read_only;

use super::cli_sqlite_reader::{
    read_conversation_gen_metadata_usage, validate_gen_metadata_schema, ConversationSqliteReaderError,
    GenMetadataSchemaValidation,
};
use super::mapper::ConversationUsage;
use super::product_variant::AntigravityProductVariant;
use super::ConversationDatabase;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct AppIdeSqliteCollectionReport {
    pub(crate) records_extracted: u32,
    pub(crate) records_rejected: u32,
    pub(crate) conversations_accepted: u32,
    pub(crate) conversations_rejected: u32,
    pub(crate) variants_accepted: BTreeSet<AntigravityProductVariant>,
    pub(crate) variants_rejected: BTreeSet<AntigravityProductVariant>,
}

impl AppIdeSqliteCollectionReport {
    fn record_accepted(&mut self, variant: AntigravityProductVariant, records: usize) {
        self.conversations_accepted = self.conversations_accepted.saturating_add(1);
        self.records_extracted = self
            .records_extracted
            .saturating_add(records.try_into().unwrap_or(u32::MAX));
        self.variants_accepted.insert(variant);
    }

    fn record_rejected(&mut self, variant: AntigravityProductVariant, records_rejected: u32) {
        self.conversations_rejected = self.conversations_rejected.saturating_add(1);
        self.records_rejected = self.records_rejected.saturating_add(records_rejected);
        self.variants_rejected.insert(variant);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppIdeSqliteFallbackOutcome {
    RejectedSchema,
    RejectedRecords,
    OpenFailed,
}

pub(crate) fn collect_app_ide_sqlite_fallback(
    conversations: &[ConversationDatabase],
) -> (Vec<ConversationUsage>, AppIdeSqliteCollectionReport) {
    let mut usage = Vec::new();
    let mut report = AppIdeSqliteCollectionReport {
        variants_accepted: BTreeSet::new(),
        variants_rejected: BTreeSet::new(),
        ..AppIdeSqliteCollectionReport::default()
    };

    for conversation in conversations.iter().filter(|conversation| {
        matches!(
            conversation.variant,
            AntigravityProductVariant::App | AntigravityProductVariant::Ide
        )
    }) {
        match read_app_ide_conversation(conversation) {
            Ok(records) => {
                report.record_accepted(conversation.variant, records.len());
                if !records.is_empty() {
                    usage.push(ConversationUsage {
                        database: conversation.clone(),
                        records,
                    });
                }
            }
            Err(AppIdeSqliteFallbackOutcome::RejectedRecords) => {
                report.record_rejected(conversation.variant, 1);
            }
            Err(AppIdeSqliteFallbackOutcome::RejectedSchema) => {
                report.record_rejected(conversation.variant, 0);
            }
            Err(AppIdeSqliteFallbackOutcome::OpenFailed) => {}
        }
    }

    (usage, report)
}

fn read_app_ide_conversation(
    conversation: &ConversationDatabase,
) -> Result<Vec<super::AntigravityUsageRecord>, AppIdeSqliteFallbackOutcome> {
    let connection = match open_external_read_only(&conversation.path) {
        Ok(connection) => connection,
        Err(_) => return Err(AppIdeSqliteFallbackOutcome::OpenFailed),
    };

    match validate_gen_metadata_schema(&connection) {
        GenMetadataSchemaValidation::Valid => {}
        GenMetadataSchemaValidation::Missing => return Err(AppIdeSqliteFallbackOutcome::OpenFailed),
        GenMetadataSchemaValidation::Mismatch => {
            return Err(AppIdeSqliteFallbackOutcome::RejectedSchema);
        }
    }

    let rows = match super::cli_sqlite_reader::read_gen_metadata_rows(&connection) {
        Ok(rows) => rows,
        Err(ConversationSqliteReaderError::Query(_)) => {
            return Err(AppIdeSqliteFallbackOutcome::RejectedSchema);
        }
        Err(ConversationSqliteReaderError::Open(_)) => {
            return Err(AppIdeSqliteFallbackOutcome::OpenFailed);
        }
        Err(ConversationSqliteReaderError::Parse(_)) => {
            return Err(AppIdeSqliteFallbackOutcome::RejectedRecords);
        }
    };

    if rows.is_empty() {
        return Err(AppIdeSqliteFallbackOutcome::RejectedRecords);
    }

    match read_conversation_gen_metadata_usage(conversation) {
        Ok(records) if records.is_empty() => Err(AppIdeSqliteFallbackOutcome::RejectedRecords),
        Ok(records) => Ok(records),
        Err(ConversationSqliteReaderError::Parse(_)) => {
            Err(AppIdeSqliteFallbackOutcome::RejectedRecords)
        }
        Err(ConversationSqliteReaderError::Open(_)) => Err(AppIdeSqliteFallbackOutcome::OpenFailed),
        Err(ConversationSqliteReaderError::Query(_)) => {
            Err(AppIdeSqliteFallbackOutcome::RejectedSchema)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use chrono::TimeZone;
    use chrono::Utc;
    use rusqlite::params;
    use rusqlite::Connection;
    use tempfile::TempDir;

    use super::*;
    use crate::infrastructure::collectors::antigravity::protobuf_usage::tests::{
        sample_gen_metadata_blob, sample_trajectory_metadata_blob,
    };

    fn write_valid_database(path: &std::path::Path, rows: &[Vec<u8>]) {
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
        connection
            .execute(
                "INSERT INTO trajectory_metadata_blob (id, data) VALUES ('main', ?1)",
                params![sample_trajectory_metadata_blob()],
            )
            .expect("insert trajectory metadata");
    }

    fn conversation(
        variant: AntigravityProductVariant,
        path: &std::path::Path,
    ) -> ConversationDatabase {
        ConversationDatabase {
            variant,
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
    fn reads_app_usage_records_from_synthetic_database() {
        let directory = TempDir::new().expect("tempdir");
        let path = directory
            .path()
            .join("antigravity/conversations/app-session.db");
        write_valid_database(&path, &[sample_gen_metadata_blob("response-app")]);

        let (usage, report) = collect_app_ide_sqlite_fallback(&[conversation(
            AntigravityProductVariant::App,
            &path,
        )]);

        assert_eq!(report.conversations_accepted, 1);
        assert!(report.variants_accepted.contains(&AntigravityProductVariant::App));
        assert_eq!(usage.len(), 1);
        assert_eq!(usage[0].records[0].input_tokens, 150);
    }

    #[test]
    fn reads_ide_usage_records_from_synthetic_database() {
        let directory = TempDir::new().expect("tempdir");
        let path = directory
            .path()
            .join("antigravity-ide/conversations/ide-session.db");
        write_valid_database(&path, &[sample_gen_metadata_blob("response-ide")]);

        let (usage, report) = collect_app_ide_sqlite_fallback(&[conversation(
            AntigravityProductVariant::Ide,
            &path,
        )]);

        assert_eq!(report.conversations_accepted, 1);
        assert!(report.variants_accepted.contains(&AntigravityProductVariant::Ide));
        assert_eq!(usage.len(), 1);
        assert_eq!(usage[0].records[0].response_id.as_deref(), Some("response-ide"));
    }

    #[test]
    fn schema_mismatch_rejects_fallback_without_panicking() {
        let directory = TempDir::new().expect("tempdir");
        let path = directory.path().join("app-session.db");
        {
            let connection = Connection::open(&path).expect("open");
            connection
                .execute_batch("CREATE TABLE gen_metadata (id integer, payload blob);")
                .expect("schema");
        }

        let (_, report) = collect_app_ide_sqlite_fallback(&[conversation(
            AntigravityProductVariant::App,
            &path,
        )]);

        assert_eq!(report.conversations_accepted, 0);
        assert_eq!(report.conversations_rejected, 1);
        assert!(report.variants_rejected.contains(&AntigravityProductVariant::App));
    }
}