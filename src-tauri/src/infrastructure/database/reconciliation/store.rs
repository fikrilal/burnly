//! Shared reconciliation store struct, connection helpers, and `UsageStore`
//! trait implementation.

use std::sync::Mutex;

use rusqlite::Connection;

use crate::application::ports::run_store::RunStoreError;
use crate::application::ports::usage_store::{UsageStore, UsageStoreError};
use crate::application::reconciliation::{
    DailyReconciliationRequest, DailyReconciliationSummary, SessionReconciliationRequest,
    SessionReconciliationSummary,
};

use super::super::Database;
use super::daily;
use super::session;

pub(crate) struct SqliteReconciliationStore {
    pub(super) database: Mutex<Database>,
}

impl SqliteReconciliationStore {
    pub(crate) fn new(database: Database) -> Self {
        Self {
            database: Mutex::new(database),
        }
    }

    pub(super) fn with_connection<T>(
        &self,
        operation: impl FnOnce(&Connection) -> Result<T, RunStoreError>,
    ) -> Result<T, RunStoreError> {
        let database = self.database.lock().map_err(|_| RunStoreError::Backend)?;
        operation(database.connection())
    }
}

impl UsageStore for SqliteReconciliationStore {
    fn reconcile_daily(
        &self,
        request: DailyReconciliationRequest,
    ) -> Result<DailyReconciliationSummary, UsageStoreError> {
        let mut database = self.database.lock().map_err(|_| UsageStoreError::Backend)?;
        let transaction = database
            .connection_mut()
            .transaction()
            .map_err(|_| UsageStoreError::Backend)?;

        let summary = daily::reconcile_daily_in_transaction(&transaction, &request)?;

        transaction.commit().map_err(|_| UsageStoreError::Backend)?;
        Ok(summary)
    }

    fn reconcile_session(
        &self,
        request: SessionReconciliationRequest,
    ) -> Result<SessionReconciliationSummary, UsageStoreError> {
        let mut database = self.database.lock().map_err(|_| UsageStoreError::Backend)?;
        let transaction = database
            .connection_mut()
            .transaction()
            .map_err(|_| UsageStoreError::Backend)?;

        let summary = session::reconcile_session_in_transaction(&transaction, &request)?;

        transaction.commit().map_err(|_| UsageStoreError::Backend)?;
        Ok(summary)
    }
}
