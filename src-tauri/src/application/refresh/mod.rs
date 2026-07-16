//! The refresh coordinator: single owner of refresh concurrency.

mod coordinator;
mod execution;
mod outcome;
mod planner;
mod request_plan;
mod scheduler;
mod state;
mod target;

#[cfg(test)]
mod tests;

#[allow(
    unused_imports,
    reason = "refresh exposes the coordinator and state to bootstrap wiring and IPC"
)]
pub(crate) use coordinator::{
    CommittedDailyUploadSink, NoopCommittedDailyUploadSink, RefreshCoordinator,
    RefreshCoordinatorHooks, RefreshEventSink,
};
#[allow(
    unused_imports,
    reason = "planner is introduced before coordinator wiring in the refresh-policy implementation series"
)]
pub(crate) use planner::{
    RefreshPlan, RefreshPlanMode, RefreshPlanRequest, RefreshPlanTarget, RefreshPolicyPlanner,
};
pub(crate) use scheduler::{RefreshPolicy, RefreshScheduler, RefreshSchedulerError};
#[allow(
    unused_imports,
    reason = "refresh exposes the coordinator and state to bootstrap wiring and IPC"
)]
pub(crate) use state::{RefreshSnapshot, RefreshStatus};
