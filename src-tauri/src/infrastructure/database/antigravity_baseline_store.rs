#![allow(
    dead_code,
    reason = "chunk 01 defines baseline store implementation consumed in chunks 03-05"
)]

use std::sync::Mutex;

use rusqlite::params;

use crate::application::ports::antigravity_baseline_store::{
    AntigravityBaselineRecord, AntigravityBaselineStatus, AntigravityBaselineStore,
    AntigravityBaselineStoreError, AntigravityBaselineVariant,
};

use super::Database;

pub(crate) struct SqliteAntigravityBaselineStore {
    database: Mutex<Database>,
}

impl SqliteAntigravityBaselineStore {
    pub(crate) fn new(database: Database) -> Self {
        Self {
            database: Mutex::new(database),
        }
    }
}

impl AntigravityBaselineStore for SqliteAntigravityBaselineStore {
    fn get_status(
        &self,
        variant: AntigravityBaselineVariant,
    ) -> Result<Option<AntigravityBaselineStatus>, AntigravityBaselineStoreError> {
        let database = self
            .database
            .lock()
            .map_err(|e| AntigravityBaselineStoreError::Database(e.to_string()))?;
        let result: Result<String, rusqlite::Error> = database.connection().query_row(
            "SELECT status FROM antigravity_baseline_state WHERE variant = ?1",
            [variant.as_str()],
            |row| row.get(0),
        );
        match result {
            Ok(status_str) => AntigravityBaselineStatus::from_str(&status_str)
                .map(Some)
                .ok_or_else(|| {
                    AntigravityBaselineStoreError::Database(format!(
                        "invalid baseline status in database: {status_str}"
                    ))
                }),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(AntigravityBaselineStoreError::Database(e.to_string())),
        }
    }

    fn begin_baseline(
        &self,
        variant: AntigravityBaselineVariant,
        started_at_ms: i64,
    ) -> Result<(), AntigravityBaselineStoreError> {
        let database = self
            .database
            .lock()
            .map_err(|e| AntigravityBaselineStoreError::Database(e.to_string()))?;
        database
            .connection()
            .execute(
                "INSERT INTO antigravity_baseline_state (
                    variant, status, started_at_ms, completed_at_ms, updated_at_ms
                ) VALUES (?1, 'pending', ?2, NULL, ?2)
                ON CONFLICT(variant) DO UPDATE SET
                    status = 'pending',
                    updated_at_ms = ?2",
                params![variant.as_str(), started_at_ms],
            )
            .map_err(|e| AntigravityBaselineStoreError::Database(e.to_string()))?;
        Ok(())
    }

    fn complete_baseline(
        &self,
        variant: AntigravityBaselineVariant,
        completed_at_ms: i64,
    ) -> Result<(), AntigravityBaselineStoreError> {
        let database = self
            .database
            .lock()
            .map_err(|e| AntigravityBaselineStoreError::Database(e.to_string()))?;
        database
            .connection()
            .execute(
                "INSERT INTO antigravity_baseline_state (
                    variant, status, started_at_ms, completed_at_ms, updated_at_ms
                ) VALUES (?1, 'complete', ?2, ?2, ?2)
                ON CONFLICT(variant) DO UPDATE SET
                    status = 'complete',
                    completed_at_ms = ?2,
                    updated_at_ms = ?2",
                params![variant.as_str(), completed_at_ms],
            )
            .map_err(|e| AntigravityBaselineStoreError::Database(e.to_string()))?;
        Ok(())
    }

    fn complete_all_variants(
        &self,
        completed_at_ms: i64,
    ) -> Result<(), AntigravityBaselineStoreError> {
        for variant in AntigravityBaselineVariant::all() {
            self.complete_baseline(variant, completed_at_ms)?;
        }
        Ok(())
    }

    fn list_statuses(
        &self,
    ) -> Result<Vec<AntigravityBaselineRecord>, AntigravityBaselineStoreError> {
        let database = self
            .database
            .lock()
            .map_err(|e| AntigravityBaselineStoreError::Database(e.to_string()))?;
        let mut statement = database
            .connection()
            .prepare(
                "SELECT variant, status, started_at_ms, completed_at_ms, updated_at_ms
                 FROM antigravity_baseline_state
                 ORDER BY variant ASC",
            )
            .map_err(|e| AntigravityBaselineStoreError::Database(e.to_string()))?;

        let rows = statement
            .query_map([], |row| {
                let variant_str: String = row.get(0)?;
                let status_str: String = row.get(1)?;
                let started_at_ms: i64 = row.get(2)?;
                let completed_at_ms: Option<i64> = row.get(3)?;
                let updated_at_ms: i64 = row.get(4)?;
                Ok((
                    variant_str,
                    status_str,
                    started_at_ms,
                    completed_at_ms,
                    updated_at_ms,
                ))
            })
            .map_err(|e| AntigravityBaselineStoreError::Database(e.to_string()))?;

        let mut records = Vec::new();
        for row in rows {
            let (variant_str, status_str, started_at_ms, completed_at_ms, updated_at_ms) =
                row.map_err(|e| AntigravityBaselineStoreError::Database(e.to_string()))?;
            let variant = AntigravityBaselineVariant::from_str(&variant_str).ok_or_else(|| {
                AntigravityBaselineStoreError::Database(format!(
                    "invalid baseline variant in database: {variant_str}"
                ))
            })?;
            let status = AntigravityBaselineStatus::from_str(&status_str).ok_or_else(|| {
                AntigravityBaselineStoreError::Database(format!(
                    "invalid baseline status in database: {status_str}"
                ))
            })?;
            records.push(AntigravityBaselineRecord {
                variant,
                status,
                started_at_ms,
                completed_at_ms,
                updated_at_ms,
            });
        }
        Ok(records)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::database::test_database::TestDatabase;

    #[test]
    fn baseline_store_tracks_status_transitions() {
        let mut test_db = TestDatabase::open();
        test_db
            .database_mut()
            .migrate_to_latest()
            .expect("migrate to latest");
        let path = test_db.path().to_path_buf();
        let store = SqliteAntigravityBaselineStore::new(Database::open(&path).expect("open db"));

        // Initially empty
        assert_eq!(
            store
                .get_status(AntigravityBaselineVariant::App)
                .expect("get status"),
            None
        );

        // Begin baseline
        store
            .begin_baseline(AntigravityBaselineVariant::App, 1000)
            .expect("begin baseline");
        assert_eq!(
            store
                .get_status(AntigravityBaselineVariant::App)
                .expect("get status"),
            Some(AntigravityBaselineStatus::Pending)
        );

        // Complete baseline
        store
            .complete_baseline(AntigravityBaselineVariant::App, 2000)
            .expect("complete baseline");
        assert_eq!(
            store
                .get_status(AntigravityBaselineVariant::App)
                .expect("get status"),
            Some(AntigravityBaselineStatus::Complete)
        );

        // Verify list_statuses
        let records = store.list_statuses().expect("list statuses");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].variant, AntigravityBaselineVariant::App);
        assert_eq!(records[0].status, AntigravityBaselineStatus::Complete);
        assert_eq!(records[0].started_at_ms, 1000);
        assert_eq!(records[0].completed_at_ms, Some(2000));
        assert_eq!(records[0].updated_at_ms, 2000);
    }

    #[test]
    fn complete_all_variants_marks_every_variant() {
        let mut test_db = TestDatabase::open();
        test_db
            .database_mut()
            .migrate_to_latest()
            .expect("migrate to latest");
        let path = test_db.path().to_path_buf();
        let store = SqliteAntigravityBaselineStore::new(Database::open(&path).expect("open db"));

        store.complete_all_variants(5000).expect("complete all");

        let records = store.list_statuses().expect("list statuses");
        assert_eq!(records.len(), 3);
        for record in records {
            assert_eq!(record.status, AntigravityBaselineStatus::Complete);
            assert_eq!(record.completed_at_ms, Some(5000));
        }
    }
}
