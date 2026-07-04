use std::collections::BTreeMap;
use std::sync::Mutex;

use chrono::{DateTime, SecondsFormat, Utc};
use chrono_tz::Tz;
use rusqlite::params;
use serde_json::Value;

use crate::application::diagnostics::{
    DiagnosticArea, DiagnosticCode, DiagnosticContext, DiagnosticEvent, DiagnosticSeverity,
    DiagnosticSummary, DiagnosticValidationError, DiagnosticsAppReport, DiagnosticsDatabaseReport,
    DiagnosticsEnvironmentReport, DiagnosticsEventReport, DiagnosticsHealth,
    DiagnosticsHealthReason, DiagnosticsHealthStatus, DiagnosticsImportRunReport,
    DiagnosticsImportsReport, DiagnosticsRefreshReport, DiagnosticsRefreshRunReport,
    DiagnosticsReport, DiagnosticsReportError, DiagnosticsReportRequest, DiagnosticsRunErrorReport,
    DiagnosticsSourceReport, DiagnosticsSourcesReport, DiagnosticsUsageIntegrityReport,
    StoredDiagnosticEvent,
};
use crate::application::ports::diagnostic_recorder::DiagnosticRecorder;
use crate::application::ports::diagnostics_report_store::DiagnosticsReportStore;

use super::{Database, PersistenceError};

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

    #[cfg(test)]
    fn recent_events(&self, limit: u32) -> Result<Vec<StoredDiagnosticEvent>, PersistenceError> {
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

impl DiagnosticsReportStore for SqliteDiagnosticStore {
    fn report(
        &self,
        request: DiagnosticsReportRequest,
    ) -> Result<DiagnosticsReport, DiagnosticsReportError> {
        let database = self
            .database
            .lock()
            .map_err(|_| DiagnosticsReportError::Store)?;
        read_report(&database, request).map_err(|_| DiagnosticsReportError::Store)
    }
}

fn read_report(
    database: &Database,
    request: DiagnosticsReportRequest,
) -> Result<DiagnosticsReport, PersistenceError> {
    let generated_at = timestamp_ms_to_rfc3339(request.generated_at_ms);
    let today = reporting_date(request.generated_at_ms, &request.timezone);
    let refresh = DiagnosticsRefreshReport {
        latest_runs: read_latest_refresh_runs(database, 10)?,
    };
    let imports = DiagnosticsImportsReport {
        latest_runs: read_latest_import_runs(database, 30)?,
    };
    let sources = DiagnosticsSourcesReport {
        recent: read_recent_sources(database)?,
    };
    let usage_integrity = read_usage_integrity(database, &today)?;
    let diagnostic_events = read_recent_events(database, 30)?
        .into_iter()
        .map(diagnostic_event_report)
        .collect::<Vec<_>>();
    let database_report = DiagnosticsDatabaseReport {
        schema_version: database.schema_version()?,
        tables_present: required_tables_present(database)?,
    };
    let health = derive_health(
        generated_at.clone(),
        &database_report,
        &refresh,
        &imports,
        &usage_integrity,
        &diagnostic_events,
    );

    Ok(DiagnosticsReport {
        schema_version: 1,
        generated_at,
        app: DiagnosticsAppReport {
            version: request.app_version,
            platform: request.platform,
            arch: request.arch,
            debug: request.debug,
        },
        environment: DiagnosticsEnvironmentReport {
            timezone: request.timezone,
            locale: "redacted-or-unset".to_owned(),
        },
        health,
        database: database_report,
        refresh,
        imports,
        sources,
        usage_integrity,
        diagnostic_events,
    })
}

fn read_latest_refresh_runs(
    database: &Database,
    limit: i64,
) -> Result<Vec<DiagnosticsRefreshRunReport>, PersistenceError> {
    let mut statement = database
        .connection()
        .prepare(
            "SELECT id, trigger, status, started_at_ms, finished_at_ms,
                    requested_by_app_version, error_code, error_summary
             FROM refresh_runs
             ORDER BY created_at_ms DESC, id DESC
             LIMIT ?1",
        )
        .map_err(|source| PersistenceError::read("diagnostics.refresh_runs", source))?;

    let rows = statement
        .query_map([limit], |row| {
            let error_code = row.get::<_, Option<String>>(6)?;
            let error_summary = row.get::<_, Option<String>>(7)?;
            Ok(DiagnosticsRefreshRunReport {
                id: row.get::<_, i64>(0)?.to_string(),
                trigger: row.get(1)?,
                status: row.get(2)?,
                started_at: row.get::<_, Option<i64>>(3)?.map(timestamp_ms_to_rfc3339),
                finished_at: row.get::<_, Option<i64>>(4)?.map(timestamp_ms_to_rfc3339),
                requested_by_app_version: row.get(5)?,
                error: run_error(error_code, error_summary),
            })
        })
        .map_err(|source| PersistenceError::read("diagnostics.refresh_runs", source))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| PersistenceError::read("diagnostics.refresh_runs", source))?;
    Ok(rows)
}

fn read_latest_import_runs(
    database: &Database,
    limit: i64,
) -> Result<Vec<DiagnosticsImportRunReport>, PersistenceError> {
    let mut statement = database
        .connection()
        .prepare(
            "SELECT ir.id, ir.refresh_run_id, s.source_key, ir.collector_key,
                    ir.collector_version, ir.profile_version, ir.projection,
                    ir.scope_kind, ir.scope_start_date, ir.scope_end_date,
                    ir.status, ir.records_seen, ir.records_rejected,
                    ir.started_at_ms, ir.finished_at_ms, ir.error_code, ir.error_detail
             FROM import_runs ir
             JOIN sources s ON s.id = ir.source_id
             ORDER BY ir.started_at_ms DESC, ir.id DESC
             LIMIT ?1",
        )
        .map_err(|source| PersistenceError::read("diagnostics.import_runs", source))?;

    let rows = statement
        .query_map([limit], |row| {
            let error_code = row.get::<_, Option<String>>(15)?;
            let error_summary = row.get::<_, Option<String>>(16)?;
            Ok(DiagnosticsImportRunReport {
                id: row.get::<_, i64>(0)?.to_string(),
                refresh_run_id: row.get::<_, i64>(1)?.to_string(),
                source_id: row.get(2)?,
                collector_key: row.get(3)?,
                collector_version: row.get(4)?,
                profile_version: row.get(5)?,
                projection: row.get(6)?,
                scope_kind: row.get(7)?,
                scope_start_date: row.get(8)?,
                scope_end_date: row.get(9)?,
                status: row.get(10)?,
                records_seen: row.get::<_, i64>(11)?.to_string(),
                records_rejected: row.get::<_, i64>(12)?.to_string(),
                started_at: timestamp_ms_to_rfc3339(row.get(13)?),
                finished_at: row.get::<_, Option<i64>>(14)?.map(timestamp_ms_to_rfc3339),
                error: run_error(error_code, error_summary),
            })
        })
        .map_err(|source| PersistenceError::read("diagnostics.import_runs", source))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| PersistenceError::read("diagnostics.import_runs", source))?;
    Ok(rows)
}

fn read_recent_sources(
    database: &Database,
) -> Result<Vec<DiagnosticsSourceReport>, PersistenceError> {
    let mut statement = database
        .connection()
        .prepare(
            "SELECT s.source_key,
                    CASE WHEN s.enabled = 1 THEN 'enabled' ELSE 'disabled' END,
                    (
                        SELECT ir.status
                        FROM import_runs ir
                        WHERE ir.source_id = s.id
                        ORDER BY ir.started_at_ms DESC, ir.id DESC
                        LIMIT 1
                    ),
                    (
                        SELECT ir.projection
                        FROM import_runs ir
                        WHERE ir.source_id = s.id
                        ORDER BY ir.started_at_ms DESC, ir.id DESC
                        LIMIT 1
                    )
             FROM sources s
             ORDER BY s.source_key ASC",
        )
        .map_err(|source| PersistenceError::read("diagnostics.sources", source))?;

    let rows = statement
        .query_map([], |row| {
            Ok(DiagnosticsSourceReport {
                source_id: row.get(0)?,
                status: row.get(1)?,
                latest_import_status: row.get(2)?,
                latest_projection: row.get(3)?,
            })
        })
        .map_err(|source| PersistenceError::read("diagnostics.sources", source))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| PersistenceError::read("diagnostics.sources", source))?;
    Ok(rows)
}

fn read_usage_integrity(
    database: &Database,
    today: &str,
) -> Result<DiagnosticsUsageIntegrityReport, PersistenceError> {
    let (today_daily_usage_rows, today_daily_usage_token_sum): (i64, i64) = database
        .connection()
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(total_tokens), 0)
             FROM daily_usage
             WHERE usage_date = ?1 AND record_state <> 'removed'",
            [today],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|source| PersistenceError::read("diagnostics.daily_usage", source))?;

    let (today_daily_model_usage_rows, today_daily_model_usage_token_sum): (i64, i64) = database
        .connection()
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(COALESCE(dmu.total_tokens, 0)), 0)
             FROM daily_model_usage dmu
             JOIN daily_usage du ON du.id = dmu.daily_usage_id
             WHERE du.usage_date = ?1 AND du.record_state <> 'removed'",
            [today],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|source| PersistenceError::read("diagnostics.daily_model_usage", source))?;

    let orphan_daily_model_rows = database
        .connection()
        .query_row(
            "SELECT COUNT(*)
             FROM daily_model_usage dmu
             LEFT JOIN daily_usage du ON du.id = dmu.daily_usage_id
             WHERE du.id IS NULL",
            [],
            |row| row.get(0),
        )
        .map_err(|source| PersistenceError::read("diagnostics.daily_model_orphans", source))?;

    let model_rows_without_total_tokens = database
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM daily_model_usage WHERE total_tokens IS NULL",
            [],
            |row| row.get(0),
        )
        .map_err(|source| PersistenceError::read("diagnostics.daily_model_tokens", source))?;

    Ok(DiagnosticsUsageIntegrityReport {
        today_daily_usage_rows,
        today_daily_model_usage_rows,
        today_daily_usage_token_sum: today_daily_usage_token_sum.to_string(),
        today_daily_model_usage_token_sum: today_daily_model_usage_token_sum.to_string(),
        orphan_daily_model_rows,
        model_rows_without_total_tokens,
    })
}

fn required_tables_present(database: &Database) -> Result<bool, PersistenceError> {
    let count: i64 = database
        .connection()
        .query_row(
            "SELECT COUNT(*)
             FROM sqlite_master
             WHERE type = 'table'
               AND name IN (
                   'refresh_runs', 'import_runs', 'daily_usage',
                   'daily_model_usage', 'sources', 'diagnostic_events'
               )",
            [],
            |row| row.get(0),
        )
        .map_err(|source| PersistenceError::read("diagnostics.tables_present", source))?;
    Ok(count == 6)
}

fn derive_health(
    generated_at: String,
    database: &DiagnosticsDatabaseReport,
    refresh: &DiagnosticsRefreshReport,
    imports: &DiagnosticsImportsReport,
    usage_integrity: &DiagnosticsUsageIntegrityReport,
    diagnostic_events: &[DiagnosticsEventReport],
) -> DiagnosticsHealth {
    let mut reasons = Vec::new();
    let mut has_error = false;

    if !database.tables_present {
        has_error = true;
        reasons.push(DiagnosticsHealthReason::new(
            "diagnostics.database_tables_missing",
            "Burnly local storage is missing required tables.",
        ));
    }

    if usage_integrity.orphan_daily_model_rows > 0
        || usage_integrity.model_rows_without_total_tokens > 0
    {
        has_error = true;
        reasons.push(DiagnosticsHealthReason::new(
            "diagnostics.usage_integrity_failed",
            "Burnly local usage totals contain inconsistent rows.",
        ));
    }

    if usage_integrity.today_daily_model_usage_rows > 0
        && usage_integrity.today_daily_usage_rows == 0
    {
        reasons.push(DiagnosticsHealthReason::new(
            "diagnostics.daily_totals_missing",
            "Daily model usage exists but today summary totals are missing.",
        ));
    }

    if let Some(latest_refresh) = refresh.latest_runs.first() {
        match latest_refresh.status.as_str() {
            "failed" => {
                if refresh
                    .latest_runs
                    .iter()
                    .any(|run| run.status.as_str() == "succeeded")
                {
                    reasons.push(DiagnosticsHealthReason::new(
                        "diagnostics.refresh_failed",
                        "The latest refresh failed, but previous refresh data is available.",
                    ));
                } else {
                    has_error = true;
                    reasons.push(DiagnosticsHealthReason::new(
                        "diagnostics.refresh_failed",
                        "The latest refresh failed and no recent successful refresh is available.",
                    ));
                }
            }
            "partial" => reasons.push(DiagnosticsHealthReason::new(
                "diagnostics.refresh_partial",
                "The latest refresh completed with partial data.",
            )),
            _ => {}
        }
    }

    if let Some(latest_refresh) = refresh.latest_runs.first() {
        if imports
            .latest_runs
            .iter()
            .filter(|import| import.refresh_run_id == latest_refresh.id)
            .any(|import| matches!(import.status.as_str(), "failed" | "partial" | "cancelled"))
        {
            reasons.push(DiagnosticsHealthReason::new(
                "diagnostics.sources_failed",
                "Some sources failed during the last refresh.",
            ));
        }
    }

    if diagnostic_events
        .iter()
        .any(|event| event.severity.as_str() == "error")
    {
        has_error = true;
        reasons.push(DiagnosticsHealthReason::new(
            "diagnostics.recent_errors",
            "Burnly recorded recent local diagnostic errors.",
        ));
    } else if diagnostic_events
        .iter()
        .any(|event| event.severity.as_str() == "warning")
    {
        reasons.push(DiagnosticsHealthReason::new(
            "diagnostics.recent_warnings",
            "Burnly recorded recent local diagnostic warnings.",
        ));
    }

    reasons.sort_by(|left, right| left.code.cmp(&right.code));
    reasons.dedup_by(|left, right| left.code == right.code);

    DiagnosticsHealth {
        status: if has_error {
            DiagnosticsHealthStatus::Error
        } else if reasons.is_empty() {
            DiagnosticsHealthStatus::Ok
        } else {
            DiagnosticsHealthStatus::Warning
        },
        reasons,
        generated_at,
    }
}

fn run_error(code: Option<String>, summary: Option<String>) -> Option<DiagnosticsRunErrorReport> {
    match (code, summary) {
        (Some(code), Some(summary)) => Some(DiagnosticsRunErrorReport { code, summary }),
        (Some(code), None) => Some(DiagnosticsRunErrorReport {
            code,
            summary: "No error summary was recorded.".to_owned(),
        }),
        (None, Some(summary)) => Some(DiagnosticsRunErrorReport {
            code: "unknown".to_owned(),
            summary,
        }),
        (None, None) => None,
    }
}

fn diagnostic_event_report(stored: StoredDiagnosticEvent) -> DiagnosticsEventReport {
    DiagnosticsEventReport {
        id: stored.id.to_string(),
        area: stored.event.area.as_str().to_owned(),
        severity: stored.event.severity.as_str().to_owned(),
        code: stored.event.code.as_str().to_owned(),
        summary: stored.event.summary.as_str().to_owned(),
        context: stored
            .event
            .context
            .as_ref()
            .and_then(|context| safe_context_json(context.as_str())),
        created_at: timestamp_ms_to_rfc3339(stored.event.created_at_ms),
    }
}

fn safe_context_json(value: &str) -> Option<BTreeMap<String, String>> {
    match serde_json::from_str::<Value>(value) {
        Ok(Value::Object(object)) => Some(
            object
                .into_iter()
                .filter_map(|(key, value)| safe_context_value(value).map(|value| (key, value)))
                .collect(),
        ),
        _ => None,
    }
}

fn safe_context_value(value: Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value),
        Value::Bool(value) => Some(value.to_string()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn timestamp_ms_to_rfc3339(value: i64) -> String {
    DateTime::<Utc>::from_timestamp_millis(value)
        .unwrap_or(DateTime::<Utc>::UNIX_EPOCH)
        .to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn reporting_date(timestamp_ms: i64, timezone: &str) -> String {
    let instant =
        DateTime::<Utc>::from_timestamp_millis(timestamp_ms).unwrap_or(DateTime::<Utc>::UNIX_EPOCH);
    match timezone.parse::<Tz>() {
        Ok(timezone) => instant.with_timezone(&timezone).date_naive().to_string(),
        Err(_) => instant.date_naive().to_string(),
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
    use crate::application::ports::diagnostics_report_store::DiagnosticsReportStore;

    fn store() -> SqliteDiagnosticStore {
        let directory = tempfile::TempDir::new().expect("temporary directory");
        let path = directory.keep().join("burnly.sqlite3");
        let mut database = Database::open(path).expect("open database");
        database.migrate_to_latest().expect("migrate database");
        SqliteDiagnosticStore::new(database)
    }

    fn database() -> Database {
        let directory = tempfile::TempDir::new().expect("temporary directory");
        let path = directory.keep().join("burnly.sqlite3");
        let mut database = Database::open(path).expect("open database");
        database.migrate_to_latest().expect("migrate database");
        database
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

    #[test]
    fn report_derives_warning_health_from_latest_import_failure() {
        let database = database();
        seed_failed_import(&database);
        let store = SqliteDiagnosticStore::new(database);

        let report = store.report(report_request()).expect("diagnostics report");

        assert_eq!(report.schema_version, 1);
        assert_eq!(report.health.status, DiagnosticsHealthStatus::Warning);
        assert!(report
            .health
            .reasons
            .iter()
            .any(|reason| reason.code == "diagnostics.sources_failed"));
        assert_eq!(report.refresh.latest_runs[0].status, "partial");
        assert_eq!(report.imports.latest_runs[0].source_id, "antigravity");
        assert_eq!(report.sources.recent[0].source_id, "antigravity");
        assert!(report.sources.recent[0]
            .latest_import_status
            .as_ref()
            .is_some_and(|status| status == "failed"));
    }

    #[test]
    fn report_includes_recent_diagnostic_events_with_object_context_only() {
        let store = store();
        store.record(event(100));

        let report = store.report(report_request()).expect("diagnostics report");

        assert_eq!(report.diagnostic_events.len(), 1);
        assert_eq!(report.diagnostic_events[0].area, "collector");
        assert_eq!(
            report.diagnostic_events[0]
                .context
                .as_ref()
                .and_then(|context| context.get("source"))
                .map(String::as_str),
            Some("antigravity")
        );
    }

    fn report_request() -> DiagnosticsReportRequest {
        DiagnosticsReportRequest {
            generated_at_ms: 1_782_828_000_000,
            app_version: "0.1.14".to_owned(),
            platform: "linux".to_owned(),
            arch: "x86_64".to_owned(),
            debug: true,
            timezone: "Asia/Jakarta".to_owned(),
        }
    }

    fn seed_failed_import(database: &Database) {
        database
            .connection()
            .execute(
                "INSERT INTO sources (
                    id, source_key, display_name, enabled, detection_state,
                    created_at_ms, updated_at_ms
                ) VALUES (1, 'antigravity', 'Antigravity', 1, 'available', 100, 100)",
                [],
            )
            .expect("source");
        database
            .connection()
            .execute(
                "INSERT INTO refresh_runs (
                    id, job_key, trigger, status, started_at_ms, finished_at_ms,
                    requested_by_app_version, created_at_ms
                ) VALUES (1, 'job-1', 'manual', 'partial', 100, 200, '0.1.14', 100)",
                [],
            )
            .expect("refresh run");
        database
            .connection()
            .execute(
                "INSERT INTO import_runs (
                    id, refresh_run_id, source_id, collector_key, collector_version,
                    profile_version, projection, scope_kind, scope_start_date,
                    scope_end_date, aggregation_timezone, status, records_seen,
                    records_rejected, started_at_ms, finished_at_ms, error_code, error_detail
                ) VALUES (
                    1, 1, 1, 'antigravity', '1', 1, 'daily', 'incremental',
                    '2026-07-03', '2026-07-03', 'Asia/Jakarta', 'failed',
                    0, 0, 110, 180, 'collector.runtime_unavailable',
                    'Antigravity runtime unavailable.'
                )",
                [],
            )
            .expect("import run");
    }
}
