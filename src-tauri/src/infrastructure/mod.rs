//! Technical adapters for persistence, collectors, and external systems.
//!
//! Infrastructure implements application-owned contracts and keeps external
//! representations inside the adapter that owns them.

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the database runtime is integrated into startup in Phase 1D"
    )
)]
pub mod database;
