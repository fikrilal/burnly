//! Zed agent collector infrastructure.
//!
//! Wires the thread store, mapper, telemetry reader, and cost calculator into
//! the collector port.

mod adapter;
mod detection;
mod mapper;
mod telemetry_reader;
mod threads_store;

pub(crate) use adapter::ZedCollector;
pub(crate) use detection::default_zed_data_dir;
