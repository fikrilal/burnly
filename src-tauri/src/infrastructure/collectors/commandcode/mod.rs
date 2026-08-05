//! Command Code collector infrastructure.
//!
//! Phase 1: source identity and detection. Phase 2: transcript reader and
//! parser. Phase 3: mapper and cost. The adapter still fails closed on
//! collection until a later chunk wires the full pipeline.

mod adapter;
mod commandcode_home;
mod detection;
mod mapper;
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
    reason = "mapper is consumed by the adapter in a later chunk"
)]
pub(crate) use mapper::{
    map_daily, map_sessions, map_transcripts, CommandCodeMappingContext, CommandCodeMappingError,
    MappedCandidates,
};
#[allow(
    unused_imports,
    reason = "parser is consumed by the adapter in a later chunk"
)]
pub(crate) use transcript_parser::{
    parse_transcript, ParsedTranscript, TranscriptKind, TranscriptUsage,
};
#[allow(
    unused_imports,
    reason = "reader is consumed by the adapter in a later chunk"
)]
pub(crate) use transcript_reader::{TranscriptFile, TranscriptReader, TranscriptScanSummary};
