mod adapter;
mod mapper;
mod messages;
mod schema;
mod store;

#[allow(
    unused_imports,
    reason = "Cline collector is wired in the runtime chunk"
)]
pub(crate) use adapter::ClineCollector;
#[allow(
    unused_imports,
    reason = "Cline parser surface is used by adapter tests"
)]
pub(crate) use messages::{decode_messages, ClineMessageUsage, ClineUsageMetrics};
#[allow(
    unused_imports,
    reason = "Cline parser surface is used by adapter tests"
)]
pub(crate) use store::{ClineSessionRow, ClineStore};
