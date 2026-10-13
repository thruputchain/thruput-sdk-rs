//! The smallest wallet that accepts signed operations.
//!
//! One owner key, one payload type, one token per operation. What it shows is
//! the shape every `SignedOp` wallet shares: embed a
//! [`SignedOpReplayCache`], hand [`consume`](SignedOpReplayCache::consume)
//! the key the wallet holds and let it do the verification and replay
//! protection, then mint. The tests pin the replay model — the chain's
//! `(nonce, block_expiry_height)` pair, consumed once, dead after expiry.

#![cfg_attr(not(feature = "dev"), no_std)]

thruput_sdk::declare_mock_contract!();

use thruput_signed_op::{Operation, SignedOp, SignedOpReplayCache};
use thruput_sdk::mintable::AuthTokenPreimage;
use thruput_sdk::signer::{Opaque, Signed};
use thruput_sdk::storage::Value;
use thruput_sdk::types::transaction::Uniquifier;
use thruput_sdk::types::{Ed25519PublicKey, Ed25519Signature};
use thruput_sdk::*;

/// What the owner signs: mint one token carrying `amount` for the caller.
#[contract_type]
pub struct Grant {
    pub amount: u64,
}

#[contract_exception]
#[non_exhaustive]
#[cfg_attr(feature = "dev", derive(thiserror::Error))]
pub enum WalletError {
    #[cfg_attr(feature = "dev", error("Wallet already initialized"))]
    AlreadyInitialized = 1,
}

#[thruput_sdk::storage]
pub struct SignedOpWallet {
    owner: Value<Ed25519PublicKey>,
    replay: SignedOpReplayCache,
}

#[contract(client = SignedOpWalletClient, throws = WalletError)]
impl SignedOpWallet {
    /// Set the key this wallet accepts. Throws
    /// [`WalletError::AlreadyInitialized`] if called more than once.
    ///
    /// Open to any caller. Call it in the transaction that deploys the
    /// wallet, so no other transaction can reach the wallet uninitialized and
    /// claim it.
    pub fn initialize(&mut self, owner: &Ed25519PublicKey) {
        if self.owner.try_get().none_if_missing().is_some() {
            throw(WalletError::AlreadyInitialized);
        }
        self.owner.set(owner);
    }

    /// Drop a consumed uniquifier once its expiry has passed. Open to anyone:
    /// it can only remove entries for operations that could no longer execute.
    ///
    /// Takes the pair's two fields: `Uniquifier` itself has no contract-spec
    /// impl, so it cannot be a calldata parameter directly.
    pub fn prune(&mut self, block_expiry_height: BlockHeight, nonce: u64) {
        self.replay.prune(&Uniquifier {
            block_expiry_height,
            nonce,
        });
    }
}

#[contract]
impl SignedOp for SignedOpWallet {
    fn execute_signed_op(
        &mut self,
        op: Operation,
        signature: &Ed25519Signature,
    ) -> Signed<Opaque> {
        // Expiry, replay, body decode and signature, against a preimage
        // bound to this network and this wallet, under the wallet's own key:
        // `op` names none, so a submitter cannot choose what the signature is
        // checked against. Consumes the pair; a refusal throws and reverts.
        let grant: Grant =
            self.replay.consume(&op, signature, &self.owner.get());

        let node =
            thruput_sdk::mintable::build_auth_token_node::<u64>(AuthTokenPreimage {
                consumer: get_caller(),
                data: grant.amount,
            });
        thruput_sdk::signer::mint_opaque_auth_token(
            thruput_sdk::block::get_block_height(),
            &thruput_sdk::mintable::encode_auth_token_node(&node),
        )
    }
}
