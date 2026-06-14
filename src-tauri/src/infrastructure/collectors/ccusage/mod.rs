#![expect(
    dead_code,
    reason = "Phase 3B metadata is consumed by sidecar execution starting in Phase 3C"
)]

mod capability_profiles;
mod command;
mod envelopes;
mod manifest;
mod process;
mod sidecar;
mod source_registry;
