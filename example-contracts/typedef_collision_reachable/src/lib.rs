//! Negative typedef-collision fixture: the *reachable* conflict case.
//!
//! Unlike `typedef_shaking_caller` (which implements neither market trait,
//! so both `SharedConfig` defs are shaken), this contract IMPLEMENTS
//! `MarketV1` — anchoring it and making its `SharedConfig` reachable from
//! the retained spec — while still linking `typedef_shaking_if_v2`, whose
//! unimplemented candidate contributes a differently-shaped `SharedConfig`
//! def to the pool.  With the retained name ambiguous, `collect_named_types`
//! must fail the spec rewrite with a typedef conflict: the wasm build
//! succeeds, but `groundhog-spec-tool rewrite` must error.  dev.sh's
//! `check_typedef_conflict_detected` asserts exactly that.

#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

use sdk::*;
use typedef_shaking_if_v1::SharedConfig as ConfigV1;
use typedef_shaking_if_v2::{MarketV2Client, SharedConfig as ConfigV2};

pub struct CollisionContract;

/// Implements `MarketV1` (anchoring it) and forwards to a `MarketV2` market
/// through its client — the linkage that drags v2's conflicting
/// `SharedConfig` def into the artifact.
#[contract]
impl typedef_shaking_if_v1::MarketV1 for CollisionContract {
    fn configure(cfg: ConfigV1) -> u32 {
        MarketV2Client::new(&ContractAddress::new([0u8; 32])).configure(
            ConfigV2 {
                limit: cfg.limit as u64,
                flag: 0,
            },
        )
    }
}
