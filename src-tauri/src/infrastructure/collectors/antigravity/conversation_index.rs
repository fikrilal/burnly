use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use chrono::{DateTime, NaiveDate, NaiveTime, TimeZone, Utc};
use chrono_tz::Tz;

use crate::application::collection::CollectionScope;

use super::product_variant::AntigravityProductVariant;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConversationDatabase {
    pub(crate) variant: AntigravityProductVariant,
    pub(crate) conversation_id: String,
    pub(crate) path: PathBuf,
    pub(crate) modified_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub(crate) struct ConversationIndex {
    data_root: PathBuf,
}

impl ConversationIndex {
    pub(crate) fn from_data_root(data_root: impl Into<PathBuf>) -> Self {
        Self {
            data_root: data_root.into(),
        }
    }

    pub(crate) fn from_home(home: impl AsRef<Path>) -> Self {
        Self::from_data_root(home.as_ref().join(".gemini"))
    }

    pub(crate) fn default() -> Self {
        default_home_directory()
            .map(Self::from_home)
            .unwrap_or_else(|| Self::from_home("."))
    }

    pub(crate) fn list(
        &self,
        scope: &CollectionScope,
        aggregation_timezone: &str,
    ) -> Result<Vec<ConversationDatabase>, ConversationIndexError> {
        let time_window = TimeWindow::from_scope(scope, aggregation_timezone)?;
        let mut databases = Vec::new();
        for variant in AntigravityProductVariant::all() {
            databases.extend(self.list_variant(variant, time_window)?);
        }
        databases.sort_by(|left, right| {
            right
                .modified_at
                .cmp(&left.modified_at)
                .then_with(|| left.variant.as_str().cmp(right.variant.as_str()))
                .then_with(|| left.conversation_id.cmp(&right.conversation_id))
        });
        Ok(databases)
    }

    fn list_variant(
        &self,
        variant: AntigravityProductVariant,
        time_window: TimeWindow,
    ) -> Result<Vec<ConversationDatabase>, ConversationIndexError> {
        let conversations_dir = variant_data_root(&self.data_root, variant).join("conversations");
        if !conversations_dir.exists() {
            return Ok(Vec::new());
        }

        let mut databases: BTreeMap<String, ConversationDatabase> = BTreeMap::new();
        let entries =
            fs::read_dir(&conversations_dir).map_err(|_| ConversationIndexError::UnreadableRoot)?;
        for entry in entries {
            let entry = entry.map_err(|_| ConversationIndexError::UnreadableRoot)?;
            let path = entry.path();
            if !is_conversation_artifact(&path) {
                continue;
            }
            let Some(conversation_id) = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .map(str::to_owned)
            else {
                continue;
            };
            if conversation_id.trim().is_empty() {
                continue;
            }

            let metadata = entry
                .metadata()
                .map_err(|_| ConversationIndexError::UnreadableRoot)?;
            if !metadata.is_file() {
                continue;
            }
            let modified_at = metadata
                .modified()
                .map_err(|_| ConversationIndexError::UnreadableRoot)?
                .duration_since(UNIX_EPOCH)
                .map_err(|_| ConversationIndexError::InvalidModifiedTime)
                .and_then(|duration| {
                    DateTime::<Utc>::from_timestamp(
                        i64::try_from(duration.as_secs())
                            .map_err(|_| ConversationIndexError::InvalidModifiedTime)?,
                        duration.subsec_nanos(),
                    )
                    .ok_or(ConversationIndexError::InvalidModifiedTime)
                })?;
            if !time_window.contains(modified_at) {
                continue;
            }

            let database = ConversationDatabase {
                variant,
                conversation_id: conversation_id.clone(),
                path,
                modified_at,
            };
            match databases.get(&conversation_id) {
                Some(existing) if existing.modified_at >= database.modified_at => {}
                _ => {
                    databases.insert(conversation_id, database);
                }
            }
        }
        Ok(databases.into_values().collect())
    }
}

fn variant_data_root(data_root: &Path, variant: AntigravityProductVariant) -> PathBuf {
    if variant == AntigravityProductVariant::Cli {
        if let Ok(home) = std::env::var("GEMINI_CLI_HOME") {
            let trimmed = home.trim();
            if !trimmed.is_empty() {
                return PathBuf::from(trimmed);
            }
        }
    }
    data_root.join(variant.data_dir_name())
}

fn default_home_directory() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

fn is_conversation_artifact(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("db" | "pb")
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TimeWindow {
    start: Option<DateTime<Utc>>,
    end: Option<DateTime<Utc>>,
}

impl TimeWindow {
    fn from_scope(
        scope: &CollectionScope,
        aggregation_timezone: &str,
    ) -> Result<Self, ConversationIndexError> {
        match scope {
            CollectionScope::Full => Ok(Self {
                start: None,
                end: None,
            }),
            CollectionScope::Incremental(scope) => {
                let timezone = aggregation_timezone
                    .parse::<Tz>()
                    .map_err(|_| ConversationIndexError::InvalidTimezone)?;
                let start = local_midnight(timezone, scope.start_date())?;
                let end_date = scope
                    .end_date()
                    .succ_opt()
                    .ok_or(ConversationIndexError::InvalidDateRange)?;
                let end = local_midnight(timezone, end_date)?;
                Ok(Self {
                    start: Some(start),
                    end: Some(end),
                })
            }
        }
    }

    fn contains(self, value: DateTime<Utc>) -> bool {
        self.start.is_none_or(|start| value >= start) && self.end.is_none_or(|end| value < end)
    }
}

fn local_midnight(timezone: Tz, date: NaiveDate) -> Result<DateTime<Utc>, ConversationIndexError> {
    timezone
        .from_local_datetime(&date.and_time(NaiveTime::MIN))
        .single()
        .map(|datetime| datetime.with_timezone(&Utc))
        .ok_or(ConversationIndexError::InvalidDateRange)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConversationIndexError {
    InvalidDateRange,
    InvalidModifiedTime,
    InvalidTimezone,
    UnreadableRoot,
}

#[cfg(test)]
mod tests {
    use std::fs::File;

    use chrono::TimeZone;
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn lists_conversation_databases_for_all_variants_newest_first() {
        let directory = TempDir::new().expect("tempdir");
        create_db(
            directory.path(),
            AntigravityProductVariant::Cli,
            "cli-session",
        );
        create_db(
            directory.path(),
            AntigravityProductVariant::App,
            "app-session",
        );
        create_ignored_file(
            directory.path(),
            AntigravityProductVariant::Ide,
            "notes.txt",
        );

        let index = ConversationIndex::from_data_root(directory.path());
        let databases = index
            .list(&CollectionScope::Full, "UTC")
            .expect("conversation index");

        let mut ids = databases
            .iter()
            .map(|database| database.conversation_id.as_str())
            .collect::<Vec<_>>();
        ids.sort_unstable();

        assert_eq!(ids, vec!["app-session", "cli-session"]);
    }

    #[test]
    fn lists_proto_conversation_artifacts() {
        let directory = TempDir::new().expect("tempdir");
        create_pb(
            directory.path(),
            AntigravityProductVariant::Ide,
            "proto-session",
        );

        let index = ConversationIndex::from_data_root(directory.path());
        let databases = index
            .list(&CollectionScope::Full, "UTC")
            .expect("conversation index");

        assert_eq!(databases.len(), 1);
        assert_eq!(databases[0].conversation_id, "proto-session");
        assert_eq!(databases[0].variant, AntigravityProductVariant::Ide);
    }

    #[test]
    fn deduplicates_conversation_artifacts_with_the_same_id() {
        let directory = TempDir::new().expect("tempdir");
        create_db(
            directory.path(),
            AntigravityProductVariant::App,
            "same-session",
        );
        create_pb(
            directory.path(),
            AntigravityProductVariant::App,
            "same-session",
        );

        let index = ConversationIndex::from_data_root(directory.path());
        let databases = index
            .list(&CollectionScope::Full, "UTC")
            .expect("conversation index");

        assert_eq!(databases.len(), 1);
        assert_eq!(databases[0].conversation_id, "same-session");
    }

    #[test]
    fn filters_incremental_scope_by_database_modified_date() {
        let in_window = Utc
            .with_ymd_and_hms(2026, 7, 2, 4, 0, 0)
            .single()
            .expect("timestamp");
        let out_window = Utc
            .with_ymd_and_hms(2026, 6, 30, 4, 0, 0)
            .single()
            .expect("timestamp");
        let window = TimeWindow::from_scope(
            &CollectionScope::incremental(
                NaiveDate::from_ymd_opt(2026, 7, 2).expect("date"),
                NaiveDate::from_ymd_opt(2026, 7, 2).expect("date"),
            )
            .expect("scope"),
            "UTC",
        )
        .expect("window");

        assert!(window.contains(in_window));
        assert!(!window.contains(out_window));
    }

    #[test]
    fn missing_variant_roots_are_empty() {
        let directory = TempDir::new().expect("tempdir");
        let index = ConversationIndex::from_data_root(directory.path());

        let databases = index
            .list(&CollectionScope::Full, "UTC")
            .expect("conversation index");

        assert!(databases.is_empty());
    }

    #[test]
    fn builds_index_from_home_directory() {
        let index = ConversationIndex::from_home(r"C:\Users\burnly");

        assert_eq!(
            index.data_root,
            PathBuf::from(r"C:\Users\burnly").join(".gemini")
        );
    }

    #[test]
    fn gemini_cli_home_overrides_cli_variant_root() {
        let data_root = TempDir::new().expect("data root");
        let cli_home = TempDir::new().expect("cli home");
        create_db_at(
            cli_home.path(),
            Path::new("conversations"),
            "cli-from-env",
        );

        let previous = std::env::var("GEMINI_CLI_HOME").ok();
        std::env::set_var("GEMINI_CLI_HOME", cli_home.path());
        let result = (|| {
            let index = ConversationIndex::from_data_root(data_root.path());
            index.list(&CollectionScope::Full, "UTC")
        })();
        if let Some(value) = previous {
            std::env::set_var("GEMINI_CLI_HOME", value);
        } else {
            std::env::remove_var("GEMINI_CLI_HOME");
        }

        let databases = result.expect("conversation index");
        assert_eq!(databases.len(), 1);
        assert_eq!(databases[0].conversation_id, "cli-from-env");
        assert_eq!(databases[0].variant, AntigravityProductVariant::Cli);
        assert_eq!(
            databases[0].path,
            cli_home
                .path()
                .join("conversations")
                .join("cli-from-env.db")
        );
    }

    fn create_db(root: &Path, variant: AntigravityProductVariant, name: &str) {
        let directory = root.join(variant.data_dir_name()).join("conversations");
        create_db_at(root, directory.strip_prefix(root).expect("relative"), name);
    }

    fn create_db_at(root: &Path, relative_dir: &Path, name: &str) {
        let directory = root.join(relative_dir);
        fs::create_dir_all(&directory).expect("conversation dir");
        File::create(directory.join(format!("{name}.db"))).expect("db file");
    }

    fn create_pb(root: &Path, variant: AntigravityProductVariant, name: &str) {
        let directory = root.join(variant.data_dir_name()).join("conversations");
        fs::create_dir_all(&directory).expect("conversation dir");
        File::create(directory.join(format!("{name}.pb"))).expect("pb file");
    }

    fn create_ignored_file(root: &Path, variant: AntigravityProductVariant, name: &str) {
        let directory = root.join(variant.data_dir_name()).join("conversations");
        fs::create_dir_all(&directory).expect("conversation dir");
        File::create(directory.join(name)).expect("ignored file");
    }
}
