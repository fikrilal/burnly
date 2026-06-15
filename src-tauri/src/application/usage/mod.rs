//! Purpose-built usage queries and application-owned read models.

#![allow(
    dead_code,
    reason = "Phase 5A read models are wired through IPC in Phase 5B"
)]

mod overview;

#[allow(
    unused_imports,
    reason = "Phase 5A exports the complete read boundary for Phase 5B"
)]
pub(crate) use overview::{
    CostCompleteness, OverviewCost, OverviewDataStatus, OverviewPeriod, OverviewQuery,
    OverviewQueryError, OverviewReadModel, OverviewSource, OverviewStoreResult,
    PersistedRefreshStatus,
};
