//! Command Code collector infrastructure.
//!
//! Phase 1: source identity and detection. Phase 2: transcript reader and
//! parser. The adapter still fails closed on collection until a later chunk
//! wires the mapper.

mod adapter;
mod commandcode_home;
mod detection;
mod transcript_parser;
mod transcript_reader;

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
#[allow(
    unused_imports,
    reason = "reader and parser are consumed by a later chunk"
)]
pub(crate) use transcript_parser::{
    parse_transcript, ParsedTranscript, TranscriptKind, TranscriptUsage,
};
#[allow(
    unused_imports,
    reason = "reader and parser are consumed by a later chunk"
)]
pub(crate) use transcript_reader::{TranscriptFile, TranscriptReader, TranscriptScanSummary};
