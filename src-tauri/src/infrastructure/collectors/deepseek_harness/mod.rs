//! DeepSeek Harness collector infrastructure.
//!
//! Phase 1: source identity and detection only. The adapter fails closed on
//! collection until a later chunk wires the session-log reader, usage fold,
//! and mapper.

mod adapter;
mod deepseek_home;
mod detection;

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
