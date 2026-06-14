#![expect(
    dead_code,
    reason = "ccusage collector modules are composed by later refresh orchestration"
)]

mod adapter;
mod capability_profiles;
mod command;
mod envelopes;
mod manifest;
mod mapper;
mod process;
mod sidecar;
mod source_registry;

#[allow(
    unused_imports,
    reason = "Concrete collector is wired by later application composition"
)]
pub(crate) use adapter::CcusageCollector;
