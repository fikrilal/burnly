use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MaintenanceActivity {
    Idle,
    Busy,
}

pub(crate) trait MaintenanceGuard: Send + Sync {
    fn activity(&self) -> MaintenanceActivity;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IntegrityOutcome {
    Healthy,
    Corrupt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CheckpointOutcome {
    pub busy: u32,
    pub log_frames: u32,
    pub checkpointed_frames: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DatabaseAccess {
    ReadWrite,
    ReadOnly,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecoveryRecord {
    pub access: DatabaseAccess,
    pub schema_version: Option<i64>,
    pub backup_available: bool,
}

pub(crate) trait DatabaseMaintenanceStore: Send + Sync {
    fn recovery_status(&self) -> Result<RecoveryRecord, DatabaseMaintenanceStoreError>;
    fn integrity_check(&self) -> Result<IntegrityOutcome, DatabaseMaintenanceStoreError>;
    fn checkpoint(&self) -> Result<CheckpointOutcome, DatabaseMaintenanceStoreError>;
    fn vacuum(&self) -> Result<(), DatabaseMaintenanceStoreError>;
    fn restore_migration_backup(&self) -> Result<(), DatabaseMaintenanceStoreError>;
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DatabaseMaintenanceStoreError {
    #[error("database is unavailable")]
    Unavailable,
    #[error("database is read only")]
    ReadOnly,
    #[error("database is busy")]
    Busy,
    #[error("database maintenance values are invalid")]
    InvalidStoredValue,
}
