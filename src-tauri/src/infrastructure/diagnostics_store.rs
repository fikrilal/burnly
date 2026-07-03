#![allow(
    dead_code,
    reason = "Diagnostics report reads are introduced before the report UI is wired"
)]

use std::sync::Mutex;

use rusqlite::params;

use crate::application::diagnostics::{
    DiagnosticArea, DiagnosticCode, DiagnosticContext, DiagnosticEvent, DiagnosticSeverity,
    DiagnosticSummary, DiagnosticValidationError, StoredDiagnosticEvent,
};
use crate::application::ports::diagnostic_recorder::DiagnosticRecorder;

use super::database::{Database, PersistenceError};

const MAX_DIAGNOSTIC_EVENTS: i64 = 500;
const DIAGNOSTIC_EVENT_RETENTION_MS: i64 = 14 * 24 * 60 * 60 * 1_000;

pub(crate) struct SqliteDiagnosticStore {
    database: Mutex<Database>,
}

impl SqliteDiagnosticStore {
    pub(crate) fn new(database: Database) -> Self {
        Self {
            database: Mutex::new(database),
        }
    }

    pub(crate) fn recent_events(
        &self,
        limit: u32,
    ) -> Result<Vec<StoredDiagnosticEvent>, PersistenceError> {
        let database = self
            .database
            .lock()
            .map_err(|_| PersistenceError::invalid_stored_value("diagnostic_events.lock"))?;
        read_recent_events(&database, i64::from(limit))
    }

    fn insert_event(&self, event: &DiagnosticEvent) -> Result<(), PersistenceError> {
        let mut database = self
            .database
            .lock()
            .map_err(|_| PersistenceError::invalid_stored_value("diagnostic_events.lock"))?;
        let transaction = database
            .connection_mut()
            .transaction()
            .map_err(|source| PersistenceError::read("diagnostic_events insert", source))?;

        transaction
            .execute(
                "INSERT INTO diagnostic_events (
                    area, severity, code, summary, context_json, created_at_ms
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    event.area.as_str(),
                    event.severity.as_str(),
                    event.code.as_str(),
                    event.summary.as_str(),
                    event.context.as_ref().map(DiagnosticContext::as_str),
                    event.created_at_ms,
                ],
            )
            .map_err(|source| PersistenceError::read("diagnostic_events insert", source))?;

        apply_retention(&transaction, event.created_at_ms)?;
        transaction
            .commit()
            .map_err(|source| PersistenceError::read("diagnostic_events commit", source))?;

        Ok(())
    }
}

impl DiagnosticRecorder for SqliteDiagnosticStore {
    fn record(&self, event: DiagnosticEvent) {
        let _ = self.insert_event(&event);
    }
}

fn apply_retention(
    transaction: &rusqlite::Transaction<'_>,
    now_ms: i64,
) -> Result<(), PersistenceError> {
    let cutoff_ms = now_ms.saturating_sub(DIAGNOSTIC_EVENT_RETENTION_MS);
    transaction
        .execute(
            "DELETE FROM diagnostic_events
             WHERE created_at_ms < ?1
                OR id NOT IN (
                    SELECT id FROM diagnostic_events
                    ORDER BY created_at_ms DESC, id DESC
                    LIMIT ?2
                )",
            params![cutoff_ms, MAX_DIAGNOSTIC_EVENTS],
        )
        .map_err(|source| PersistenceError::read("diagnostic_events retention", source))?;
    Ok(())
}

fn read_recent_events(
    database: &Database,
    limit: i64,
) -> Result<Vec<StoredDiagnosticEvent>, PersistenceError> {
    let mut statement = database
        .connection()
        .prepare(
            "SELECT id, area, severity, code, summary, context_json, created_at_ms
             FROM diagnostic_events
             ORDER BY created_at_ms DESC, id DESC
             LIMIT ?1",
        )
        .map_err(|source| PersistenceError::read("diagnostic_events recent", source))?;
    let events = statement
        .query_map([limit], |row| {
            let context = row
                .get::<_, Option<String>>(5)?
                .map(DiagnosticContext::new)
                .transpose()
                .map_err(invalid_diagnostic_value)?;
            let area = DiagnosticArea::from_storage(row.get::<_, String>(1)?.as_str())
                .ok_or(DiagnosticValidationError::Context)
                .map_err(invalid_diagnostic_value)?;
            let severity = DiagnosticSeverity::from_storage(row.get::<_, String>(2)?.as_str())
                .ok_or(DiagnosticValidationError::Context)
                .map_err(invalid_diagnostic_value)?;
            let event = DiagnosticEvent::new(
                area,
                severity,
                DiagnosticCode::new(row.get::<_, String>(3)?).map_err(invalid_diagnostic_value)?,
                DiagnosticSummary::new(row.get::<_, String>(4)?)
                    .map_err(invalid_diagnostic_value)?,
                context,
                row.get(6)?,
            )
            .map_err(invalid_diagnostic_value)?;
            StoredDiagnosticEvent::new(row.get(0)?, event).map_err(invalid_diagnostic_value)
        })
        .map_err(|source| PersistenceError::read("diagnostic_events recent", source))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| PersistenceError::read("diagnostic_events recent", source))?;
    Ok(events)
}

fn invalid_diagnostic_value(error: DiagnosticValidationError) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn store() -> SqliteDiagnosticStore {
        let directory = tempfile::TempDir::new().expect("temporary directory");
        let path = directory.keep().join("burnly.sqlite3");
        let mut database = Database::open(path).expect("open database");
        database.migrate_to_latest().expect("migrate database");
        SqliteDiagnosticStore::new(database)
    }

    fn event(created_at_ms: i64) -> DiagnosticEvent {
        DiagnosticEvent::new(
            DiagnosticArea::Collector,
            DiagnosticSeverity::Warning,
            DiagnosticCode::new("collector.source_failed").expect("code"),
            DiagnosticSummary::new("A source failed during collection.").expect("summary"),
            Some(
                DiagnosticContext::new(
                    json!({
                        "source": "antigravity",
                        "status": "failed"
                    })
                    .to_string(),
                )
                .expect("context"),
            ),
            created_at_ms,
        )
        .expect("event")
    }

    #[test]
    fn records_and_reads_recent_diagnostic_events() {
        let store = store();

        store.record(event(100));

        let events = store.recent_events(10).expect("recent events");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event.area, DiagnosticArea::Collector);
        assert_eq!(events[0].event.severity, DiagnosticSeverity::Warning);
        assert_eq!(events[0].event.code.as_str(), "collector.source_failed");
        assert_eq!(events[0].event.created_at_ms, 100);
    }

    #[test]
    fn retention_keeps_recent_bounded_window() {
        let store = store();
        let old_event = event(1);
        store.record(old_event);

        for offset in 0..=MAX_DIAGNOSTIC_EVENTS {
            store.record(event(DIAGNOSTIC_EVENT_RETENTION_MS + offset));
        }

        let events = store
            .recent_events((MAX_DIAGNOSTIC_EVENTS + 10) as u32)
            .expect("recent events");
        assert_eq!(events.len(), MAX_DIAGNOSTIC_EVENTS as usize);
        assert!(events
            .iter()
            .all(|stored| stored.event.created_at_ms >= DIAGNOSTIC_EVENT_RETENTION_MS));
        assert_eq!(
            events.first().map(|stored| stored.event.created_at_ms),
            Some(DIAGNOSTIC_EVENT_RETENTION_MS + MAX_DIAGNOSTIC_EVENTS)
        );
    }
}
