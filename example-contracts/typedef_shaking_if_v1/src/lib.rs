//! Interface definitions standing in for "v1" of a market interface crate.
//!
//! Together with `typedef_shaking_if_v2`, this models a caller depending on
//! two semver-incompatible versions of the same interface crate purely to
//! call other contracts.  Both crates define a `#[contract_type]` named
//! `SharedConfig` with different shapes.  Since the caller implements
//! neither `MarketV1` nor `MarketV2`, both candidate interfaces — and both
//! `SharedConfig` defs — are unreachable and must be shaken from the final
//! spec without the def conflict failing the caller's build.

#![cfg_attr(not(feature = "dev"), no_std)]

use sdk::*;

/// The "v1" shape of the shared config type.  Collides by name with the
/// differently-shaped `SharedConfig` in `typedef_shaking_if_v2`.
#[contract_type]
pub struct SharedConfig {
    pub limit: u32,
}

/// Never implemented by the caller; its candidate fragments (and the
/// `SharedConfig` typedef they reference) must be shaken from the spec.
#[contract]
pub trait MarketV1 {
    fn configure(cfg: SharedConfig) -> u32;
}
