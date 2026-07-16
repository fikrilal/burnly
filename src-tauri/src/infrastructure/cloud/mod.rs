//! Burnly cloud client platform for burnly-api.
//!
//! Product features must use this module for burnly-api HTTP and session
//! secrets. Do not open ad-hoc HTTP clients for cloud product calls.

#![allow(
    dead_code,
    reason = "Cloud core is constructed by later auth/collect wiring"
)]

pub(crate) mod client;
pub(crate) mod config;
pub(crate) mod desktop_token;
pub(crate) mod device_id;
pub(crate) mod error;
pub(crate) mod jwt;
pub(crate) mod logout;
pub(crate) mod memory_token_store;
pub(crate) mod refresh;
pub(crate) mod token_store;
