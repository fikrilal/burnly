//! Application-owned clock port for timezone-independent epoch timestamps.
//!
//! Use cases obtain wall-clock time through this port so they remain testable
//! with deterministic fakes.

#![allow(
    dead_code,
    reason = "The clock port is implemented now and consumed by the Phase 4E refresh coordinator"
)]

pub(crate) trait Clock: Send + Sync {
    /// Milliseconds since the Unix epoch. Implementations return a non-negative
    /// value; a clock before the epoch is reported as `0`.
    fn now_epoch_ms(&self) -> i64;
}
