mod adapter;
mod conversation_index;
mod discovery;
pub(crate) mod product_variant;

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
