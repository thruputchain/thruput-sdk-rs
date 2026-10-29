//! Typedef-id coexistence fixture: the *reachable* two-crate case.
//!
//! Unlike `typedef_shaking_caller` (which implements neither market trait, so
//! both `SharedConfig` defs are shaken), this contract IMPLEMENTS `MarketV1`
//! — anchoring it and making its `SharedConfig` reachable from the retained
//! spec — while still linking `typedef_shaking_if_v2`, whose unimplemented
//! candidate contributes a differently-shaped `SharedConfig` def to the pool.
//!
//! Two crates, one simple name, both defs in the pool, one of them reachable.
//! This used to be an unfixable build failure: the retained reference read
//! `SharedConfig`, both defs answered to it, and canonicalization died with
//! `ConflictingTypeDef` naming a type the author never wrote. Now each def
//! carries its own id, the reference names v1's, and it resolves to exactly
//! one candidate — so the build succeeds and v2's def is shaken as the
//! unreachable thing it always was.
//!
//! `cargo gnaw run fixtures:specs` asserts the resulting spec carries v1's id
//! and not v2's. The collision that must still fail is a genuine one — two
//! defs claiming a single id — and lives in `typedef_id_collision`.

#![cfg_attr(not(feature = "dev"), no_std)]

thruput_sdk::declare_mock_contract!();

use thruput_sdk::*;
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
