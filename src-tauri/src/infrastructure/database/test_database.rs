use std::path::{Path, PathBuf};

use tempfile::TempDir;

use super::Database;

pub(super) struct TestDatabase {
    _directory: TempDir,
    path: PathBuf,
    database: Database,
}

impl TestDatabase {
    pub(super) fn open() -> Self {
        let directory = TempDir::new().expect("create temporary database directory");
        let path = directory.path().join("burnly.sqlite3");
        let database = Database::open(&path).expect("open temporary database");

        Self {
            _directory: directory,
            path,
            database,
        }
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    pub(super) fn database(&self) -> &Database {
        &self.database
    }

    pub(super) fn database_mut(&mut self) -> &mut Database {
        &mut self.database
    }
}
