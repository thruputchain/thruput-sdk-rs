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
//! Nothing here reaches the node. The contract verifies with the
//! `VerifySignature` and `SHA256` syscalls it already has and mints with
//! `MintAuthToken`; `hogd` enforces the auth-token tree it already enforces and
//! never parses an operation preimage.
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
//! [`SignedOpNonces::verify_and_spend`] performs all of it, and each check maps
//! to a [`SignedOpError`]. Skipping any one of them reopens a replay path:
//!
//! - the nonce slot, against what this signer has already spent;
//! - `valid_until`, against the current block height;
//! - that the body decodes as the payload type the contract expects;
//! - the signature, against the bytes all of that produces.
//!
//! Three further bindings are inside those bytes rather than checked
//! separately — the verifying contract (the contract's own address), the
//! network, and the payload type's own definition. The last is why there is no
//! "wrong operation type" check to forget: name the wrong payload type and the
//! signature simply fails.
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
//! is why [`SignedOpNonces`] stores one rather than reading it. Set it at
//! deployment with [`SignedOpNonces::initialize`] and treat it as
//! configuration. Until the protocol grows a way to read it, a contract that
//! hardcodes or omits it is replayable across any network carrying the same
//! address.

extern crate alloc;

mod error;
pub use error::*;

mod nonces;
pub use nonces::*;

mod verify;
pub use verify::*;

use alloc::vec::Vec;
use sdk::signer::{Opaque, Signed};
use sdk::xdr::BytesM;
use sdk::types::Ed25519PublicKey;
use sdk::{BlockHeight, contract};

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
    /// The nonce **slot** this operation spends — an index, not a counter.
    ///
    /// Slots may be spent in any order, so a signer can have several operations
    /// outstanding without the submission order mattering. See
    /// [`SignedOpNonces`]. Read the signer's current word with
    /// [`SignedOp::signed_op_nonce_word`] to pick a free one.
    pub nonce: u64,
    /// Last block height at which this operation may execute.
    pub valid_until: BlockHeight,
    /// The CXS-encoded application payload.
    ///
    /// Bytes on the wire because calldata is bytes, but never treated as bytes:
    /// [`SignedOpNonces::verify_and_spend`] decodes them as the payload type the
    /// contract expects and rebuilds the preimage from the typed value, so the
    /// payload's
    /// [`ContractTypeSpec::TYPE_DEF`](groundhog_contract_spec::ContractTypeSpec)
    /// is bound into the signature. There is no operation-type field here for
    /// the same reason: the type is in the signed bytes, not on the wire.
    ///
    /// Typed as an unbounded [`BytesM`], the same as a transaction's
    /// `Calldata`: an operation body is application calldata, and this
    /// interface has no basis for a tighter bound than a call itself carries.
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
    /// The 64-slot bitmap covering nonces `word * 64 ..= word * 64 + 63` for
    /// `signer`; a set bit is a spent slot.
    ///
    /// Clients read one word to find a free slot, which is why this returns a
    /// word rather than a single slot's state: it answers 64 questions per
    /// call. See [`SignedOpNonces::word`].
    fn signed_op_nonce_word(
        &self,
        signer: &Ed25519PublicKey,
        word: u64,
    ) -> u64;

    /// Verify `op` under `signature`, spend its nonce, and mint its auth
    /// tokens.
    ///
    /// Returns an empty `Vec` if the operation authorizes something needing no
    /// tokens. Throws a [`SignedOpError`] if any check fails; it must not
    /// return an empty `Vec` to signal refusal, which a caller expecting no
    /// tokens cannot distinguish from success.
    ///
    /// `signature` is a borrow, matching `ERC1271::is_valid_signature`: SDK
    /// argument decoding borrows the calldata buffer for the whole handler
    /// frame, so a slice is zero-copy where a `Vec<u8>` would copy 64 bytes
    /// onto a guest heap that never reclaims them.
    fn execute_signed_op(
        &mut self,
        op: Operation,
        signature: &[u8],
    ) -> Vec<Signed<Opaque>>;
}
