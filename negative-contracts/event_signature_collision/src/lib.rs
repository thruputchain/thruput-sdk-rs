//! Negative spec fixture: two events the spec names apart and the wire cannot.
//!
//! An event's spec id carries its module (`CRATE@MAJOR::module::path::Name`),
//! but its signature — topic 0 of every emission, and all a consumer has to
//! tell events apart on the wire — is hashed from (crate, MAJOR, ident) only.
//! So the two `Updated` events below have distinct ids and one signature.
//!
//! That is the one case where the spec can promise a distinction the wire
//! cannot deliver. It needs a crate to declare one ident twice in two of its
//! own modules and emit both, which is entirely within one author's control
//! and fixed by renaming one — the cross-crate and cross-version collisions
//! that qualified ids exist for never reach it, because the crate name and
//! major version are both in the signature preimage.
//!
//! Before ids, this failed to build for the wrong reason: both events were
//! named `Updated` in the spec, so canonicalization rejected them as
//! conflicting definitions and the wire collision never got the chance to
//! matter. Ids made the spec accept them, so the wire collision is now
//! checked on its own terms.
//!
//! The rv32 pipeline canonicalizes with the pinned CLI's embedded spec-tool,
//! which carries the rule from CLI 0.19.0-pre.1 on.
//! `check_rv32_event_signature_conflict_detected` in dev.sh asserts the build
//! fails with the conflict naming both `Updated` events.

#![cfg_attr(not(feature = "dev"), no_std)]

thruput_sdk::declare_mock_contract!();

use thruput_sdk::*;

pub mod ledger {
    use thruput_sdk::*;

    #[derive(Event)]
    pub struct Updated {
        #[topic]
        pub who: ContractAddress,
        pub limit: u32,
    }
}

pub mod vault {
    use thruput_sdk::*;

    /// Same ident as `ledger::Updated`, different module, different payload —
    /// and the same signature, because the signature does not carry the module.
    #[derive(Event)]
    pub struct Updated {
        #[topic]
        pub who: ContractAddress,
        pub ceiling: u64,
    }
}

pub struct SignatureCollision;

/// Emitting both is what makes them both retained, and so both checked.
#[contract(emits(ledger::Updated, vault::Updated))]
impl SignatureCollision {
    pub fn ping() -> u32 {
        0
    }
}
