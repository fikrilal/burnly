//! Burnly use cases, orchestration, and application-owned ports.
//!
//! Application modules may depend on the domain, but not on delivery or
//! infrastructure implementations.

pub(crate) mod account;
pub(crate) mod auth_loopback;
pub(crate) mod bootstrap;
pub(crate) mod cloud_session;
pub(crate) mod pkce;
pub(crate) mod collection;
pub(crate) mod diagnostics;
pub(crate) mod ports;
pub(crate) mod reconciliation;
pub(crate) mod refresh;
pub(crate) mod settings;
pub(crate) mod update;
pub(crate) mod usage;
