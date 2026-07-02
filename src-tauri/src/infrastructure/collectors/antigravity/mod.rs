mod adapter;
mod conversation_index;
mod discovery;
pub(crate) mod product_variant;
mod runtime_client;
mod usage_extractor;

#[allow(
    unused_imports,
    reason = "Antigravity collector is wired before runtime discovery is implemented"
)]
pub(crate) use adapter::AntigravityCollector;
#[allow(
    unused_imports,
    reason = "Antigravity conversation index is wired into collection in a later chunk"
)]
pub(crate) use conversation_index::{ConversationDatabase, ConversationIndex};
#[allow(
    unused_imports,
    reason = "Antigravity runtime discovery is wired into collection in a later chunk"
)]
pub(crate) use discovery::{RuntimeDiscovery, RuntimeEndpoint};
#[allow(
    unused_imports,
    reason = "Antigravity runtime client is wired into collection in a later chunk"
)]
pub(crate) use runtime_client::{RuntimeClient, RuntimeClientError};
#[allow(
    unused_imports,
    reason = "Antigravity usage extraction is wired into collection in a later chunk"
)]
pub(crate) use usage_extractor::{extract_usage_records, AntigravityUsageRecord};
