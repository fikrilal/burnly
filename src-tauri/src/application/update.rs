use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UpdateSnapshot {
    pub status: UpdateStatus,
    pub available_version: Option<String>,
    pub downloaded_version: Option<String>,
    pub last_checked_at_ms: Option<i64>,
    pub error: Option<UpdateErrorSummary>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UpdateStatus {
    #[allow(
        dead_code,
        reason = "unavailable status is used by deterministic test/runtime fakes"
    )]
    Unavailable,
    Idle,
    Checking,
    Available,
    Downloading,
    Ready,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UpdateErrorSummary {
    pub code: &'static str,
    pub retryable: bool,
}

pub(crate) type UpdateRuntimeFuture<'a> =
    Pin<Box<dyn Future<Output = Result<UpdateSnapshot, UpdateRuntimeError>> + Send + 'a>>;

pub(crate) trait UpdateRuntime: Send + Sync {
    fn status(&self) -> UpdateSnapshot;
    fn check(self: Arc<Self>) -> UpdateRuntimeFuture<'static>;
    fn download(self: Arc<Self>) -> UpdateRuntimeFuture<'static>;
    fn restart(self: Arc<Self>) -> UpdateRuntimeFuture<'static>;
}

pub(crate) struct UpdateService {
    runtime: Arc<dyn UpdateRuntime>,
}

impl UpdateService {
    pub(crate) fn new(runtime: Arc<dyn UpdateRuntime>) -> Self {
        Self { runtime }
    }

    pub(crate) fn status(&self) -> UpdateSnapshot {
        self.runtime.status()
    }

    pub(crate) async fn check(&self) -> Result<UpdateSnapshot, UpdateRuntimeError> {
        self.runtime.clone().check().await
    }

    pub(crate) async fn download(&self) -> Result<UpdateSnapshot, UpdateRuntimeError> {
        self.runtime.clone().download().await
    }

    pub(crate) async fn restart(&self) -> Result<UpdateSnapshot, UpdateRuntimeError> {
        self.runtime.clone().restart().await
    }
}

#[derive(Debug, Clone, Copy, Error)]
pub(crate) enum UpdateRuntimeError {
    #[error("updates are unavailable")]
    Unavailable,
    #[error("update operation cannot run in the current state")]
    InvalidState,
    #[error("update network operation failed")]
    Network,
    #[error("update signature verification failed")]
    Signature,
    #[error("update installation failed")]
    Install,
    #[error("update runtime failed")]
    Internal,
}

impl UpdateRuntimeError {
    pub(crate) const fn code(&self) -> &'static str {
        match self {
            Self::Unavailable => "update.unavailable",
            Self::InvalidState => "update.invalid_state",
            Self::Network => "update.network_failed",
            Self::Signature => "update.signature_failed",
            Self::Install => "update.install_failed",
            Self::Internal => "update.internal",
        }
    }

    pub(crate) const fn retryable(&self) -> bool {
        match self {
            Self::Unavailable | Self::InvalidState | Self::Signature => false,
            Self::Network | Self::Install | Self::Internal => true,
        }
    }
}

pub(crate) fn update_status_label(value: UpdateStatus) -> &'static str {
    match value {
        UpdateStatus::Unavailable => "unavailable",
        UpdateStatus::Idle => "idle",
        UpdateStatus::Checking => "checking",
        UpdateStatus::Available => "available",
        UpdateStatus::Downloading => "downloading",
        UpdateStatus::Ready => "ready",
        UpdateStatus::Failed => "failed",
    }
}

/// Deterministic update runtime for platforms (and tests) where Burnly does not
/// ship auto-update support, e.g. the macOS `.dmg` preview.
pub(crate) struct UnavailableUpdateRuntime {
    snapshot: Mutex<UpdateSnapshot>,
}

impl UnavailableUpdateRuntime {
    pub(crate) fn new() -> Self {
        Self {
            snapshot: Mutex::new(UpdateSnapshot {
                status: UpdateStatus::Unavailable,
                available_version: None,
                downloaded_version: None,
                last_checked_at_ms: None,
                error: Some(UpdateErrorSummary {
                    code: UpdateRuntimeError::Unavailable.code(),
                    retryable: UpdateRuntimeError::Unavailable.retryable(),
                }),
            }),
        }
    }
}

impl UpdateRuntime for UnavailableUpdateRuntime {
    fn status(&self) -> UpdateSnapshot {
        self.snapshot
            .lock()
            .expect("update snapshot lock is poisoned")
            .clone()
    }

    fn check(self: Arc<Self>) -> UpdateRuntimeFuture<'static> {
        Box::pin(async { Err(UpdateRuntimeError::Unavailable) })
    }

    fn download(self: Arc<Self>) -> UpdateRuntimeFuture<'static> {
        Box::pin(async { Err(UpdateRuntimeError::Unavailable) })
    }

    fn restart(self: Arc<Self>) -> UpdateRuntimeFuture<'static> {
        Box::pin(async { Err(UpdateRuntimeError::Unavailable) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::Future;
    use std::sync::Arc;
    use std::task::{Context, Poll, Wake, Waker};

    #[test]
    fn unavailable_runtime_reports_stable_snapshot_and_rejects_commands() {
        let service = UpdateService::new(Arc::new(UnavailableUpdateRuntime::new()));

        let snapshot = service.status();

        assert_eq!(snapshot.status, UpdateStatus::Unavailable);
        assert_eq!(
            snapshot.error,
            Some(UpdateErrorSummary {
                code: "update.unavailable",
                retryable: false,
            })
        );
        assert!(matches!(
            poll_ready(service.check()),
            Err(UpdateRuntimeError::Unavailable)
        ));
        assert!(matches!(
            poll_ready(service.download()),
            Err(UpdateRuntimeError::Unavailable)
        ));
        assert!(matches!(
            poll_ready(service.restart()),
            Err(UpdateRuntimeError::Unavailable)
        ));
    }

    fn poll_ready<T>(future: impl Future<Output = T>) -> T {
        let waker = Waker::from(Arc::new(NoopWake));
        let mut context = Context::from_waker(&waker);
        let mut future = Box::pin(future);

        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("test future should complete without awaiting runtime work"),
        }
    }

    struct NoopWake;

    impl Wake for NoopWake {
        fn wake(self: Arc<Self>) {}
    }
}
