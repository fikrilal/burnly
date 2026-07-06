//! SQLite infrastructure module exports.

mod antigravity_cache_store;
mod bootstrap_store;
mod connection;
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
pub use connection::Database;
pub(crate) use diagnostics_store::SqliteDiagnosticStore;
pub use error::{PersistenceError, PersistenceErrorKind};
pub(crate) use grok_cache_store::SqliteGrokUsageCacheStore;
pub(crate) use reconciliation::SqliteReconciliationStore;
pub(crate) use settings_store::SqliteSettingsStore;
pub(crate) use tray_summary_store::SqliteTraySummaryStore;
