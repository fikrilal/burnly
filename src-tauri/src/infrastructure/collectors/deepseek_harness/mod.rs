//! DeepSeek Harness collector infrastructure.
//!
//! Phases 1-3 provide source identity, detection, bounded session-log reading,
//! event parsing, and usage folding. The adapter still fails closed until a
//! later chunk wires the mapper and refresh integration.

mod adapter;
mod deepseek_home;
mod detection;
mod discovery;
mod event_parser;
mod session_log_reader;
mod usage_fold;

#[allow(
    unused_imports,
    reason = "adapter is wired into routing in a later chunk"
)]
pub(crate) use adapter::DeepSeekHarnessCollector;
#[allow(
    unused_imports,
    reason = "data-root resolution is consumed by a later chunk"
)]
pub(crate) use deepseek_home::{default_deepseek_harness_home, resolve_deepseek_harness_home};
