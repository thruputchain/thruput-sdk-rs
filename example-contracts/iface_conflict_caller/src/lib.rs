//! Interface-metadata conflict shaking example contract.
//!
//! Links two interface crates whose `Token` traits collide on the same
//! interface key (default name = trait ident, default version = shared
//! workspace crate version) with different emits metadata, purely to call
//! other contracts — implementing neither.  Both `Token` candidates are
//! shaken, so their metadata disagreement must not fail the build, and
//! neither `Token` nor `PingEvent` may appear in the final spec.

#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

use iface_conflict_if_a::TokenClient as TokenClientA;
use iface_conflict_if_b::TokenClient as TokenClientB;
use sdk::*;

pub struct ConflictCaller;

#[contract]
impl ConflictCaller {
    /// Ping both tokens through their generated clients — the realistic
    /// reason a contract links interface crates it does not implement.
    pub fn ping_both(
        token_a: &ContractAddress,
        token_b: &ContractAddress,
    ) -> u32 {
        TokenClientA::new(token_a).ping() + TokenClientB::new(token_b).ping()
    }
}
