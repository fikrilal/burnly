#![allow(
    dead_code,
    reason = "Cline parser modules are introduced before the collector adapter wires runtime calls"
)]

mod messages;
mod schema;
mod store;

#[allow(
    unused_imports,
    reason = "Cline parser surface is wired in later chunks"
)]
pub(crate) use messages::{decode_messages, ClineMessageUsage, ClineUsageMetrics};
#[allow(
    unused_imports,
    reason = "Cline parser surface is wired in later chunks"
)]
pub(crate) use store::{ClineSessionRow, ClineStore};
