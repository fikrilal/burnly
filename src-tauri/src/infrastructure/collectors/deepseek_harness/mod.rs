//! DeepSeek Harness collector infrastructure.
//!
//! Provides source identity, detection, bounded session-log reading, event
//! parsing, usage folding, candidate mapping, and collector routing for
//! DeepSeek Harness local session data.

mod adapter;
mod deepseek_home;
mod detection;
mod discovery;
mod event_parser;
mod mapper;
mod session_log_reader;
mod usage_fold;

pub(crate) use adapter::DeepSeekHarnessCollector;
pub(crate) use deepseek_home::default_deepseek_harness_home;
