use std::path::Path;

use rusqlite::{Connection, OpenFlags};

pub(in crate::infrastructure::collectors) fn open_external_read_only(
    path: impl AsRef<Path>,
) -> Result<Connection, rusqlite::Error> {
    Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
}

#[cfg(test)]
mod tests {
    use tempfile::NamedTempFile;

    use super::*;

    #[test]
    fn opens_existing_database_without_write_access() {
        let database = NamedTempFile::new().expect("temp database");
        let writable = Connection::open(database.path()).expect("writable database");
        writable
            .execute("CREATE TABLE fixture (id INTEGER PRIMARY KEY)", [])
            .expect("schema");
        drop(writable);

        let readonly = open_external_read_only(database.path()).expect("readonly database");

        let error = readonly
            .execute("INSERT INTO fixture (id) VALUES (1)", [])
            .expect_err("readonly write rejected");
        assert_eq!(
            error.sqlite_error_code(),
            Some(rusqlite::ErrorCode::ReadOnly)
        );
    }

    #[test]
    fn missing_database_is_not_created() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("missing.sqlite");

        let error = open_external_read_only(&path).expect_err("missing database");

        assert!(!path.exists());
        assert_eq!(
            error.sqlite_error_code(),
            Some(rusqlite::ErrorCode::CannotOpen)
        );
    }
}
