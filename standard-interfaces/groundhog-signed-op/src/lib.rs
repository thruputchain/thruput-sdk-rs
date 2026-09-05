#![cfg_attr(not(feature = "dev"), no_std)]
#![allow(clippy::result_large_err)]

//! Signed operations: off-chain authorization for Groundhog contracts.
//!
//! A transaction signature authorizes a submission and is checked by the
//! validator. A *signed operation* authorizes an action and is checked by a
//! contract, so the signer need not be the submitter: the pattern behind
//! permits, session keys and delegated wallet actions.
//!
//! This crate is the interface half: the [`Operation`] wire type, the
//! [`SignedOp`] contract trait, and [`SignedOpError`]. It pulls in nothing
//! beyond the SDK, so a caller that only needs to *invoke* a signed-op
//! contract through `SignedOpClient` depends on this crate alone. The bytes
//! that get signed are defined by `groundhog_domain_separator::SignedOp`, and
//! the verification and nonce state a verifying contract embeds live in the
//! `groundhog-signed-op` component crate.
//!
//! Like `ERC1271::is_valid_signature`, [`SignedOp::execute_signed_op`] returns
//! auth tokens: a validated operation mints an auth-token tree, so one
//! signature can authorize a whole scoped call graph, each downstream contract
//! consuming only its own node. Callers destructure the result with
//! [`AssumeTokens::assume_tokens`](sdk::AssumeTokens::assume_tokens).

extern crate alloc;

mod error;
pub use error::*;

use alloc::vec::Vec;
use sdk::signer::{Opaque, Signed};
use sdk::types::{Ed25519PublicKey, Ed25519Signature};
use sdk::xdr::BytesM;
use sdk::{BlockHeight, contract};

/// The wire form of a signed operation.
///
/// The network hash and the verifying contract are covered by the signature
/// but deliberately absent here: the verifier supplies them from what it
/// knows (`sdk::block::get_network_hash`, `sdk::get_current_address`). Taking
/// them off the wire would let a submitter name the network and contract the
/// signature was meant for.
#[sdk::contract_type]
pub struct Operation {
    /// The key that signed this operation. Whether that key is *allowed* to
    /// authorize the action is the verifying contract's policy.
    pub signer: Ed25519PublicKey,
    /// The signer's sequence number at the verifying contract; see
    /// [`SignedOp::next_signed_op_nonce`].
    pub nonce: u64,
    /// Last block height (inclusive) at which this operation may execute.
    pub valid_until: BlockHeight,
    /// The CXS-encoded application payload. A verifier decodes it as the
    /// payload type it expects before checking the signature, so the type's
    /// definition is bound into the signature and a non-canonical encoding
    /// re-encodes to what was signed or fails.
    pub body: BytesM,
}

/// A contract that accepts operations authorized by an off-chain signature.
#[contract(client = SignedOpClient, throws = SignedOpError)]
pub trait SignedOp {
    /// The nonce `signer`'s next operation at this contract must carry.
    ///
    /// Nonces are sequential per signer, starting at zero: each accepted
    /// operation increments it, so operations from one signer execute in the
    /// order they were signed, and an unexecuted one is retired by any later
    /// operation.
    fn next_signed_op_nonce(&self, signer: &Ed25519PublicKey) -> u64;

    /// Verify `op` under `signature`, spend its nonce, and mint its auth
    /// tokens.
    ///
    /// Returns an empty `Vec` if the operation authorizes something needing no
    /// tokens. Throws a [`SignedOpError`] if any check fails; it must not
    /// return an empty `Vec` to signal refusal.
    fn execute_signed_op(
        &mut self,
        op: Operation,
        signature: &Ed25519Signature,
    ) -> Vec<Signed<Opaque>>;
}
