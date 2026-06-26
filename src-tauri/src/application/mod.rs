//! Burnly use cases, orchestration, and application-owned ports.
//!
//! Application modules may depend on the domain, but not on delivery or
//! infrastructure implementations.

pub(crate) mod bootstrap;
pub(crate) mod collection;
pub(crate) mod export;
pub(crate) mod history;
pub(crate) mod history_deletion;
pub(crate) mod ports;
pub(crate) mod reconciliation;
pub(crate) mod refresh;
pub(crate) mod settings;
pub(crate) mod usage;
