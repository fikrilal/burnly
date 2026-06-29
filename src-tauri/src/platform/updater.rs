use std::sync::{Arc, Mutex};

use chrono::Utc;
use tauri::{AppHandle, Runtime};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::application::update::{
    UpdateErrorSummary, UpdateRuntime, UpdateRuntimeError, UpdateRuntimeFuture, UpdateSnapshot,
    UpdateStatus,
};

struct UpdateRuntimeState {
    snapshot: UpdateSnapshot,
    pending: Option<Update>,
    downloaded: Option<Vec<u8>>,
}

pub(crate) struct TauriUpdateRuntime<R: Runtime> {
    app: AppHandle<R>,
    state: Arc<Mutex<UpdateRuntimeState>>,
}

impl<R: Runtime> TauriUpdateRuntime<R> {
    pub(crate) fn new(app: AppHandle<R>) -> Self {
        Self {
            app,
            state: Arc::new(Mutex::new(UpdateRuntimeState {
                snapshot: UpdateSnapshot {
                    status: UpdateStatus::Idle,
                    available_version: None,
                    downloaded_version: None,
                    last_checked_at_ms: None,
                    error: None,
                },
                pending: None,
                downloaded: None,
            })),
        }
    }
}

impl<R: Runtime> UpdateRuntime for TauriUpdateRuntime<R> {
    fn status(&self) -> UpdateSnapshot {
        self.state
            .lock()
            .expect("update runtime state lock is poisoned")
            .snapshot
            .clone()
    }

    fn check(self: Arc<Self>) -> UpdateRuntimeFuture<'static> {
        Box::pin(async move {
            set_snapshot(
                &self.state,
                UpdateSnapshot {
                    status: UpdateStatus::Checking,
                    available_version: None,
                    downloaded_version: None,
                    last_checked_at_ms: None,
                    error: None,
                },
            );

            let checked_at = Utc::now().timestamp_millis();
            let result = self
                .app
                .updater()
                .map_err(map_updater_error)?
                .check()
                .await
                .map_err(map_updater_error);

            match result {
                Ok(Some(update)) => {
                    let version = update.version.clone();
                    let snapshot = UpdateSnapshot {
                        status: UpdateStatus::Available,
                        available_version: Some(version),
                        downloaded_version: None,
                        last_checked_at_ms: Some(checked_at),
                        error: None,
                    };
                    let mut state = self
                        .state
                        .lock()
                        .expect("update runtime state lock is poisoned");
                    state.pending = Some(update);
                    state.downloaded = None;
                    state.snapshot = snapshot.clone();
                    Ok(snapshot)
                }
                Ok(None) => {
                    let snapshot = UpdateSnapshot {
                        status: UpdateStatus::Idle,
                        available_version: None,
                        downloaded_version: None,
                        last_checked_at_ms: Some(checked_at),
                        error: None,
                    };
                    let mut state = self
                        .state
                        .lock()
                        .expect("update runtime state lock is poisoned");
                    state.pending = None;
                    state.downloaded = None;
                    state.snapshot = snapshot.clone();
                    Ok(snapshot)
                }
                Err(error) => {
                    let snapshot = failed_snapshot(error, Some(checked_at));
                    set_snapshot(&self.state, snapshot.clone());
                    Err(error)
                }
            }
        })
    }

    fn download(self: Arc<Self>) -> UpdateRuntimeFuture<'static> {
        Box::pin(async move {
            let update = {
                let state = self
                    .state
                    .lock()
                    .expect("update runtime state lock is poisoned");
                state
                    .pending
                    .clone()
                    .ok_or(UpdateRuntimeError::InvalidState)?
            };
            let version = update.version.clone();
            set_snapshot(
                &self.state,
                UpdateSnapshot {
                    status: UpdateStatus::Downloading,
                    available_version: Some(version.clone()),
                    downloaded_version: None,
                    last_checked_at_ms: self.status().last_checked_at_ms,
                    error: None,
                },
            );

            match update
                .download(|_, _| {}, || {})
                .await
                .map_err(map_updater_error)
            {
                Ok(bytes) => {
                    let snapshot = UpdateSnapshot {
                        status: UpdateStatus::Ready,
                        available_version: Some(version.clone()),
                        downloaded_version: Some(version),
                        last_checked_at_ms: self.status().last_checked_at_ms,
                        error: None,
                    };
                    let mut state = self
                        .state
                        .lock()
                        .expect("update runtime state lock is poisoned");
                    state.downloaded = Some(bytes);
                    state.snapshot = snapshot.clone();
                    Ok(snapshot)
                }
                Err(error) => {
                    let snapshot = failed_snapshot(error, self.status().last_checked_at_ms);
                    set_snapshot(&self.state, snapshot.clone());
                    Err(error)
                }
            }
        })
    }

    fn restart(self: Arc<Self>) -> UpdateRuntimeFuture<'static> {
        Box::pin(async move {
            let (update, bytes) = {
                let mut state = self
                    .state
                    .lock()
                    .expect("update runtime state lock is poisoned");
                let update = state
                    .pending
                    .clone()
                    .ok_or(UpdateRuntimeError::InvalidState)?;
                let bytes = state
                    .downloaded
                    .take()
                    .ok_or(UpdateRuntimeError::InvalidState)?;
                (update, bytes)
            };

            update.install(bytes).map_err(map_updater_error)?;
            self.app.request_restart();

            Ok(UpdateSnapshot {
                status: UpdateStatus::Ready,
                available_version: Some(update.version.clone()),
                downloaded_version: Some(update.version),
                last_checked_at_ms: self.status().last_checked_at_ms,
                error: None,
            })
        })
    }
}

fn set_snapshot(state: &Arc<Mutex<UpdateRuntimeState>>, snapshot: UpdateSnapshot) {
    state
        .lock()
        .expect("update runtime state lock is poisoned")
        .snapshot = snapshot;
}

fn failed_snapshot(error: UpdateRuntimeError, last_checked_at_ms: Option<i64>) -> UpdateSnapshot {
    UpdateSnapshot {
        status: UpdateStatus::Failed,
        available_version: None,
        downloaded_version: None,
        last_checked_at_ms,
        error: Some(UpdateErrorSummary {
            code: error.code(),
            retryable: error.retryable(),
        }),
    }
}

fn map_updater_error(error: tauri_plugin_updater::Error) -> UpdateRuntimeError {
    match error {
        tauri_plugin_updater::Error::EmptyEndpoints => UpdateRuntimeError::Unavailable,
        tauri_plugin_updater::Error::Reqwest(_)
        | tauri_plugin_updater::Error::ReleaseNotFound
        | tauri_plugin_updater::Error::Network(_) => UpdateRuntimeError::Network,
        tauri_plugin_updater::Error::Minisign(_)
        | tauri_plugin_updater::Error::Base64(_)
        | tauri_plugin_updater::Error::SignatureUtf8(_) => UpdateRuntimeError::Signature,
        tauri_plugin_updater::Error::AuthenticationFailed
        | tauri_plugin_updater::Error::DebInstallFailed
        | tauri_plugin_updater::Error::PackageInstallFailed
        | tauri_plugin_updater::Error::InvalidUpdaterFormat => UpdateRuntimeError::Install,
        _ => UpdateRuntimeError::Internal,
    }
}
