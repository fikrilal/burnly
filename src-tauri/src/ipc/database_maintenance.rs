use serde::Serialize;
use tauri::State;

use crate::application::database_maintenance::{
    DatabaseMaintenanceService, MaintenanceActionOutcome, MaintenanceError, MaintenanceStatus,
};
use crate::application::ports::database_maintenance::DatabaseAccess;

use super::response::{ErrorCategory, IpcError, IpcResponse};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DatabaseMaintenanceStatusResponse {
    access: &'static str,
    schema_version: Option<i64>,
    backup_available: bool,
    maintenance_available: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DatabaseMaintenanceActionResponse {
    status: &'static str,
    message: &'static str,
    checkpoint: Option<CheckpointResponse>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CheckpointResponse {
    busy: u32,
    log_frames: u32,
    checkpointed_frames: u32,
}

#[tauri::command]
pub(super) fn database_get_maintenance_status(
    service: State<'_, DatabaseMaintenanceService>,
) -> IpcResponse<DatabaseMaintenanceStatusResponse> {
    match service.status() {
        Ok(status) => IpcResponse::success(status.into()),
        Err(error) => IpcResponse::failure(maintenance_error(error)),
    }
}

#[tauri::command]
pub(super) fn database_integrity_check(
    service: State<'_, DatabaseMaintenanceService>,
) -> IpcResponse<DatabaseMaintenanceActionResponse> {
    action_response(service.integrity_check())
}

#[tauri::command]
pub(super) fn database_checkpoint(
    service: State<'_, DatabaseMaintenanceService>,
) -> IpcResponse<DatabaseMaintenanceActionResponse> {
    action_response(service.checkpoint())
}

#[tauri::command]
pub(super) fn database_vacuum(
    service: State<'_, DatabaseMaintenanceService>,
) -> IpcResponse<DatabaseMaintenanceActionResponse> {
    action_response(service.vacuum())
}

#[tauri::command]
pub(super) fn database_restore_migration_backup(
    service: State<'_, DatabaseMaintenanceService>,
) -> IpcResponse<DatabaseMaintenanceActionResponse> {
    action_response(service.restore_migration_backup())
}

fn action_response(
    result: Result<MaintenanceActionOutcome, MaintenanceError>,
) -> IpcResponse<DatabaseMaintenanceActionResponse> {
    match result {
        Ok(outcome) => IpcResponse::success(outcome.into()),
        Err(error) => IpcResponse::failure(maintenance_error(error)),
    }
}

impl From<MaintenanceStatus> for DatabaseMaintenanceStatusResponse {
    fn from(value: MaintenanceStatus) -> Self {
        Self {
            access: match value.access {
                DatabaseAccess::ReadWrite => "read_write",
                DatabaseAccess::ReadOnly => "read_only",
                DatabaseAccess::Unavailable => "unavailable",
            },
            schema_version: value.schema_version,
            backup_available: value.backup_available,
            maintenance_available: value.maintenance_available,
        }
    }
}

impl From<MaintenanceActionOutcome> for DatabaseMaintenanceActionResponse {
    fn from(value: MaintenanceActionOutcome) -> Self {
        match value {
            MaintenanceActionOutcome::IntegrityHealthy => Self {
                status: "healthy",
                message: "Database integrity check passed.",
                checkpoint: None,
            },
            MaintenanceActionOutcome::IntegrityCorrupt => Self {
                status: "corrupt",
                message: "Database integrity check found corruption.",
                checkpoint: None,
            },
            MaintenanceActionOutcome::Checkpoint(outcome) => Self {
                status: if outcome.busy == 0 {
                    "checkpointed"
                } else {
                    "busy"
                },
                message: if outcome.busy == 0 {
                    "WAL checkpoint completed."
                } else {
                    "WAL checkpoint could not process every frame."
                },
                checkpoint: Some(CheckpointResponse {
                    busy: outcome.busy,
                    log_frames: outcome.log_frames,
                    checkpointed_frames: outcome.checkpointed_frames,
                }),
            },
            MaintenanceActionOutcome::Vacuumed => Self {
                status: "vacuumed",
                message: "Database vacuum completed.",
                checkpoint: None,
            },
            MaintenanceActionOutcome::Restored => Self {
                status: "restored",
                message: "The verified pre-migration backup was restored.",
                checkpoint: None,
            },
        }
    }
}

fn maintenance_error(error: MaintenanceError) -> IpcError {
    match error {
        MaintenanceError::ActiveOperation => IpcError::new(
            "database.active_operation",
            "Wait for the active refresh to finish before database maintenance.",
            ErrorCategory::Conflict,
            true,
        ),
        MaintenanceError::Unavailable => IpcError::new(
            "database.maintenance_unavailable",
            "Database maintenance is unavailable.",
            ErrorCategory::Unavailable,
            true,
        ),
        MaintenanceError::ReadOnly => IpcError::new(
            "database.read_only",
            "The database is read-only.",
            ErrorCategory::Permission,
            false,
        ),
        MaintenanceError::Busy => IpcError::new(
            "database.busy",
            "The database is busy. Try again after active work finishes.",
            ErrorCategory::Conflict,
            true,
        ),
        MaintenanceError::InvalidStoredValue => IpcError::new(
            "database.invalid_maintenance_result",
            "The database returned an invalid maintenance result.",
            ErrorCategory::Persistence,
            false,
        ),
    }
}
