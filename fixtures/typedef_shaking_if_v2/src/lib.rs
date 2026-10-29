//! Interface definitions standing in for "v2" of a market interface crate.
//!
//! See `typedef_shaking_if_v1` for the full scenario.  This crate's
//! `SharedConfig` has a different shape than v1's, so a caller linking both
//! crates carries two conflicting defs of the same spec symbol.

#![cfg_attr(not(feature = "dev"), no_std)]

use thruput_sdk::*;

/// The "v2" shape of the shared config type: same spec symbol as v1's
/// `SharedConfig`, different fields.
#[contract_type]
pub struct SharedConfig {
    pub limit: u64,
    pub flag: u32,
}

/// Never implemented by the caller; its candidate fragments (and the
/// `SharedConfig` typedef they reference) must be shaken from the spec.
#[contract]
pub trait MarketV2 {
    fn configure(cfg: SharedConfig) -> u32;
}
