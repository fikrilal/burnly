//! SQLite infrastructure module exports.

mod antigravity_cache_store;
mod bootstrap_store;
mod collect_sync_store;
mod connection;
mod daily_usage_export_store;
mod diagnostics_store;
mod error;
mod grok_cache_store;
mod migrations;
mod reconciliation;
mod settings_store;
#[cfg(test)]
mod test_database;
mod tray_summary_store;

pub(crate) use antigravity_cache_store::SqliteAntigravityUsageCacheStore;
pub(crate) use bootstrap_store::SqliteBootstrapStore;
#[allow(unused_imports)] // constructed by later collect-sync composition
pub(crate) use collect_sync_store::SqliteCollectSyncStore;
pub use connection::Database;
#[allow(unused_imports)]
pub(crate) use daily_usage_export_store::SqliteDailyUsageExportStore;
pub(crate) use diagnostics_store::SqliteDiagnosticStore;
pub use error::{PersistenceError, PersistenceErrorKind};
pub(crate) use grok_cache_store::SqliteGrokUsageCacheStore;
pub(crate) use reconciliation::SqliteReconciliationStore;
pub(crate) use settings_store::SqliteSettingsStore;
pub(crate) use tray_summary_store::SqliteTraySummaryStore;
