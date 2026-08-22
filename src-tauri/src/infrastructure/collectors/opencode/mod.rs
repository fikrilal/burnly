//! Privacy-safe read-only access to OpenCode V1 and V2 usage storage.
//!
//! This module owns upstream table names, JSON paths, schema compatibility,
//! and cross-generation precedence. Callers receive usage-only scalar records;
//! raw message JSON and conversation-bearing columns never cross this boundary.

#![allow(
    dead_code,
    reason = "chunk 1 establishes the reader consumed by the later ledger and adapter chunks"
)]

mod adapter;
mod discovery;
mod mapper;
mod schema;
mod store;

pub(crate) use adapter::OpenCodeCollector;
pub(crate) use discovery::default_opencode_database;
#[allow(
    unused_imports,
    reason = "explicit-path discovery is covered by unit tests"
)]
pub(crate) use discovery::resolve_opencode_database;
#[allow(
    unused_imports,
    reason = "chunk 3 exposes mapping APIs for the adapter introduced in chunk 4"
)]
pub(crate) use mapper::{
    map_daily, map_sessions, source_cost_usd_to_micros, OpenCodeMappingContext,
    OpenCodeMappingError,
};
#[allow(
    unused_imports,
    reason = "chunk 1 exposes the reader API for the adapter introduced in a later chunk"
)]
pub(crate) use schema::{OpenCodeGeneration, OpenCodeSchemaCapabilities, OpenCodeSchemaError};
#[allow(
    unused_imports,
    reason = "chunk 1 exposes the reader API for the adapter introduced in a later chunk"
)]
pub(crate) use store::{
    OpenCodeMessageUsage, OpenCodePageSize, OpenCodePageSizeError, OpenCodeReadSnapshot,
    OpenCodeSessionHeader, OpenCodeStore, OpenCodeStoreError, OpenCodeTokenCounters,
};
