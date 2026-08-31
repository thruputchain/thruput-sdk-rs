#![cfg_attr(not(feature = "dev"), no_std)]
#![allow(clippy::result_large_err)]

//! Signed operations: off-chain authorization for Groundhog contracts.
//!
//! A transaction signature authorizes a submission and is checked by the
//! validator. A *signed operation* authorizes an action and is checked by a
//! contract, so the signer need not be the submitter — the pattern behind
//! permits, session keys, meta-transactions and delegated wallet actions.
//!
//! This crate is the interface half. The bytes that get signed live in
//! [`groundhog_domain_separator::SignedOp`]; everything here is about carrying
//! one over the wire, checking it, and turning it into auth tokens.
//!
//! # Why it returns auth tokens
//!
//! [`SignedOp::execute_signed_op`] returns `Vec<Signed<Opaque>>`, the same as
//! `ERC1271::is_valid_signature`. That is what makes
//! these *operations* rather than signature checks: a validated operation mints
//! an auth-token tree, and because a `Signed<T>` field encodes to zero bytes
//! and becomes a child node addressed to its own consumer, one signature can
//! authorize a whole scoped call graph. Each downstream contract consumes
//! exactly its own node; none can consume another's. The caller destructures
//! the returned tokens with
//! [`AssumeTokens::assume_tokens`](sdk::AssumeTokens::assume_tokens).
//!
//! # What a verifier must check
//!
//! [`check_signed_op`] performs all of it, and each check maps to a
//! [`SignedOpError`]. Skipping any one of them reopens a replay path:
//!
//! - the signature, against the operation's digest;
//! - `valid_until`, against the current block height;
//! - the nonce, against what this signer owes;
//! - the `type_id`, against the operation the contract thinks it is running.
//!
//! Two further bindings are inside the digest rather than checked separately —
//! the verifying contract (the contract's own address) and the network.
//!
//! # The network hash
//!
//! A contract cannot read its own network hash: there is no syscall for it, and
//! `sdk::block` exposes only the height. It cannot be recovered from the
//! contract address either, because address derivation
//! (`sha256(cxs(DomainSeparator("PubkeyAccount")) || cxs({pubkey, nonce}))`)
//! has no network in its preimage — the same pubkey yields the same address on
//! every network, which is precisely why transaction signatures carry a network
//! hash explicitly.
//!
//! So a verifying contract must be *told* its network hash and store it, which
//! is why [`check_signed_op`] takes one rather than reading it. Store it at
//! deployment and treat it as configuration. Until the protocol grows a way to
//! read it, a contract that hardcodes or omits it is replayable across any
//! network carrying the same address.

extern crate alloc;

mod error;
pub use error::*;

mod verify;
pub use verify::*;

use alloc::vec::Vec;
use sdk::signer::{Opaque, Signed};
use sdk::xdr::BytesM;
use sdk::{BlockHeight, ContractAddress, contract};

/// The wire form of a signed operation: everything the signer committed to that
/// the verifier cannot already know.
///
/// The network hash and the verifying contract are deliberately *absent*. Both
/// are covered by the signature (they are fields of
/// [`groundhog_domain_separator::SignedOp`]), but a verifier must supply them
/// from what it knows rather than from what a submitter sends — taking them
/// off the wire would let a submitter name the network and contract the
/// signature was meant for, which is the replay these fields exist to stop.
#[sdk::contract_type]
pub struct Operation {
    /// Which kind of operation this is, as
    /// [`OperationTypeId::to_u64`](groundhog_domain_separator::OperationTypeId::to_u64).
    ///
    /// A `u64` rather than the type itself so the operation stays expressible
    /// in a contract spec; convert with
    /// [`OperationTypeId::from_u64`](groundhog_domain_separator::OperationTypeId::from_u64).
    pub type_id: u64,
    /// The signer's nonce this operation spends.
    pub nonce: u64,
    /// Last block height at which this operation may execute.
    pub valid_until: BlockHeight,
    /// The CXS-encoded application payload, opaque to this interface.
    ///
    /// Typed as an unbounded [`BytesM`], the same as a transaction's
    /// `Calldata`: an operation body is application calldata, and this
    /// interface has no basis for a tighter bound than the one a call itself
    /// carries.
    pub body: BytesM,
}

/// A contract that accepts operations authorized by an off-chain signature.
///
/// Implementations are wallets and any contract that honours a signature from
/// someone other than its caller. An implementation should also implement
/// `ERC1271` when it wants to answer generic
/// "is this signature valid" questions; the two compose, and
/// `execute_signed_op` is the structured, replay-protected form.
#[contract(client = SignedOpClient, throws = SignedOpError)]
pub trait SignedOp {
    /// The nonce `signer`'s next operation must carry.
    ///
    /// Clients read this to build an operation. Implementations must increment
    /// it when an operation is accepted, so a signature spends exactly once.
    fn signed_op_nonce(&self, signer: &ContractAddress) -> u64;

    /// Verify `op` under `signature`, spend its nonce, and mint its auth
    /// tokens.
    ///
    /// Returns an empty `Vec` if the operation authorizes something needing no
    /// tokens. Throws a [`SignedOpError`] if any check fails; it must not
    /// return an empty `Vec` to signal refusal, which a caller expecting no
    /// tokens cannot distinguish from success.
    fn execute_signed_op(
        &mut self,
        op: Operation,
        signature: Vec<u8>,
    ) -> Vec<Signed<Opaque>>;
}
