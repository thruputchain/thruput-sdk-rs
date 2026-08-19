//! Negative interface-metadata conflict fixture: the *anchored* case.
//!
//! Unlike `iface_conflict_caller` (which implements neither `Token`, so
//! both colliding candidates are shaken), this contract IMPLEMENTS
//! `iface_conflict_if_a::Token` — anchoring the shared `Token` interface
//! key — while still linking `iface_conflict_if_b`, whose colliding
//! candidate carries different emits metadata for that key.  The retained
//! key's metadata is genuinely ambiguous, so the spec rewrite must fail:
//! the ELF links, but the spec step of `groundhog build --target rv32`
//! must error.
//! dev.sh's `check_iface_conflict_detected` asserts exactly that.

#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

use iface_conflict_if_b::TokenClient as TokenClientB;
use sdk::*;

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
