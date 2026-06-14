//! Tauri command, event, and transport mapping boundary.
//!
//! IPC handlers invoke application use cases and do not own product rules or
//! infrastructure behavior.

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "response primitives are consumed by registered commands in Phase 2B"
    )
)]
mod response;
