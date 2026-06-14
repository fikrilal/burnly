#![expect(
    dead_code,
    reason = "Phase 3B metadata is consumed by sidecar execution starting in Phase 3C"
)]

mod capability_profiles;
mod manifest;
mod source_registry;
