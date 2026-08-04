//! Command Code collector infrastructure.
//!
//! Phase 1: source identity and detection only. The adapter fails closed on
//! collection until a later chunk wires the transcript reader and mapper.

mod adapter;
mod commandcode_home;
mod detection;

#[allow(
    unused_imports,
    reason = "adapter is wired into routing in a later chunk"
)]
pub(crate) use adapter::CommandCodeCollector;
#[allow(
    unused_imports,
    reason = "data-root resolution is consumed by a later chunk"
)]
pub(crate) use commandcode_home::{default_commandcode_home, resolve_commandcode_home};
