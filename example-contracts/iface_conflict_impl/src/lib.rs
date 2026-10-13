//! Interface-collision fixture: the *anchored* case.
//!
//! Unlike `iface_conflict_caller` (which implements neither `Token`, so
//! both colliding candidates are shaken), this contract IMPLEMENTS
//! `iface_conflict_if_a::Token` — anchoring the shared `Token` interface
//! key — while still linking `iface_conflict_if_b`, whose colliding
//! candidate carries different emits metadata for that key.
//!
//! This used to fail the spec rewrite: fragments carried only the key, so
//! B's candidate looked like a second, disagreeing description of the one
//! interface. Each fragment now names its declarer (the trait's spec id), so
//! B's candidate is shaken like any unimplemented interface and the spec
//! describes A's `Token`, emits and all. dev.sh's
//! `check_rv32_contract_spec_artifacts` asserts that. Implementing *both*
//! traits is still refused — see `negative-contracts/iface_declarer_collision`.

#![cfg_attr(not(feature = "dev"), no_std)]

thruput_sdk::declare_mock_contract!();

use iface_conflict_if_b::TokenClient as TokenClientB;
use thruput_sdk::*;

pub struct ConflictImpl;

/// Implements A's `Token` (anchoring the shared key) and forwards to a B
/// token through its client — the linkage that drags B's colliding
/// candidate into the artifact.
#[contract]
impl iface_conflict_if_a::Token for ConflictImpl {
    fn ping() -> u32 {
        TokenClientB::new(&ContractAddress::new([0u8; 32])).ping()
    }
}
