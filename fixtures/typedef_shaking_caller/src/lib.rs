//! Typedef-collision shaking example contract.
//!
//! Depends on two "versions" of a market interface crate purely to call
//! other contracts, implementing neither `MarketV1` nor `MarketV2`.  The
//! two crates define differently-shaped `#[contract_type]`s that share the
//! spec symbol `SharedConfig`; both defs co-travel into the raw spec
//! section via the unimplemented trait candidates.  Because neither def is
//! reachable from this contract's anchored interface, the conflict must not
//! fail the build, and `SharedConfig` (plus both market interfaces) must be
//! absent from the final canonicalized spec.

#![cfg_attr(not(feature = "dev"), no_std)]

thruput_sdk::declare_mock_contract!();

use thruput_sdk::*;
use typedef_shaking_if_v1::{MarketV1Client, SharedConfig as ConfigV1};
use typedef_shaking_if_v2::{MarketV2Client, SharedConfig as ConfigV2};

pub struct Caller;

#[contract]
impl Caller {
    /// Configure both markets through their generated clients — the realistic
    /// reason a contract links interface crates it does not implement.
    pub fn configure_both(
        market_v1: &ContractAddress,
        market_v2: &ContractAddress,
    ) -> u32 {
        let a =
            MarketV1Client::new(market_v1).configure(ConfigV1 { limit: 1 });
        let b = MarketV2Client::new(market_v2)
            .configure(ConfigV2 { limit: 2, flag: 3 });
        a + b
    }
}
