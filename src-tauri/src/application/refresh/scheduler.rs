//! Background refresh scheduling.

use std::convert::TryFrom;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use thiserror::Error;

use crate::application::reconciliation::RefreshTrigger;

use super::RefreshCoordinator;

pub(crate) trait ScheduledRefreshRequester: Send + Sync {
    fn request_scheduled_refresh(&self);
}

impl ScheduledRefreshRequester for RefreshCoordinator {
    fn request_scheduled_refresh(&self) {
        self.request_refresh(RefreshTrigger::Scheduled);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RefreshPolicy {
    interval: Option<Duration>,
}

impl RefreshPolicy {
    pub(crate) const fn disabled() -> Self {
        Self { interval: None }
    }

    pub(crate) fn enabled_minutes(minutes: i64) -> Self {
        let seconds = u64::try_from(minutes)
            .ok()
            .and_then(|value| value.checked_mul(60));
        match seconds {
            Some(0) | None => Self::disabled(),
            Some(seconds) => Self {
                interval: Some(Duration::from_secs(seconds)),
            },
        }
    }

    const fn interval(self) -> Option<Duration> {
        self.interval
    }
}

#[derive(Debug, Error)]
#[error("failed to start refresh scheduler")]
pub(crate) struct RefreshSchedulerError;

impl RefreshSchedulerError {
    fn thread_spawn() -> Self {
        Self
    }
}

pub(crate) struct RefreshScheduler {
    control: Arc<SchedulerControl>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl RefreshScheduler {
    pub(crate) fn start(
        policy: RefreshPolicy,
        requester: Arc<dyn ScheduledRefreshRequester>,
    ) -> Result<Self, RefreshSchedulerError> {
        let control = Arc::new(SchedulerControl::new(policy));
        let worker_control = control.clone();
        let worker = thread::Builder::new()
            .name("burnly-refresh-scheduler".to_owned())
            .spawn(move || run_scheduler(worker_control, requester))
            .map_err(|_| RefreshSchedulerError::thread_spawn())?;

        Ok(Self {
            control,
            worker: Mutex::new(Some(worker)),
        })
    }
}

impl Drop for RefreshScheduler {
    fn drop(&mut self) {
        self.control.stop();
        let worker = self
            .worker
            .lock()
            .expect("scheduler lock is poisoned")
            .take();
        if let Some(worker) = worker {
            let _ = worker.join();
        }
    }
}

struct SchedulerControl {
    state: Mutex<SchedulerState>,
    changed: Condvar,
}

impl SchedulerControl {
    fn new(policy: RefreshPolicy) -> Self {
        Self {
            state: Mutex::new(SchedulerState {
                policy,
                stopped: false,
            }),
            changed: Condvar::new(),
        }
    }

    fn stop(&self) {
        let mut state = self.state.lock().expect("scheduler state is poisoned");
        state.stopped = true;
        self.changed.notify_all();
    }
}

struct SchedulerState {
    policy: RefreshPolicy,
    stopped: bool,
}

fn run_scheduler(control: Arc<SchedulerControl>, requester: Arc<dyn ScheduledRefreshRequester>) {
    loop {
        match wait_for_interval(&control) {
            WaitResult::Stopped => return,
            WaitResult::Changed => continue,
            WaitResult::Elapsed => requester.request_scheduled_refresh(),
        }
    }
}

fn wait_for_interval(control: &SchedulerControl) -> WaitResult {
    let mut state = control.state.lock().expect("scheduler state is poisoned");
    loop {
        if state.stopped {
            return WaitResult::Stopped;
        }
        let Some(interval) = state.policy.interval() else {
            state = control.changed.wait(state).expect("scheduler wait failed");
            continue;
        };
        let result = control
            .changed
            .wait_timeout(state, interval)
            .expect("scheduler wait failed");
        state = result.0;
        if state.stopped {
            return WaitResult::Stopped;
        }
        if state.policy.interval() != Some(interval) {
            return WaitResult::Changed;
        }
        if result.1.timed_out() {
            return WaitResult::Elapsed;
        }
    }
}

enum WaitResult {
    Stopped,
    Changed,
    Elapsed,
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    struct RecordingRequester {
        requests: AtomicUsize,
    }

    impl RecordingRequester {
        fn new() -> Self {
            Self {
                requests: AtomicUsize::new(0),
            }
        }

        fn requests(&self) -> usize {
            self.requests.load(Ordering::Acquire)
        }

        fn wait_for_request(&self) {
            for _ in 0..200 {
                if self.requests() > 0 {
                    return;
                }
                thread::sleep(Duration::from_millis(1));
            }
            panic!("scheduler did not submit a refresh request");
        }
    }

    impl ScheduledRefreshRequester for RecordingRequester {
        fn request_scheduled_refresh(&self) {
            self.requests.fetch_add(1, Ordering::AcqRel);
        }
    }

    fn enabled_for_test() -> RefreshPolicy {
        RefreshPolicy {
            interval: Some(Duration::from_millis(2)),
        }
    }

    #[test]
    fn disabled_policy_does_not_submit_refresh() {
        let requester = Arc::new(RecordingRequester::new());
        let scheduler = RefreshScheduler::start(RefreshPolicy::disabled(), requester.clone())
            .expect("start scheduler");

        thread::sleep(Duration::from_millis(20));

        assert_eq!(requester.requests(), 0);
        drop(scheduler);
    }

    #[test]
    fn enabled_policy_submits_scheduled_refresh() {
        let requester = Arc::new(RecordingRequester::new());
        let scheduler = RefreshScheduler::start(enabled_for_test(), requester.clone())
            .expect("start scheduler");

        requester.wait_for_request();

        assert!(requester.requests() > 0);
        drop(scheduler);
    }

    #[test]
    fn invalid_enabled_interval_is_disabled() {
        assert_eq!(RefreshPolicy::enabled_minutes(0), RefreshPolicy::disabled());
        assert_eq!(
            RefreshPolicy::enabled_minutes(-1),
            RefreshPolicy::disabled()
        );
    }
}
