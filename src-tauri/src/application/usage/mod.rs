//! Purpose-built usage queries and application-owned read models.

mod tray_summary;

pub(crate) use tray_summary::{
    OverviewDataStatus, PersistedRefreshStatus, TraySummaryDataQuality, TraySummaryModelRow,
    TraySummaryPeriodMetric, TraySummaryQuery, TraySummaryQueryError, TraySummaryReadModel,
    TraySummaryScope, TraySummaryStoreModelUsage, TraySummaryStoreResult, TraySummaryTrend,
    TraySummaryTrendDirection,
};
