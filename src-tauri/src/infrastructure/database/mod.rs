//! SQLite infrastructure module exports.

mod bootstrap_store;
mod connection;
mod diagnostics_store;
mod error;
mod migrations;
mod reconciliation_store;
mod settings_store;
#[cfg(test)]
mod test_database;
mod tray_summary_store;

pub(crate) use bootstrap_store::SqliteBootstrapStore;
pub use connection::Database;
pub(crate) use diagnostics_store::SqliteDiagnosticStore;
pub use error::{PersistenceError, PersistenceErrorKind};
pub(crate) use reconciliation_store::SqliteReconciliationStore;
pub(crate) use settings_store::SqliteSettingsStore;
pub(crate) use tray_summary_store::SqliteTraySummaryStore;
