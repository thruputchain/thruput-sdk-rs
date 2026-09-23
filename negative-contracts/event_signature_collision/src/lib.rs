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
//! dev.sh cannot assert that yet: the rv32 pipeline canonicalizes with the
//! pinned CLI's embedded spec-tool, which does not carry the rule until a CLI
//! release does. `check_rv32_event_signature_pending_cli` asserts the build
//! still succeeds and fails the moment it stops — that is the signal to flip
//! it to assert the conflict.

#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

use sdk::*;

pub mod ledger {
    use sdk::*;

    #[derive(Event)]
    pub struct Updated {
        #[topic]
        pub who: ContractAddress,
        pub limit: u32,
    }
}

pub mod vault {
    use sdk::*;

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
