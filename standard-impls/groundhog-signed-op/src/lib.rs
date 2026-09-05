//! Embeddable implementation of the [`SignedOp`] standard interface.
//!
//! The interface crate (`groundhog-signed-op-if`, re-exported here) is the
//! trait, wire type and error. This crate is the verifying side: the checks a
//! contract performs on an [`Operation`], and the replay state behind them,
//! packaged as [`SignedOpReplayCache`], a storage component a contract embeds
//! as a field of its own storage struct. The contract still declares the
//! `#[contract] impl SignedOp` block itself:
//!
//! ```ignore
//! #[sdk::storage]
//! pub struct MyWallet {
//!     replay: SignedOpReplayCache,
//!     // ...
//! }
//!
//! #[contract]
//! impl SignedOp for MyWallet {
//!     fn execute_signed_op(
//!         &mut self,
//!         op: Operation,
//!         signature: &Ed25519Signature,
//!     ) -> Signed<Opaque> {
//!         let transfer: Transfer = self.replay.consume(&op, signature);
//!         // ... check that `op.signer` may authorize this, then mint
//!     }
//! }
//! ```
//!
//! # What a verifier must check
//!
//! [`SignedOpReplayCache::consume`] performs all of it, in this order, and
//! each check maps to a [`SignedOpError`]:
//!
//! - `block_expiry_height`, against the current block height;
//! - the `(nonce, block_expiry_height)` pair, against what this contract has
//!   already consumed;
//! - that the body decodes as the payload type the contract expects;
//! - the signature, against the bytes all of that produces.
//!
//! The network hash and the verifying contract are inside those bytes rather
//! than on the wire: they come from `sdk::block::get_network_hash` and
//! `sdk::get_current_address`, so a submitter cannot name the network or
//! contract a signature was meant for.
//!
//! # The replay model
//!
//! This is the transaction replay cache's model, kept in contract storage
//! until the chain exposes its own to contracts. A consumption is identified
//! by the pair, scoped to this contract: the same nonce under a different
//! expiry is a different operation. Entries are dead once their height has
//! passed and may be [`prune`](SignedOpReplayCache::prune)d by anyone; that
//! can never re-admit an operation, because expiry is checked before the
//! cache. Two consumptions of one pair in the same block conflict at commit.

#![cfg_attr(not(feature = "dev"), no_std)]
#![allow(clippy::result_large_err)]

pub use groundhog_signed_op_if::*;

use groundhog_contract_spec::ContractTypeSpec;
use groundhog_domain_separator::SignedOp as SignedOpPreimage;
use sdk::exn::NoneIfMissing;
use sdk::storage::{Deletable, Map, Value};
use sdk::types::Ed25519Signature;
use sdk::types::transaction::Uniquifier;
use sdk::xdr::{ReadXDR, WriteXDR};

/// The stateless checks: expiry, decode, signature. Returns the decoded
/// payload, or throws the matching [`SignedOpError`].
///
/// This does not touch the replay cache, so on its own it does not make an
/// operation single-use. Use [`SignedOpReplayCache::consume`] unless the
/// contract keeps its own replay state.
///
/// `T` is the payload type this contract expects. Its `TYPE_DEF` is part of
/// the signed bytes, so naming the wrong `T` fails as
/// [`SignedOpError::InvalidSignature`] rather than reinterpreting the message.
pub fn verify_signed_op<T>(op: &Operation, signature: &Ed25519Signature) -> T
where
    T: WriteXDR + for<'r> ReadXDR<'r> + ContractTypeSpec,
{
    check_expiry(op);
    decode_and_verify(op, signature)
}

/// Throws [`SignedOpError::Expired`] once the current height is past
/// `op.block_expiry_height`. Inclusive, as the chain reads a transaction's
/// `block_expiry_height`.
fn check_expiry(op: &Operation) {
    if sdk::block::get_block_height() > op.block_expiry_height {
        sdk::throw(SignedOpError::Expired);
    }
}

/// Decode the body as `T` and check the signature over the preimage that
/// decoding produces.
fn decode_and_verify<T>(op: &Operation, signature: &Ed25519Signature) -> T
where
    T: WriteXDR + for<'r> ReadXDR<'r> + ContractTypeSpec,
{
    let body: T = match sdk::xdr::cxs::decode(op.body.0.as_slice()) {
        Ok(body) => body,
        Err(_) => sdk::throw(SignedOpError::MalformedBody),
    };

    let preimage = SignedOpPreimage {
        network: sdk::block::get_network_hash(),
        verifying_contract: sdk::get_current_address(),
        uniquifier: op.uniquifier(),
        body: &body,
    }
    .preimage()
    .unwrap_or_else(|_| sdk::throw(SignedOpError::MalformedBody));

    if !sdk::crypto::verify_signature_ed25519(&op.signer, signature, &preimage)
    {
        sdk::throw(SignedOpError::InvalidSignature);
    }
    body
}

/// The consumed uniquifiers of a signed-operation verifier.
///
/// Embed this in the contract's storage and drive it with
/// [`consume`](Self::consume), which checks and records the pair in one call
/// so "verified but never consumed" is not expressible.
///
/// Keyed by the [`Uniquifier`] alone and scoped to the contract, exactly as
/// the chain's replay cache is scoped to an address rather than a key: every
/// key authorized here shares one space, and a pair one key has consumed is
/// refused from every other. A contract that admits several signers should
/// have them draw nonces at random — 64 random bits do not collide by
/// accident, two counters do — and a collision under any scheme costs a
/// fresh pair, never an execution.
#[sdk::storage]
pub struct SignedOpReplayCache {
    consumed: Map<Uniquifier, Value<()>>,
}

impl SignedOpReplayCache {
    /// Verify an operation and consume its uniquifier, returning the decoded
    /// payload. Throws [`SignedOpError`] on any failed check, in which case
    /// nothing is written.
    ///
    /// The checks run in the order the crate docs list them. Expiry comes
    /// before the cache, so a dead operation is
    /// [`Expired`](SignedOpError::Expired) whether or not its entry has been
    /// pruned; pruning changes nothing observable. The cache comes before the
    /// signature, so a replay costs no verification.
    pub fn consume<T>(
        &mut self,
        op: &Operation,
        signature: &Ed25519Signature,
    ) -> T
    where
        T: WriteXDR + for<'r> ReadXDR<'r> + ContractTypeSpec,
    {
        check_expiry(op);
        let uniquifier = op.uniquifier();
        if self
            .consumed
            .at(&uniquifier)
            .try_get()
            .none_if_missing()
            .is_some()
        {
            sdk::throw(SignedOpError::AlreadyConsumed);
        }
        let body = decode_and_verify::<T>(op, signature);
        // Exclusive: two transactions carrying the same operation in one
        // block both see the pair absent and would both write it, and a
        // shared write lets identical values commit side by side. The
        // exclusive lock makes the second consumption conflict instead — the
        // chain's rule for consuming the same uniquifier twice.
        self.consumed.at_mut_exclusive(&uniquifier).set(&());
        body
    }

    /// Delete a dead entry. Throws [`SignedOpError::NotYetExpired`] unless
    /// the current height is past `uniquifier`'s expiry, so an entry can only
    /// be removed once the operation it records could no longer execute
    /// anyway. A no-op for an entry that is not present.
    ///
    /// Anyone may call this through whatever entrypoint a contract exposes
    /// for it; the chain's own cache will garbage-collect instead once it is
    /// available to contracts.
    pub fn prune(&mut self, uniquifier: &Uniquifier) {
        if sdk::block::get_block_height() <= uniquifier.block_expiry_height {
            sdk::throw(SignedOpError::NotYetExpired);
        }
        let mut slot = self.consumed.at_mut(uniquifier);
        if slot.try_get().none_if_missing().is_some() {
            slot.delete();
        }
    }
}
