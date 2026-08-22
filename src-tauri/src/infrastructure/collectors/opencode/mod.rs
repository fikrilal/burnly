//! Privacy-safe read-only access to OpenCode V1 and V2 usage storage.
//!
//! This module owns upstream table names, JSON paths, schema compatibility,
//! and cross-generation precedence. Callers receive usage-only scalar records;
//! raw message JSON and conversation-bearing columns never cross this boundary.

#![allow(
    dead_code,
    reason = "chunk 1 establishes the reader consumed by the later ledger and adapter chunks"
)]

mod discovery;
mod schema;
mod store;

#[allow(
    unused_imports,
    reason = "chunk 1 exposes the reader API for the adapter introduced in a later chunk"
)]
pub(crate) use discovery::{default_opencode_database, resolve_opencode_database};
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
