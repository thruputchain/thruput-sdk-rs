//! A wallet that authorizes ERC20 permits from off-chain signatures.
//!
//! This is the owner's half of the permit flow. The token contract never sees
//! a public key or a signature: a submitter brings the signed operation here,
//! [`SignedOp::execute_signed_op`] validates it and mints a `PermitGrant` auth
//! token naming the requested ERC20 as consumer, and the submitter carries
//! that token down into the token contract. Which key is currently
//! authoritative is therefore wallet state, not something derived from the
//! wallet's address.
//!
//! Deliberately single-purpose. A signature binds exactly one payload type
//! (the preimage covers `PermitRequest::TYPE_DEF`), so a wallet serving
//! several kinds of signed operation would need one enum spanning all of them
//! — and every added variant would change that `TYPE_DEF` and retire every
//! outstanding signature. Per-purpose wallets also give each purpose its own
//! nonce sequence, so an unsubmitted permit doesn't stall unrelated
//! operations.

#![cfg_attr(not(feature = "dev"), no_std)]

extern crate alloc;

sdk::declare_mock_contract!();

use alloc::vec;
use alloc::vec::Vec;

use groundhog_erc20_permit_if::{PermitGrant, PermitRequest};
use groundhog_signed_op::{Operation, SignedOp, SignedOpNonces};
use sdk::mintable::{AuthTokenPreimage, MintableType};
use sdk::signer::{Opaque, Signed};
use sdk::storage::Value;
use sdk::types::{Ed25519PublicKey, Ed25519Signature};
use sdk::*;

/// What goes into a `PermitGrant` mint: `#[contract_type]` generates a
/// preimage mirror of every mintable type, and this names it.
type PermitGrantPreimage = <PermitGrant as MintableType>::Preimage;

/// Why the wallet refused, as distinct from why an operation failed
/// verification — those are
/// [`SignedOpError`](groundhog_signed_op::SignedOpError).
#[contract_exception]
#[non_exhaustive]
#[cfg_attr(feature = "dev", derive(thiserror::Error))]
pub enum WalletError {
    #[cfg_attr(feature = "dev", error("Wallet already initialized"))]
    AlreadyInitialized = 1,
    /// The operation's signer is not the key this wallet currently accepts.
    /// What a rotated-out key gets.
    #[cfg_attr(feature = "dev", error("Signer is not the wallet owner"))]
    NotOwner = 2,
    /// `rotate_owner` called by anyone other than the administrator.
    #[cfg_attr(
        feature = "dev",
        error("Caller is not the wallet administrator")
    )]
    NotAdministrator = 3,
}

#[sdk::storage]
pub struct PermitWallet {
    /// Who may rotate the owner key. A wallet with richer governance would put
    /// a multisig or a timelock here; the point is only that rotation is not
    /// open.
    administrator: Value<ContractAddress>,
    owner: Value<Ed25519PublicKey>,
    nonces: SignedOpNonces,
}

#[contract(client = PermitWalletClient, throws = WalletError)]
impl PermitWallet {
    /// Set the key this wallet accepts and the address allowed to change it.
    /// Throws [`WalletError::AlreadyInitialized`] if called more than once.
    pub fn initialize(
        &mut self,
        administrator: &ContractAddress,
        owner: &Ed25519PublicKey,
    ) {
        if self.owner.try_get().none_if_missing().is_some() {
            throw(WalletError::AlreadyInitialized);
        }
        self.administrator.set(administrator);
        self.owner.set(owner);
    }

    /// Replace the accepted key. Throws [`WalletError::NotAdministrator`]
    /// unless the caller is the administrator.
    ///
    /// Permits signed by the previous key stop being honored here for as long
    /// as it is not the owner: authority lives in the wallet, so revocation is
    /// a write rather than something a token contract has to be told about.
    /// It is not permanent, though — see [`execute_signed_op`] — so rotating
    /// *back* to a key re-admits whatever it signed and never submitted.
    ///
    /// [`execute_signed_op`]: SignedOp::execute_signed_op
    pub fn rotate_owner(&mut self, new_owner: &Ed25519PublicKey) {
        if get_caller() != self.administrator.get() {
            throw(WalletError::NotAdministrator);
        }
        self.owner.set(new_owner);
    }

    /// The key this wallet currently accepts.
    pub fn owner(&self) -> Ed25519PublicKey {
        self.owner.get()
    }
}

#[contract(version = "0.1.0")]
impl SignedOp for PermitWallet {
    fn next_signed_op_nonce(&self, signer: &Ed25519PublicKey) -> u64 {
        self.nonces.next_nonce(signer)
    }

    fn execute_signed_op(
        &mut self,
        op: Operation,
        signature: &Ed25519Signature,
    ) -> Vec<Signed<Opaque>> {
        // Authority first: only the current owner's operations are worth
        // verifying. Note what this does *not* do. A refusal is a throw, the
        // throw reverts this frame, and a reverted frame writes nothing — so
        // no refusal can spend the nonce, and a permit signed by a former key
        // is not retired, only refused while that key is not the owner. The
        // `SignedOp` interface requires refusal to throw, so this is
        // inherent, not a choice made here.
        if op.signer != self.owner.get() {
            throw(WalletError::NotOwner);
        }

        // Nonce, expiry, body decode and signature, against a preimage bound
        // to this network and this wallet. Spends the nonce, so the operation
        // is single-use whatever happens downstream.
        let request: PermitRequest =
            self.nonces.verify_and_spend(&op, signature);

        // `consumer` comes from the signed body, so the owner picks which
        // token may consume this grant. Taking it from `get_caller()` instead
        // would let whoever submits the operation point it at a different
        // ERC20 the owner also holds.
        let node = sdk::mintable::build_auth_token_node::<PermitGrant>(
            AuthTokenPreimage {
                consumer: request.token,
                data: PermitGrantPreimage {
                    spender: request.spender,
                    amount: request.amount,
                },
            },
        );

        // The submitter carries the token to the ERC20 in this same frame, so
        // it need not outlive the current block.
        vec![sdk::signer::mint_opaque_auth_token(
            sdk::block::get_block_height(),
            &sdk::mintable::encode_auth_token_node(&node),
        )]
    }
}
