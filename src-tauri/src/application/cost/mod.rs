//! Burnly cost calculator.
//!
//! Prices token usage from an embedded, build-time models.dev pricing
//! snapshot. This is the third cost layer: source-reported and
//! collector-calculated costs win when present; this calculator fills the
//! sources that report no cost (Grok, Antigravity, ZCode) and gap-fills
//! zero-with-positive-tokens cases where the collector lacks pricing for a
//! model the snapshot knows.

#![allow(
    dead_code,
    reason = "calculator is consumed by collector wiring in a later chunk"
)]

mod calculator;
mod snapshot;

#[allow(
    unused_imports,
    reason = "consumed by collector wiring in a later chunk"
)]
pub(crate) use calculator::{
    calculate_cost, gap_fill_cost, gap_fill_daily, gap_fill_session, BurnlyCostCalculator,
    CostCalculation,
};
#[allow(
    unused_imports,
    reason = "consumed by collector wiring in a later chunk"
)]
pub(crate) use snapshot::{PricingEntry, PricingSnapshot};
