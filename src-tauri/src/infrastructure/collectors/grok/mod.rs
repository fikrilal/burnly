//! Grok Build collector infrastructure.
#![allow(
    dead_code,
    unused_imports,
    reason = "Grok collector is not wired into RoutedCollector until chunk 05"
)]

mod adapter;
mod detection;
mod grok_home;
mod mapper;
mod model_resolver;
mod session_index;
mod unified_log_reader;
mod usage_cache;

#[cfg(test)]
mod tests;

pub(crate) use adapter::GrokCollector;
pub(crate) use detection::{inspect_grok_home, GrokHomeInspection};
pub(crate) use grok_home::{default_grok_home, resolve_grok_home, unified_log_path};
pub(crate) use mapper::{
    dedupe_inferences, map_daily, map_inferences, map_sessions, GrokMappedInference,
    GrokMappingContext, GrokMappingError,
};
pub(crate) use model_resolver::{GrokModelResolver, ModelResolverError};
pub(crate) use session_index::{GrokSessionIndex, GrokSessionSummary, SessionIndexError};
pub(crate) use unified_log_reader::{
    GrokInferenceUsage, UnifiedLogFileMetadata, UnifiedLogReadError, UnifiedLogReadSummary,
    UnifiedLogReader,
};
pub(crate) use usage_cache::{GrokIngestReport, GrokUsageCacheClient, NoOpGrokUsageCache};
