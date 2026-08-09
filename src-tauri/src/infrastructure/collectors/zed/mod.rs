//! Zed agent collector infrastructure.
//!
//! Chunk 1-2: source identity, thread store, and mapper. The collector is not
//! wired into routing yet; a later chunk adds the adapter and bootstrap.

mod mapper;
mod threads_store;

#[allow(
    unused_imports,
    reason = "store and mapper are consumed by the adapter in a later chunk"
)]
pub(crate) use mapper::{map_sessions, map_threads, ZedMappingContext};
#[allow(
    unused_imports,
    reason = "store is consumed by the adapter in a later chunk"
)]
pub(crate) use threads_store::{ZedThreadStore, ZedThreadUsage};
