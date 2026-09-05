#![cfg_attr(not(feature = "dev"), no_std)]
#![allow(clippy::result_large_err)]

//! Signed operations: off-chain authorization for Groundhog contracts.
//!
//! A transaction signature authorizes a submission and is checked by the
//! validator. A *signed operation* authorizes an action and is checked by a
//! contract, so the signer need not be the submitter: the pattern behind
//! permits, session keys and delegated wallet actions.
//!
//! This is Groundhog's equivalent of ERC-1271 — a contract validates a
//! signature under its own policy and hands back what it authorizes — with
//! the message format built in rather than left to each wallet. The message
//! is typed and domain-separated in the spirit of EIP-712: the signed bytes
//! bind the network, the verifying contract, the payload type's definition,
//! a [`Uniquifier`] and the payload itself, so a signature cannot be moved to
//! another network, another contract, another shape of message, or replayed.
//! See `groundhog_domain_separator::SignedOp` for the exact layout.
//!
//! Replay protection is the chain's own model. An operation carries a
//! [`Uniquifier`] — a signer-chosen nonce and an expiry height — and a
//! verifying contract consumes the `(nonce, block_expiry_height)` pair
//! exactly once, the way the transaction replay cache consumes a
//! transaction's uniquifier at its invoked address. Nonces are unordered, so
//! nothing on-chain has to be read before signing and operations need not be
//! submitted in the order they were signed. The consuming side lives in the
//! `groundhog-signed-op` component crate.
//!
//! This crate is the interface half: the [`Operation`] wire type, the
//! [`SignedOp`] contract trait, and [`SignedOpError`]. It pulls in nothing
//! beyond the SDK, so a caller that only needs to *invoke* a signed-op
//! contract through `SignedOpClient` depends on this crate alone.
//!
//! [`SignedOp::execute_signed_op`] returns auth tokens: a validated operation
//! mints an auth-token tree, so one signature can authorize a whole scoped
//! call graph, each downstream contract consuming only its own node. Callers
//! destructure the result with
//! [`AssumeTokens::assume_tokens`](sdk::AssumeTokens::assume_tokens).

extern crate alloc;

mod error;
pub use error::*;

use alloc::vec::Vec;
use sdk::signer::{Opaque, Signed};
use sdk::types::transaction::Uniquifier;
use sdk::types::{Ed25519PublicKey, Ed25519Signature};
use sdk::xdr::BytesM;
use sdk::{BlockHeight, contract};

/// How far past the current height an operation may set its expiry.
///
/// The chain's replay cache keeps consumed uniquifiers until they expire and
/// bounds how many distinct unexpired heights it will hold
/// (`kReplayCacheHeightsMaxLen`); a verifying contract applies the same bound
/// so no consumed operation outlives what the chain's own cache would keep.
/// Signers pick `block_expiry_height` at most this far ahead.
pub const MAX_EXPIRY_HORIZON: i64 =
    groundhog_xdr::storage_item::kReplayCacheHeightsMaxLen as i64;

/// The wire form of a signed operation.
///
/// The network hash and the verifying contract are covered by the signature
/// but deliberately absent here: the verifier supplies them from what it
/// knows (`sdk::block::get_network_hash`, `sdk::get_current_address`). Taking
/// them off the wire would let a submitter name the network and contract the
/// signature was meant for.
///
/// `block_expiry_height` and `nonce` are a [`Uniquifier`], declared in its
/// field order so their encoding is byte-for-byte the chain's;
/// [`uniquifier`](Self::uniquifier) reassembles it.
#[sdk::contract_type]
pub struct Operation {
    /// The key that signed this operation. Whether that key is *allowed* to
    /// authorize the action is the verifying contract's policy.
    pub signer: Ed25519PublicKey,
    /// Last block height (inclusive) at which this operation may execute. At
    /// most [`MAX_EXPIRY_HORIZON`] past the current height.
    pub block_expiry_height: BlockHeight,
    /// Chosen by the signer: any value, as long as the pair
    /// `(nonce, block_expiry_height)` has not been consumed at the verifying
    /// contract. Random or locally counted both work; there is no ordering.
    pub nonce: u64,
    /// The CXS-encoded application payload. A verifier decodes it as the
    /// payload type it expects before checking the signature, so the type's
    /// definition is bound into the signature and a non-canonical encoding
    /// re-encodes to what was signed or fails.
    pub body: BytesM,
}

impl Operation {
    /// The `(nonce, block_expiry_height)` pair a verifier consumes.
    pub fn uniquifier(&self) -> Uniquifier {
        Uniquifier {
            block_expiry_height: self.block_expiry_height.clone(),
            nonce: self.nonce,
        }
    }
}

/// A contract that accepts operations authorized by an off-chain signature.
#[contract(client = SignedOpClient, throws = SignedOpError)]
pub trait SignedOp {
    /// Verify `op` under `signature`, consume its uniquifier, and mint its
    /// auth tokens.
    ///
    /// Returns an empty `Vec` if the operation authorizes something needing no
    /// tokens. Throws a [`SignedOpError`] if any check fails; it must not
    /// return an empty `Vec` to signal refusal. A refusal is a throw, so it
    /// consumes nothing: a refused operation stays submittable until it
    /// expires.
    fn execute_signed_op(
        &mut self,
        op: Operation,
        signature: &Ed25519Signature,
    ) -> Vec<Signed<Opaque>>;
}
