#![allow(
    dead_code,
    reason = "chunk 03 defines baseline repair service consumed in chunks 04 and 05"
)]

use std::sync::{Arc, Mutex};

use rusqlite::params;
use serde_json::json;

use crate::application::diagnostics::{
    DiagnosticArea, DiagnosticCode, DiagnosticContext, DiagnosticEvent, DiagnosticSeverity,
    DiagnosticSummary,
};
use crate::application::ports::baseline_repair::{
    AntigravityBaselineRepairStage, BaselineRepairError,
};
use crate::application::ports::diagnostic_recorder::DiagnosticRecorder;
use crate::infrastructure::database::Database;

const REPAIR_VERSION: i64 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AntigravityBaselineRepairStateRecord {
    pub(crate) repair_version: i64,
    pub(crate) stage: AntigravityBaselineRepairStage,
    pub(crate) records_reclassified: u64,
    pub(crate) import_run_id: Option<i64>,
    pub(crate) interval_started_at_ms: Option<i64>,
    pub(crate) interval_finished_at_ms: Option<i64>,
    pub(crate) stage_updated_at_ms: i64,
    pub(crate) skip_reason: Option<String>,
}

pub(crate) struct AntigravityBaselineRepairService {
    database: Mutex<Database>,
    diagnostics: Option<Arc<dyn DiagnosticRecorder>>,
}

impl AntigravityBaselineRepairService {
    pub(crate) fn new(database: Database) -> Self {
        Self {
            database: Mutex::new(database),
            diagnostics: None,
        }
    }

    pub(crate) fn with_diagnostics(
        database: Database,
        diagnostics: Option<Arc<dyn DiagnosticRecorder>>,
    ) -> Self {
        Self {
            database: Mutex::new(database),
            diagnostics,
        }
    }

    pub(crate) fn current_stage(
        &self,
    ) -> Result<AntigravityBaselineRepairStage, BaselineRepairError> {
        let database = self
            .database
            .lock()
            .map_err(|e| BaselineRepairError::Database(e.to_string()))?;
        Self::current_stage_from_conn(database.connection())
    }

    pub(crate) fn get_repair_state(
        &self,
    ) -> Result<Option<AntigravityBaselineRepairStateRecord>, BaselineRepairError> {
        let database = self
            .database
            .lock()
            .map_err(|e| BaselineRepairError::Database(e.to_string()))?;
        Self::repair_state_from_conn(database.connection())
    }

    pub(crate) fn ensure_cache_reclassified(
        &self,
        now_ms: i64,
    ) -> Result<AntigravityBaselineRepairStage, BaselineRepairError> {
        let mut database = self
            .database
            .lock()
            .map_err(|e| BaselineRepairError::Database(e.to_string()))?;

        let current = Self::current_stage_from_conn(database.connection())?;
        if current != AntigravityBaselineRepairStage::NotStarted {
            return Ok(current);
        }

        let transaction = database
            .connection_mut()
            .transaction()
            .map_err(|e| BaselineRepairError::Database(e.to_string()))?;

        let current = Self::current_stage_from_conn(&transaction)?;
        if current != AntigravityBaselineRepairStage::NotStarted {
            return Ok(current);
        }

        use rusqlite::OptionalExtension;

        // 1. Resolve source_id for Antigravity.
        let source_id: Option<i64> = transaction
            .query_row(
                "SELECT id FROM sources WHERE source_key = 'antigravity'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| BaselineRepairError::Database(e.to_string()))?;

        let Some(source_id) = source_id else {
            Self::persist_skipped(&transaction, None, "no_profile2_full_run", now_ms)?;
            transaction
                .commit()
                .map_err(|e| BaselineRepairError::Database(e.to_string()))?;
            self.record_diagnostic_skipped("no_profile2_full_run", None, now_ms);
            return Ok(AntigravityBaselineRepairStage::Skipped);
        };

        // 2. Earliest successful full daily import with profile_version = 2.
        struct ImportRunBounds {
            id: i64,
            refresh_run_id: i64,
            started_at_ms: i64,
            finished_at_ms: i64,
        }

        let daily_import: Option<ImportRunBounds> = transaction
            .query_row(
                "SELECT id, refresh_run_id, started_at_ms, finished_at_ms
                 FROM import_runs
                 WHERE source_id = ?1
                   AND profile_version = 2
                   AND projection = 'daily'
                   AND scope_kind = 'full'
                   AND status = 'succeeded'
                 ORDER BY id ASC
                 LIMIT 1",
                params![source_id],
                |row| {
                    Ok(ImportRunBounds {
                        id: row.get(0)?,
                        refresh_run_id: row.get(1)?,
                        started_at_ms: row.get(2)?,
                        finished_at_ms: row.get(3)?,
                    })
                },
            )
            .optional()
            .map_err(|e| BaselineRepairError::Database(e.to_string()))?;

        let Some(daily_import) = daily_import else {
            Self::persist_skipped(&transaction, None, "no_profile2_full_run", now_ms)?;
            transaction
                .commit()
                .map_err(|e| BaselineRepairError::Database(e.to_string()))?;
            self.record_diagnostic_skipped("no_profile2_full_run", None, now_ms);
            return Ok(AntigravityBaselineRepairStage::Skipped);
        };

        // 3. Matching successful full session import in the same refresh run.
        let session_import: Option<ImportRunBounds> = transaction
            .query_row(
                "SELECT id, refresh_run_id, started_at_ms, finished_at_ms
                 FROM import_runs
                 WHERE source_id = ?1
                   AND refresh_run_id = ?2
                   AND profile_version = 2
                   AND projection = 'session'
                   AND scope_kind = 'full'
                   AND status = 'succeeded'
                 LIMIT 1",
                params![source_id, daily_import.refresh_run_id],
                |row| {
                    Ok(ImportRunBounds {
                        id: row.get(0)?,
                        refresh_run_id: row.get(1)?,
                        started_at_ms: row.get(2)?,
                        finished_at_ms: row.get(3)?,
                    })
                },
            )
            .optional()
            .map_err(|e| BaselineRepairError::Database(e.to_string()))?;

        let Some(session_import) = session_import else {
            Self::persist_skipped(
                &transaction,
                Some(daily_import.id),
                "missing_matching_session_run",
                now_ms,
            )?;
            transaction
                .commit()
                .map_err(|e| BaselineRepairError::Database(e.to_string()))?;
            self.record_diagnostic_skipped(
                "missing_matching_session_run",
                Some(daily_import.id),
                now_ms,
            );
            return Ok(AntigravityBaselineRepairStage::Skipped);
        };

        // 4. Absolute absence of prior profile-2 attempts.
        let earliest_id = daily_import.id.min(session_import.id);
        let prior_attempt_exists: bool = transaction
            .query_row(
                "SELECT 1 FROM import_runs
                 WHERE source_id = ?1
                   AND profile_version = 2
                   AND id < ?2
                 LIMIT 1",
                params![source_id, earliest_id],
                |_| Ok(true),
            )
            .optional()
            .map_err(|e| BaselineRepairError::Database(e.to_string()))?
            .unwrap_or(false);

        if prior_attempt_exists {
            Self::persist_skipped(
                &transaction,
                Some(daily_import.id),
                "prior_profile2_runs_exist",
                now_ms,
            )?;
            transaction
                .commit()
                .map_err(|e| BaselineRepairError::Database(e.to_string()))?;
            self.record_diagnostic_skipped(
                "prior_profile2_runs_exist",
                Some(daily_import.id),
                now_ms,
            );
            return Ok(AntigravityBaselineRepairStage::Skipped);
        }

        // 5. Strict timestamp bounds and reclassification.
        let window_start_ms = daily_import.started_at_ms.min(session_import.started_at_ms);
        let window_end_ms = daily_import
            .finished_at_ms
            .max(session_import.finished_at_ms);

        let records_reclassified = transaction
            .execute(
                "UPDATE antigravity_usage_cache
                 SET calendar_attribution = 'undated_baseline'
                 WHERE timestamp_origin IN ('first_seen', 'legacy_unknown')
                   AND first_seen_at_ms >= ?1 AND first_seen_at_ms <= ?2",
                params![window_start_ms, window_end_ms],
            )
            .map_err(|e| BaselineRepairError::Database(e.to_string()))?;

        let records_reclassified_i64 = i64::try_from(records_reclassified).unwrap_or(i64::MAX);
        let records_reclassified_u64 = u64::try_from(records_reclassified).unwrap_or(0);

        transaction
            .execute(
                "INSERT INTO antigravity_baseline_repair_state (
                    repair_version,
                    stage,
                    records_reclassified,
                    import_run_id,
                    interval_started_at_ms,
                    interval_finished_at_ms,
                    stage_updated_at_ms,
                    skip_reason
                ) VALUES (
                    ?1,
                    'cache_reclassified',
                    ?2,
                    ?3,
                    ?4,
                    ?5,
                    ?6,
                    NULL
                )
                ON CONFLICT(repair_version) DO UPDATE SET
                    stage = excluded.stage,
                    records_reclassified = excluded.records_reclassified,
                    import_run_id = excluded.import_run_id,
                    interval_started_at_ms = excluded.interval_started_at_ms,
                    interval_finished_at_ms = excluded.interval_finished_at_ms,
                    stage_updated_at_ms = excluded.stage_updated_at_ms,
                    skip_reason = excluded.skip_reason",
                params![
                    REPAIR_VERSION,
                    records_reclassified_i64,
                    daily_import.id,
                    window_start_ms,
                    window_end_ms,
                    now_ms,
                ],
            )
            .map_err(|e| BaselineRepairError::Database(e.to_string()))?;

        transaction
            .commit()
            .map_err(|e| BaselineRepairError::Database(e.to_string()))?;

        self.record_diagnostic_applied(
            records_reclassified_u64,
            daily_import.id,
            window_start_ms,
            window_end_ms,
            now_ms,
        );

        Ok(AntigravityBaselineRepairStage::CacheReclassified)
    }

    fn current_stage_from_conn(
        conn: &rusqlite::Connection,
    ) -> Result<AntigravityBaselineRepairStage, BaselineRepairError> {
        use rusqlite::OptionalExtension;
        let stage_str: Option<String> = conn
            .query_row(
                "SELECT stage FROM antigravity_baseline_repair_state WHERE repair_version = ?1",
                params![REPAIR_VERSION],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| BaselineRepairError::Database(e.to_string()))?;

        match stage_str.as_deref() {
            Some(s) => AntigravityBaselineRepairStage::from_str(s)
                .ok_or_else(|| BaselineRepairError::Database(format!("invalid repair stage: {s}"))),
            None => Ok(AntigravityBaselineRepairStage::NotStarted),
        }
    }

    fn repair_state_from_conn(
        conn: &rusqlite::Connection,
    ) -> Result<Option<AntigravityBaselineRepairStateRecord>, BaselineRepairError> {
        use rusqlite::OptionalExtension;
        conn.query_row(
            "SELECT repair_version, stage, records_reclassified, import_run_id,
                    interval_started_at_ms, interval_finished_at_ms, stage_updated_at_ms,
                    skip_reason
             FROM antigravity_baseline_repair_state
             WHERE repair_version = ?1",
            params![REPAIR_VERSION],
            |row| {
                let stage_str: String = row.get(1)?;
                let stage = AntigravityBaselineRepairStage::from_str(&stage_str)
                    .ok_or_else(|| rusqlite::Error::InvalidQuery)?;
                let records_reclassified: i64 = row.get(2)?;
                Ok(AntigravityBaselineRepairStateRecord {
                    repair_version: row.get(0)?,
                    stage,
                    records_reclassified: u64::try_from(records_reclassified).unwrap_or(0),
                    import_run_id: row.get(3)?,
                    interval_started_at_ms: row.get(4)?,
                    interval_finished_at_ms: row.get(5)?,
                    stage_updated_at_ms: row.get(6)?,
                    skip_reason: row.get(7)?,
                })
            },
        )
        .optional()
        .map_err(|e| BaselineRepairError::Database(e.to_string()))
    }

    fn persist_skipped(
        conn: &rusqlite::Connection,
        import_run_id: Option<i64>,
        skip_reason: &str,
        now_ms: i64,
    ) -> Result<(), BaselineRepairError> {
        conn.execute(
            "INSERT INTO antigravity_baseline_repair_state (
                repair_version,
                stage,
                records_reclassified,
                import_run_id,
                interval_started_at_ms,
                interval_finished_at_ms,
                stage_updated_at_ms,
                skip_reason
            ) VALUES (
                ?1,
                'skipped',
                0,
                ?2,
                NULL,
                NULL,
                ?3,
                ?4
            )
            ON CONFLICT(repair_version) DO UPDATE SET
                stage = excluded.stage,
                records_reclassified = excluded.records_reclassified,
                import_run_id = excluded.import_run_id,
                interval_started_at_ms = excluded.interval_started_at_ms,
                interval_finished_at_ms = excluded.interval_finished_at_ms,
                stage_updated_at_ms = excluded.stage_updated_at_ms,
                skip_reason = excluded.skip_reason",
            params![REPAIR_VERSION, import_run_id, now_ms, skip_reason],
        )
        .map_err(|e| BaselineRepairError::Database(e.to_string()))?;
        Ok(())
    }

    fn record_diagnostic_applied(
        &self,
        records_reclassified: u64,
        import_run_id: i64,
        interval_started_at_ms: i64,
        interval_finished_at_ms: i64,
        now_ms: i64,
    ) {
        let Some(recorder) = &self.diagnostics else {
            return;
        };
        let Ok(code) = DiagnosticCode::new("antigravity.baseline_repair_applied") else {
            return;
        };
        let Ok(summary) = DiagnosticSummary::new(format!(
            "Reclassified {records_reclassified} historical Antigravity cache records to undated baseline"
        )) else {
            return;
        };
        let context = DiagnosticContext::new(
            json!({
                "source": "antigravity",
                "repairVersion": REPAIR_VERSION,
                "stage": "cache_reclassified",
                "recordsReclassified": records_reclassified,
                "importRunId": import_run_id,
                "intervalStartedAtMs": interval_started_at_ms,
                "intervalFinishedAtMs": interval_finished_at_ms,
            })
            .to_string(),
        )
        .ok();
        let Ok(event) = DiagnosticEvent::new(
            DiagnosticArea::Collector,
            DiagnosticSeverity::Info,
            code,
            summary,
            context,
            now_ms,
        ) else {
            return;
        };
        recorder.record(event);
    }

    fn record_diagnostic_skipped(
        &self,
        skip_reason: &str,
        import_run_id: Option<i64>,
        now_ms: i64,
    ) {
        let Some(recorder) = &self.diagnostics else {
            return;
        };
        let Ok(code) = DiagnosticCode::new("antigravity.baseline_repair_skipped") else {
            return;
        };
        let Ok(summary) = DiagnosticSummary::new(format!(
            "Antigravity baseline repair skipped: {skip_reason}"
        )) else {
            return;
        };
        let context = DiagnosticContext::new(
            json!({
                "source": "antigravity",
                "repairVersion": REPAIR_VERSION,
                "stage": "skipped",
                "skipReason": skip_reason,
                "importRunId": import_run_id,
            })
            .to_string(),
        )
        .ok();
        let Ok(event) = DiagnosticEvent::new(
            DiagnosticArea::Collector,
            DiagnosticSeverity::Info,
            code,
            summary,
            context,
            now_ms,
        ) else {
            return;
        };
        recorder.record(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::database::test_database::TestDatabase;

    #[derive(Default)]
    struct RecordingDiagnosticRecorder {
        events: Mutex<Vec<DiagnosticEvent>>,
    }

    impl DiagnosticRecorder for RecordingDiagnosticRecorder {
        fn record(&self, event: DiagnosticEvent) {
            self.events.lock().unwrap().push(event);
        }
    }

    fn seed_source(conn: &rusqlite::Connection) -> i64 {
        conn.execute(
            "INSERT INTO sources (source_key, display_name, enabled, detection_state, created_at_ms, updated_at_ms)
             VALUES ('antigravity', 'Antigravity', 1, 'available', 1000, 1000)",
            [],
        )
        .expect("seed source");
        conn.last_insert_rowid()
    }

    fn seed_refresh_run(conn: &rusqlite::Connection, id: i64, started_at_ms: i64) {
        conn.execute(
            "INSERT INTO refresh_runs (id, job_key, trigger, status, started_at_ms, finished_at_ms, requested_by_app_version, created_at_ms)
             VALUES (?1, 'job-' || ?1, 'launch', 'succeeded', ?2, ?2 + 1000, '0.1.28', ?2)",
            params![id, started_at_ms],
        )
        .expect("seed refresh run");
    }

    #[allow(clippy::too_many_arguments)]
    fn seed_import_run(
        conn: &rusqlite::Connection,
        id: i64,
        refresh_run_id: i64,
        source_id: i64,
        profile_version: i64,
        projection: &str,
        scope_kind: &str,
        status: &str,
        started_at_ms: i64,
        finished_at_ms: Option<i64>,
    ) {
        conn.execute(
            "INSERT INTO import_runs (
                id, refresh_run_id, source_id, collector_key, collector_version,
                profile_version, projection, scope_kind, aggregation_timezone,
                status, records_seen, records_rejected, started_at_ms, finished_at_ms
            ) VALUES (
                ?1, ?2, ?3, 'antigravity', '0.1.28',
                ?4, ?5, ?6, 'UTC',
                ?7, 10, 0, ?8, ?9
            )",
            params![
                id,
                refresh_run_id,
                source_id,
                profile_version,
                projection,
                scope_kind,
                status,
                started_at_ms,
                finished_at_ms,
            ],
        )
        .expect("seed import run");
    }

    fn seed_cache_row(
        conn: &rusqlite::Connection,
        dedupe_key: &str,
        variant: &str,
        first_seen_at_ms: i64,
        timestamp_origin: &str,
    ) {
        conn.execute(
            "INSERT INTO antigravity_usage_cache (
                dedupe_key, variant, conversation_id, response_id, raw_model_id,
                model_label, api_provider, input_tokens, output_tokens,
                thinking_output_tokens, response_output_tokens, cache_read_tokens,
                cache_write_tokens, observed_at_ms, collector_version,
                first_seen_at_ms, last_seen_at_ms, source_record_index,
                timestamp_origin, calendar_attribution
            ) VALUES (
                ?1, ?2, 'conv-1', ?1, 'model-1',
                'Model 1', NULL, 10, 2,
                0, 2, 0,
                0, ?3, '0.1.28',
                ?3, ?3, 0,
                ?4, 'dated'
            )",
            params![dedupe_key, variant, first_seen_at_ms, timestamp_origin],
        )
        .expect("seed cache row");
    }

    #[test]
    fn proven_initial_run_reclassifies_first_seen_and_legacy_unknown_within_interval() {
        let mut test_database = TestDatabase::open();
        test_database.database_mut().migrate_to_latest().unwrap();
        let path = test_database.path().to_path_buf();
        let conn = &test_database.database().connection;

        let source_id = seed_source(conn);
        seed_refresh_run(conn, 100, 10_000);

        // Correlated initial full daily & session run
        seed_import_run(
            conn,
            1,
            100,
            source_id,
            2,
            "daily",
            "full",
            "succeeded",
            10_000,
            Some(12_000),
        );
        seed_import_run(
            conn,
            2,
            100,
            source_id,
            2,
            "session",
            "full",
            "succeeded",
            10_500,
            Some(12_500),
        );

        // Seed cache rows:
        // 1. CLI first_seen inside interval [10_000, 12_500] -> should be reclassified
        seed_cache_row(conn, "cli-1", "antigravity-cli", 11_000, "first_seen");
        // 2. App legacy_unknown inside interval -> should be reclassified
        seed_cache_row(conn, "app-1", "antigravity", 11_500, "legacy_unknown");
        // 3. IDE legacy_unknown inside interval -> should be reclassified
        seed_cache_row(conn, "ide-1", "antigravity-ide", 10_000, "legacy_unknown");
        // 4. Source-reported row inside interval -> MUST NOT be reclassified
        seed_cache_row(conn, "source-1", "antigravity", 11_000, "source_reported");
        // 5. First-seen row outside interval -> MUST NOT be reclassified
        seed_cache_row(conn, "cli-late", "antigravity-cli", 13_000, "first_seen");

        let recorder = Arc::new(RecordingDiagnosticRecorder::default());
        let db = Database::open(&path).expect("open db");
        let service =
            AntigravityBaselineRepairService::with_diagnostics(db, Some(recorder.clone()));

        let stage = service
            .ensure_cache_reclassified(20_000)
            .expect("reclassify");
        assert_eq!(stage, AntigravityBaselineRepairStage::CacheReclassified);

        // Verify state record
        let state = service
            .get_repair_state()
            .expect("get state")
            .expect("record present");
        assert_eq!(
            state.stage,
            AntigravityBaselineRepairStage::CacheReclassified
        );
        assert_eq!(state.records_reclassified, 3);
        assert_eq!(state.import_run_id, Some(1));
        assert_eq!(state.interval_started_at_ms, Some(10_000));
        assert_eq!(state.interval_finished_at_ms, Some(12_500));
        assert_eq!(state.skip_reason, None);

        // Verify cache rows in db
        let get_attribution = |key: &str| -> String {
            conn.query_row(
                "SELECT calendar_attribution FROM antigravity_usage_cache WHERE dedupe_key = ?1",
                [key],
                |r| r.get(0),
            )
            .unwrap()
        };
        assert_eq!(get_attribution("cli-1"), "undated_baseline");
        assert_eq!(get_attribution("app-1"), "undated_baseline");
        assert_eq!(get_attribution("ide-1"), "undated_baseline");
        assert_eq!(get_attribution("source-1"), "dated");
        assert_eq!(get_attribution("cli-late"), "dated");

        // Verify diagnostic event
        let events = recorder.events.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].code.as_str(),
            "antigravity.baseline_repair_applied"
        );
    }

    #[test]
    fn presence_of_earlier_failed_run_safely_skips_repair() {
        let mut test_database = TestDatabase::open();
        test_database.database_mut().migrate_to_latest().unwrap();
        let path = test_database.path().to_path_buf();
        let conn = &test_database.database().connection;

        let source_id = seed_source(conn);
        seed_refresh_run(conn, 50, 5_000);
        seed_refresh_run(conn, 100, 10_000);

        // Earlier failed attempt with records_seen = 0 (e.g. cancelled/failed)
        seed_import_run(
            conn,
            1,
            50,
            source_id,
            2,
            "daily",
            "full",
            "failed",
            5_000,
            Some(5_100),
        );
        // Later successful run
        seed_import_run(
            conn,
            2,
            100,
            source_id,
            2,
            "daily",
            "full",
            "succeeded",
            10_000,
            Some(12_000),
        );
        seed_import_run(
            conn,
            3,
            100,
            source_id,
            2,
            "session",
            "full",
            "succeeded",
            10_000,
            Some(12_000),
        );

        seed_cache_row(conn, "cli-1", "antigravity-cli", 11_000, "first_seen");

        let recorder = Arc::new(RecordingDiagnosticRecorder::default());
        let db = Database::open(&path).expect("open db");
        let service =
            AntigravityBaselineRepairService::with_diagnostics(db, Some(recorder.clone()));

        let stage = service
            .ensure_cache_reclassified(20_000)
            .expect("reclassify");
        assert_eq!(stage, AntigravityBaselineRepairStage::Skipped);

        let state = service.get_repair_state().unwrap().unwrap();
        assert_eq!(state.stage, AntigravityBaselineRepairStage::Skipped);
        assert_eq!(
            state.skip_reason.as_deref(),
            Some("prior_profile2_runs_exist")
        );
        assert_eq!(state.records_reclassified, 0);
        assert_eq!(state.import_run_id, Some(2));

        // Cache row unchanged
        let attribution: String = conn
            .query_row(
                "SELECT calendar_attribution FROM antigravity_usage_cache WHERE dedupe_key = 'cli-1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(attribution, "dated");

        let events = recorder.events.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].code.as_str(),
            "antigravity.baseline_repair_skipped"
        );
    }

    #[test]
    fn missing_matching_session_import_safely_skips_repair() {
        let mut test_database = TestDatabase::open();
        test_database.database_mut().migrate_to_latest().unwrap();
        let path = test_database.path().to_path_buf();
        let conn = &test_database.database().connection;

        let source_id = seed_source(conn);
        seed_refresh_run(conn, 100, 10_000);

        // Only daily succeeded, session missing or failed
        seed_import_run(
            conn,
            1,
            100,
            source_id,
            2,
            "daily",
            "full",
            "succeeded",
            10_000,
            Some(12_000),
        );

        let recorder = Arc::new(RecordingDiagnosticRecorder::default());
        let db = Database::open(&path).expect("open db");
        let service =
            AntigravityBaselineRepairService::with_diagnostics(db, Some(recorder.clone()));

        let stage = service
            .ensure_cache_reclassified(20_000)
            .expect("reclassify");
        assert_eq!(stage, AntigravityBaselineRepairStage::Skipped);

        let state = service.get_repair_state().unwrap().unwrap();
        assert_eq!(state.stage, AntigravityBaselineRepairStage::Skipped);
        assert_eq!(
            state.skip_reason.as_deref(),
            Some("missing_matching_session_run")
        );
    }

    #[test]
    fn pruned_history_safely_skips_repair() {
        let mut test_database = TestDatabase::open();
        test_database.database_mut().migrate_to_latest().unwrap();
        let path = test_database.path().to_path_buf();
        let conn = &test_database.database().connection;

        seed_source(conn);
        // No import_runs at all

        let recorder = Arc::new(RecordingDiagnosticRecorder::default());
        let db = Database::open(&path).expect("open db");
        let service =
            AntigravityBaselineRepairService::with_diagnostics(db, Some(recorder.clone()));

        let stage = service
            .ensure_cache_reclassified(20_000)
            .expect("reclassify");
        assert_eq!(stage, AntigravityBaselineRepairStage::Skipped);

        let state = service.get_repair_state().unwrap().unwrap();
        assert_eq!(state.stage, AntigravityBaselineRepairStage::Skipped);
        assert_eq!(state.skip_reason.as_deref(), Some("no_profile2_full_run"));
    }

    #[test]
    fn re_invoking_already_reclassified_repair_does_not_duplicate_sql_execution() {
        let mut test_database = TestDatabase::open();
        test_database.database_mut().migrate_to_latest().unwrap();
        let path = test_database.path().to_path_buf();
        let conn = &test_database.database().connection;

        let source_id = seed_source(conn);
        seed_refresh_run(conn, 100, 10_000);
        seed_import_run(
            conn,
            1,
            100,
            source_id,
            2,
            "daily",
            "full",
            "succeeded",
            10_000,
            Some(12_000),
        );
        seed_import_run(
            conn,
            2,
            100,
            source_id,
            2,
            "session",
            "full",
            "succeeded",
            10_000,
            Some(12_000),
        );
        seed_cache_row(conn, "cli-1", "antigravity-cli", 11_000, "first_seen");

        let recorder = Arc::new(RecordingDiagnosticRecorder::default());
        let db = Database::open(&path).expect("open db");
        let service =
            AntigravityBaselineRepairService::with_diagnostics(db, Some(recorder.clone()));

        let first_stage = service.ensure_cache_reclassified(20_000).unwrap();
        assert_eq!(
            first_stage,
            AntigravityBaselineRepairStage::CacheReclassified
        );
        assert_eq!(recorder.events.lock().unwrap().len(), 1);

        // Second invocation: should be no-op, same stage, no additional diagnostic events
        let second_stage = service.ensure_cache_reclassified(25_000).unwrap();
        assert_eq!(
            second_stage,
            AntigravityBaselineRepairStage::CacheReclassified
        );
        assert_eq!(recorder.events.lock().unwrap().len(), 1);

        // State update timestamp must remain the first one (20_000)
        let state = service.get_repair_state().unwrap().unwrap();
        assert_eq!(state.stage_updated_at_ms, 20_000);
    }
}
