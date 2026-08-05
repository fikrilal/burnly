//! Command Code collector infrastructure.
//!
//! Wires the transcript reader, parser, and mapper into the collector port.
//! Collection reads `~/.commandcode/projects/**/<session>.jsonl` transcripts
//! read-only and maps usage-bearing messages into Burnly daily/session
//! candidates.

mod adapter;
mod commandcode_home;
mod detection;
mod mapper;
mod transcript_parser;
mod transcript_reader;

pub(crate) use adapter::CommandCodeCollector;
pub(crate) use commandcode_home::default_commandcode_home;
