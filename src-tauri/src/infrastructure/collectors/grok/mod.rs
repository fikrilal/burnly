//! Grok Build collector infrastructure.
#![allow(
    dead_code,
    reason = "Grok adapter and runtime wiring consume this reader surface in later chunks"
)]

mod detection;
mod grok_home;
mod session_index;
mod unified_log_reader;

#[cfg(test)]
mod tests;

#[allow(
    unused_imports,
    reason = "Grok detection surface is used by the adapter chunk"
)]
pub(crate) use detection::{inspect_grok_home, GrokHomeInspection};
#[allow(
    unused_imports,
    reason = "Grok home resolution is used by the adapter chunk"
)]
pub(crate) use grok_home::{default_grok_home, resolve_grok_home, unified_log_path};
#[allow(
    unused_imports,
    reason = "Grok session index is used by the adapter chunk"
)]
pub(crate) use session_index::{GrokSessionIndex, GrokSessionSummary, SessionIndexError};
#[allow(
    unused_imports,
    reason = "Grok unified log reader is used by the adapter chunk"
)]
pub(crate) use unified_log_reader::{
    GrokInferenceUsage, UnifiedLogReadError, UnifiedLogReadSummary, UnifiedLogReader,
};
