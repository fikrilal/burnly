use std::path::Path;

use crate::application::diagnostics::{
    DiagnosticArea, DiagnosticCode, DiagnosticContext, DiagnosticEvent, DiagnosticSeverity,
    DiagnosticSummary,
};
use crate::application::ports::diagnostic_recorder::DiagnosticRecorder;
use crate::infrastructure::database::{Database, SqliteDiagnosticStore, SqliteReconciliationStore};

use super::StartupError;

pub(super) fn initialize_database(
    database_path: &Path,
    reporting_timezone: &str,
    created_at_ms: i64,
) -> Result<Database, StartupError> {
    let mut database = Database::open(database_path).map_err(StartupError::Persistence)?;
    if database
        .needs_migration()
        .map_err(StartupError::Persistence)?
    {
        database
            .create_verified_migration_backup(database_path)
            .map_err(StartupError::Persistence)?;
    }
    database
        .migrate_to_latest()
        .map_err(StartupError::Persistence)?;
    database
        .verify_health()
        .map_err(StartupError::Persistence)?;
    database
        .ensure_app_settings(reporting_timezone, created_at_ms)
        .map_err(StartupError::Persistence)?;

    Ok(database)
}

pub(super) fn recover_interrupted_runs(
    database_path: &Path,
    now_ms: i64,
) -> Result<(), StartupError> {
    let database = Database::open(database_path).map_err(StartupError::Persistence)?;
    let recovery = SqliteReconciliationStore::new(database)
        .recover_interrupted_runs(now_ms)
        .map_err(StartupError::RunRecovery)?;
    record_recovery_diagnostic(
        database_path,
        now_ms,
        recovery.refresh_runs,
        recovery.import_runs,
    );

    Ok(())
}

fn record_recovery_diagnostic(
    database_path: &Path,
    now_ms: i64,
    refresh_runs: usize,
    import_runs: usize,
) {
    if refresh_runs == 0 && import_runs == 0 {
        return;
    }

    let Ok(database) = Database::open(database_path) else {
        return;
    };
    let Ok(code) = DiagnosticCode::new("refresh.interrupted_recovered") else {
        return;
    };
    let Ok(summary) = DiagnosticSummary::new("Recovered interrupted refresh state at startup.")
    else {
        return;
    };
    let Ok(context) = DiagnosticContext::new(
        serde_json::json!({
            "refreshRuns": refresh_runs,
            "importRuns": import_runs
        })
        .to_string(),
    ) else {
        return;
    };
    let Ok(event) = DiagnosticEvent::new(
        DiagnosticArea::Refresh,
        DiagnosticSeverity::Warning,
        code,
        summary,
        Some(context),
        now_ms,
    ) else {
        return;
    };

    SqliteDiagnosticStore::new(database).record(event);
}
