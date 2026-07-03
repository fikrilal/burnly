//! Technical adapters for persistence, collectors, and external systems.
//!
//! Infrastructure implements application-owned contracts and keeps external
//! representations inside the adapter that owns them.

pub(crate) mod bootstrap_store;
pub(crate) mod collectors;
pub mod database;
pub(crate) mod diagnostics_store;
pub(crate) mod project_identity;
pub(crate) mod settings_store;
