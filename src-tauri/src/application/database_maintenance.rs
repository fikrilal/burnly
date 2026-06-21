use std::sync::Arc;

use thiserror::Error;

use crate::application::ports::database_maintenance::{
    CheckpointOutcome, DatabaseAccess, DatabaseMaintenanceStore, DatabaseMaintenanceStoreError,
    IntegrityOutcome, MaintenanceActivity, MaintenanceGuard,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MaintenanceStatus {
    pub access: DatabaseAccess,
    pub schema_version: Option<i64>,
    pub backup_available: bool,
    pub maintenance_available: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MaintenanceActionOutcome {
    IntegrityHealthy,
    IntegrityCorrupt,
    Checkpoint(CheckpointOutcome),
    Vacuumed,
    Restored,
}

pub(crate) struct DatabaseMaintenanceService {
    store: Arc<dyn DatabaseMaintenanceStore>,
    guard: Arc<dyn MaintenanceGuard>,
}

impl DatabaseMaintenanceService {
    pub(crate) fn new(
        store: Arc<dyn DatabaseMaintenanceStore>,
        guard: Arc<dyn MaintenanceGuard>,
    ) -> Self {
        Self { store, guard }
    }

    pub(crate) fn status(&self) -> Result<MaintenanceStatus, MaintenanceError> {
        let record = self.store.recovery_status()?;
        Ok(MaintenanceStatus {
            maintenance_available: record.access == DatabaseAccess::ReadWrite
                && self.guard.activity() == MaintenanceActivity::Idle,
            access: record.access,
            schema_version: record.schema_version,
            backup_available: record.backup_available,
        })
    }

    pub(crate) fn integrity_check(&self) -> Result<MaintenanceActionOutcome, MaintenanceError> {
        self.ensure_idle()?;
        match self.store.integrity_check()? {
            IntegrityOutcome::Healthy => Ok(MaintenanceActionOutcome::IntegrityHealthy),
            IntegrityOutcome::Corrupt => Ok(MaintenanceActionOutcome::IntegrityCorrupt),
        }
    }

    pub(crate) fn checkpoint(&self) -> Result<MaintenanceActionOutcome, MaintenanceError> {
        self.ensure_idle()?;
        Ok(MaintenanceActionOutcome::Checkpoint(
            self.store.checkpoint()?,
        ))
    }

    pub(crate) fn vacuum(&self) -> Result<MaintenanceActionOutcome, MaintenanceError> {
        self.ensure_idle()?;
        self.store.vacuum()?;
        Ok(MaintenanceActionOutcome::Vacuumed)
    }

    pub(crate) fn restore_migration_backup(
        &self,
    ) -> Result<MaintenanceActionOutcome, MaintenanceError> {
        self.ensure_idle()?;
        self.store.restore_migration_backup()?;
        Ok(MaintenanceActionOutcome::Restored)
    }

    fn ensure_idle(&self) -> Result<(), MaintenanceError> {
        if self.guard.activity() == MaintenanceActivity::Busy {
            Err(MaintenanceError::ActiveOperation)
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MaintenanceError {
    #[error("database maintenance is blocked by an active operation")]
    ActiveOperation,
    #[error("database is unavailable")]
    Unavailable,
    #[error("database is read only")]
    ReadOnly,
    #[error("database is busy")]
    Busy,
    #[error("database maintenance returned invalid values")]
    InvalidStoredValue,
}

impl From<DatabaseMaintenanceStoreError> for MaintenanceError {
    fn from(value: DatabaseMaintenanceStoreError) -> Self {
        match value {
            DatabaseMaintenanceStoreError::Unavailable => Self::Unavailable,
            DatabaseMaintenanceStoreError::ReadOnly => Self::ReadOnly,
            DatabaseMaintenanceStoreError::Busy => Self::Busy,
            DatabaseMaintenanceStoreError::InvalidStoredValue => Self::InvalidStoredValue,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::ports::database_maintenance::RecoveryRecord;

    struct BusyGuard;

    impl MaintenanceGuard for BusyGuard {
        fn activity(&self) -> MaintenanceActivity {
            MaintenanceActivity::Busy
        }
    }

    struct UnexpectedStore;

    impl DatabaseMaintenanceStore for UnexpectedStore {
        fn recovery_status(&self) -> Result<RecoveryRecord, DatabaseMaintenanceStoreError> {
            panic!("store must not be called while maintenance is blocked")
        }

        fn integrity_check(&self) -> Result<IntegrityOutcome, DatabaseMaintenanceStoreError> {
            panic!("store must not be called while maintenance is blocked")
        }

        fn checkpoint(&self) -> Result<CheckpointOutcome, DatabaseMaintenanceStoreError> {
            panic!("store must not be called while maintenance is blocked")
        }

        fn vacuum(&self) -> Result<(), DatabaseMaintenanceStoreError> {
            panic!("store must not be called while maintenance is blocked")
        }

        fn restore_migration_backup(&self) -> Result<(), DatabaseMaintenanceStoreError> {
            panic!("store must not be called while maintenance is blocked")
        }
    }

    #[test]
    fn active_operation_blocks_maintenance_before_storage_is_touched() {
        let service =
            DatabaseMaintenanceService::new(Arc::new(UnexpectedStore), Arc::new(BusyGuard));

        assert_eq!(
            service.integrity_check(),
            Err(MaintenanceError::ActiveOperation)
        );
        assert_eq!(service.checkpoint(), Err(MaintenanceError::ActiveOperation));
        assert_eq!(service.vacuum(), Err(MaintenanceError::ActiveOperation));
        assert_eq!(
            service.restore_migration_backup(),
            Err(MaintenanceError::ActiveOperation)
        );
    }
}
