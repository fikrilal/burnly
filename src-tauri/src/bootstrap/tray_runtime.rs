use std::sync::{Arc, Mutex};

use tauri::{Listener, Manager, Runtime};

use crate::application::reconciliation::RefreshTrigger;
use crate::application::refresh::{
    RefreshCoordinator, RefreshEventSink, RefreshSnapshot, RefreshStatus,
};
use crate::application::usage::TraySummaryQuery;
use crate::ipc::refresh_event_sink;
use crate::platform::system_clock::SystemClock;
use crate::platform::{system_clock, system_timezone, tray};

const TRAY_OPEN_STALE_AFTER_MS: i64 = 5 * 60 * 1_000;
const TRAY_OPEN_REFRESH_THROTTLE_MS: i64 = 60 * 1_000;

struct RuntimeRefreshEventSink<R: Runtime> {
    frontend: Arc<dyn RefreshEventSink>,
    tray: Arc<Mutex<Option<tray::TrayController<R>>>>,
    tray_summary: TraySummaryQuery,
}

impl<R: Runtime> RefreshEventSink for RuntimeRefreshEventSink<R> {
    fn publish(&self, snapshot: RefreshSnapshot, usage_changed: bool) {
        self.frontend.publish(snapshot.clone(), usage_changed);
        if let Some(tray) = self
            .tray
            .lock()
            .expect("tray state lock is poisoned")
            .as_ref()
        {
            let timezone = system_timezone::resolve().unwrap_or_else(|_| "UTC".to_owned());
            tray.update(&tray_snapshot(&snapshot, &self.tray_summary, &timezone));
        }
    }
}

pub(super) fn runtime_refresh_event_sink<R: Runtime>(
    app: tauri::AppHandle<R>,
    tray: Arc<Mutex<Option<tray::TrayController<R>>>>,
    tray_summary: TraySummaryQuery,
) -> Arc<dyn RefreshEventSink> {
    Arc::new(RuntimeRefreshEventSink {
        frontend: refresh_event_sink(app),
        tray,
        tray_summary,
    })
}

pub(super) fn install_tray_invalidation_listener<R: Runtime>(
    app: tauri::AppHandle<R>,
    tray_summary: TraySummaryQuery,
) {
    let listener_app = app.clone();
    app.listen("burnly://v1/data-invalidated", move |_| {
        if let (Some(controller), Some(coordinator)) = (
            listener_app.try_state::<tray::TrayController<R>>(),
            listener_app.try_state::<RefreshCoordinator>(),
        ) {
            let timezone = system_timezone::resolve().unwrap_or_else(|_| "UTC".to_owned());
            controller.update(&tray_snapshot(
                &coordinator.snapshot(),
                &tray_summary,
                &timezone,
            ));
        }
    });
}

pub(super) fn tray_snapshot(
    snapshot: &RefreshSnapshot,
    tray_summary: &TraySummaryQuery,
    reporting_timezone: &str,
) -> tray::TraySnapshot {
    let summary = tray_summary.get(reporting_timezone).ok();
    let today_tokens = summary.as_ref().map(|s| s.today.total_tokens);
    let week_tokens = summary.as_ref().map(|s| s.week.total_tokens);
    let month_tokens = summary.as_ref().map(|s| s.month.total_tokens);

    tray::TraySnapshot {
        status: tray_refresh_status(snapshot.status),
        last_successful_refresh_at_ms: snapshot.last_successful_refresh_at_ms,
        budget_summary: None,
        today_tokens,
        week_tokens,
        month_tokens,
    }
}

const fn tray_refresh_status(status: RefreshStatus) -> tray::TrayRefreshStatus {
    match status {
        RefreshStatus::Idle => tray::TrayRefreshStatus::Idle,
        RefreshStatus::Queued => tray::TrayRefreshStatus::Queued,
        RefreshStatus::Running => tray::TrayRefreshStatus::Running,
        RefreshStatus::Cancelling => tray::TrayRefreshStatus::Cancelling,
        RefreshStatus::Succeeded => tray::TrayRefreshStatus::Succeeded,
        RefreshStatus::Partial => tray::TrayRefreshStatus::Partial,
        RefreshStatus::Failed => tray::TrayRefreshStatus::Failed,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TrayOpenRefreshDecision {
    Request,
    SkipActive,
    SkipFresh,
    SkipThrottled,
    SkipClock,
    SkipReadFailure,
}

pub(super) trait TrayOpenClock: Send + Sync {
    fn now_epoch_ms(&self) -> Option<i64>;
}

impl TrayOpenClock for SystemClock {
    fn now_epoch_ms(&self) -> Option<i64> {
        system_clock::now_epoch_ms().ok()
    }
}

pub(super) struct TrayOpenRefreshController {
    reporting_timezone: String,
    summary_query: TraySummaryQuery,
    coordinator: RefreshCoordinator,
    clock: Arc<dyn TrayOpenClock>,
    last_request_at_ms: Mutex<Option<i64>>,
}

impl TrayOpenRefreshController {
    pub(super) fn new(
        reporting_timezone: String,
        summary_query: TraySummaryQuery,
        coordinator: RefreshCoordinator,
        clock: Arc<dyn TrayOpenClock>,
    ) -> Self {
        Self {
            reporting_timezone,
            summary_query,
            coordinator,
            clock,
            last_request_at_ms: Mutex::new(None),
        }
    }

    pub(super) fn request_startup_refresh_if_stale(&self) -> TrayOpenRefreshDecision {
        self.request_if_stale(RefreshTrigger::Launch)
    }

    pub(super) fn request_tray_open_refresh_if_stale(&self) -> TrayOpenRefreshDecision {
        self.request_if_stale(RefreshTrigger::Manual)
    }

    fn request_if_stale(&self, trigger: RefreshTrigger) -> TrayOpenRefreshDecision {
        let now_ms = match self.clock.now_epoch_ms() {
            Some(value) => value,
            None => return TrayOpenRefreshDecision::SkipClock,
        };
        let last_successful_refresh_at_ms = match self.summary_query.get(&self.reporting_timezone) {
            Ok(summary) => summary.last_successful_refresh_at_ms,
            Err(_) => return TrayOpenRefreshDecision::SkipReadFailure,
        };
        let snapshot = self.coordinator.snapshot();
        let mut last_request = self
            .last_request_at_ms
            .lock()
            .expect("tray open refresh lock is poisoned");
        let decision = tray_open_refresh_decision(
            now_ms,
            last_successful_refresh_at_ms,
            *last_request,
            snapshot.status.is_active(),
        );
        if decision == TrayOpenRefreshDecision::Request {
            *last_request = Some(now_ms);
            if matches!(trigger, RefreshTrigger::Manual) {
                self.coordinator.request_freshness_refresh(trigger);
            } else {
                self.coordinator.request_refresh(trigger);
            }
        }
        decision
    }
}

fn tray_open_refresh_decision(
    now_ms: i64,
    last_successful_refresh_at_ms: Option<i64>,
    last_request_at_ms: Option<i64>,
    refresh_active: bool,
) -> TrayOpenRefreshDecision {
    if refresh_active {
        return TrayOpenRefreshDecision::SkipActive;
    }
    if let Some(last_request_at_ms) = last_request_at_ms {
        if now_ms.saturating_sub(last_request_at_ms) < TRAY_OPEN_REFRESH_THROTTLE_MS {
            return TrayOpenRefreshDecision::SkipThrottled;
        }
    }
    if let Some(last_successful_refresh_at_ms) = last_successful_refresh_at_ms {
        if now_ms.saturating_sub(last_successful_refresh_at_ms) < TRAY_OPEN_STALE_AFTER_MS {
            return TrayOpenRefreshDecision::SkipFresh;
        }
    }
    TrayOpenRefreshDecision::Request
}

#[cfg(test)]
mod tests {
    use super::{tray_open_refresh_decision, TrayOpenRefreshDecision};

    #[test]
    fn tray_open_refresh_requests_only_when_stale_and_not_throttled() {
        assert_eq!(
            tray_open_refresh_decision(600_000, None, None, false),
            TrayOpenRefreshDecision::Request
        );
        assert_eq!(
            tray_open_refresh_decision(600_000, Some(550_001), None, false),
            TrayOpenRefreshDecision::SkipFresh
        );
        assert_eq!(
            tray_open_refresh_decision(600_000, Some(200_000), Some(590_001), false),
            TrayOpenRefreshDecision::SkipThrottled
        );
        assert_eq!(
            tray_open_refresh_decision(600_000, Some(200_000), Some(500_000), false),
            TrayOpenRefreshDecision::Request
        );
        assert_eq!(
            tray_open_refresh_decision(600_000, Some(200_000), None, true),
            TrayOpenRefreshDecision::SkipActive
        );
    }
}
