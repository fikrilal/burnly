use std::sync::Arc;

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::application::ports::history_deletion_store::{
    HistoryDeletionSnapshot, HistoryDeletionStore, HistoryDeletionStoreError,
};

pub(crate) const DELETE_CONFIRMATION: &str = "DELETE ALL HISTORY";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HistoryDeletionPreview {
    pub snapshot: HistoryDeletionSnapshot,
    pub total_records: u64,
    pub scope: String,
    pub preserved: Vec<String>,
    pub preview_token: String,
    pub can_delete: bool,
    pub confirmation_text: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConfirmedHistoryDeletion {
    pub preview_token: String,
    pub confirmation: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HistoryDeletionResult {
    pub deleted_records: u64,
}

pub(crate) struct HistoryDeletionService {
    store: Arc<dyn HistoryDeletionStore>,
}

impl HistoryDeletionService {
    pub(crate) fn new(store: Arc<dyn HistoryDeletionStore>) -> Self {
        Self { store }
    }

    pub(crate) fn preview(&self) -> Result<HistoryDeletionPreview, HistoryDeletionError> {
        let snapshot = self.store.preview()?;
        Ok(build_preview(snapshot))
    }

    pub(crate) fn delete(
        &self,
        request: ConfirmedHistoryDeletion,
    ) -> Result<HistoryDeletionResult, HistoryDeletionError> {
        if request.confirmation != DELETE_CONFIRMATION {
            return Err(HistoryDeletionError::ConfirmationRequired);
        }
        let current = self.store.preview()?;
        let preview = build_preview(current.clone());
        if preview.preview_token != request.preview_token {
            return Err(HistoryDeletionError::StalePreview);
        }
        if current.active_refresh {
            return Err(HistoryDeletionError::ActiveRefresh);
        }
        let deleted = self.store.delete(&current)?;
        Ok(HistoryDeletionResult {
            deleted_records: deleted.total_records(),
        })
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HistoryDeletionError {
    #[error("explicit deletion confirmation is required")]
    ConfirmationRequired,
    #[error("history deletion preview is stale")]
    StalePreview,
    #[error("history deletion is blocked by an active refresh")]
    ActiveRefresh,
    #[error("history deletion storage is unavailable")]
    Unavailable,
    #[error("history deletion values are invalid")]
    InvalidStoredValue,
}

impl From<HistoryDeletionStoreError> for HistoryDeletionError {
    fn from(value: HistoryDeletionStoreError) -> Self {
        match value {
            HistoryDeletionStoreError::Unavailable => Self::Unavailable,
            HistoryDeletionStoreError::InvalidStoredValue => Self::InvalidStoredValue,
            HistoryDeletionStoreError::StalePreview => Self::StalePreview,
            HistoryDeletionStoreError::ActiveRefresh => Self::ActiveRefresh,
        }
    }
}

fn build_preview(snapshot: HistoryDeletionSnapshot) -> HistoryDeletionPreview {
    let total_records = snapshot.total_records();
    let preview_token = snapshot_token(&snapshot);
    let can_delete = total_records > 0 && !snapshot.active_refresh;
    HistoryDeletionPreview {
        snapshot,
        total_records,
        preview_token,
        scope: "All imported history, all dates, and all sources.".to_owned(),
        preserved: vec![
            "Sources and source enablement".to_owned(),
            "Settings and notification preferences".to_owned(),
            "Budget definitions and thresholds".to_owned(),
            "Application configuration".to_owned(),
        ],
        can_delete,
        confirmation_text: DELETE_CONFIRMATION,
    }
}

fn snapshot_token(snapshot: &HistoryDeletionSnapshot) -> String {
    let mut digest = Sha256::new();
    for value in [
        snapshot.daily_usage,
        snapshot.daily_model_usage,
        snapshot.sessions,
        snapshot.session_model_usage,
        snapshot.refresh_runs,
        snapshot.import_runs,
        snapshot.projects,
        snapshot.source_models,
        snapshot.notification_records,
        snapshot.source_count,
    ] {
        digest.update(value.to_le_bytes());
    }
    digest.update(
        snapshot
            .earliest_date
            .as_deref()
            .unwrap_or_default()
            .as_bytes(),
    );
    digest.update([0]);
    digest.update(
        snapshot
            .latest_date
            .as_deref()
            .unwrap_or_default()
            .as_bytes(),
    );
    digest.update([u8::from(snapshot.active_refresh)]);
    format!("{:x}", digest.finalize())
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    struct FakeStore {
        snapshot: Mutex<HistoryDeletionSnapshot>,
        delete_calls: Mutex<u32>,
    }

    impl HistoryDeletionStore for FakeStore {
        fn preview(&self) -> Result<HistoryDeletionSnapshot, HistoryDeletionStoreError> {
            Ok(self.snapshot.lock().expect("lock snapshot").clone())
        }

        fn delete(
            &self,
            expected: &HistoryDeletionSnapshot,
        ) -> Result<HistoryDeletionSnapshot, HistoryDeletionStoreError> {
            *self.delete_calls.lock().expect("lock calls") += 1;
            Ok(expected.clone())
        }
    }

    #[test]
    fn requires_exact_confirmation_and_current_preview() {
        let store = Arc::new(FakeStore {
            snapshot: Mutex::new(snapshot(1, false)),
            delete_calls: Mutex::new(0),
        });
        let service = HistoryDeletionService::new(store.clone());
        let preview = service.preview().expect("preview");
        assert_eq!(
            service.delete(ConfirmedHistoryDeletion {
                preview_token: preview.preview_token.clone(),
                confirmation: "delete".to_owned()
            }),
            Err(HistoryDeletionError::ConfirmationRequired)
        );
        *store.snapshot.lock().expect("lock snapshot") = snapshot(2, false);
        assert_eq!(
            service.delete(ConfirmedHistoryDeletion {
                preview_token: preview.preview_token,
                confirmation: DELETE_CONFIRMATION.to_owned()
            }),
            Err(HistoryDeletionError::StalePreview)
        );
        assert_eq!(*store.delete_calls.lock().expect("lock calls"), 0);
    }

    #[test]
    fn active_refresh_blocks_deletion() {
        let store = Arc::new(FakeStore {
            snapshot: Mutex::new(snapshot(1, true)),
            delete_calls: Mutex::new(0),
        });
        let service = HistoryDeletionService::new(store.clone());
        let preview = service.preview().expect("preview");
        assert!(!preview.can_delete);
        assert_eq!(
            service.delete(ConfirmedHistoryDeletion {
                preview_token: preview.preview_token,
                confirmation: DELETE_CONFIRMATION.to_owned()
            }),
            Err(HistoryDeletionError::ActiveRefresh)
        );
    }

    fn snapshot(rows: u64, active_refresh: bool) -> HistoryDeletionSnapshot {
        HistoryDeletionSnapshot {
            daily_usage: rows,
            daily_model_usage: 0,
            sessions: 0,
            session_model_usage: 0,
            refresh_runs: 0,
            import_runs: 0,
            projects: 0,
            source_models: 0,
            notification_records: 0,
            source_count: 1,
            earliest_date: Some("2026-06-01".to_owned()),
            latest_date: Some("2026-06-01".to_owned()),
            active_refresh,
        }
    }
}
