//! The smallest wallet that accepts signed operations.
//!
//! One owner key, one payload type, one token per operation. What it shows is
//! the shape every `SignedOp` wallet shares: embed a
//! [`SignedOpReplayCache`], let [`consume`](SignedOpReplayCache::consume) do
//! the verification and replay protection, apply the wallet's own policy to
//! `op.signer`, then mint. The tests pin the replay model — the chain's
//! `(nonce, block_expiry_height)` pair, consumed once, dead after expiry.

#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

use groundhog_signed_op::{Operation, SignedOp, SignedOpReplayCache};
use sdk::mintable::AuthTokenPreimage;
use sdk::signer::{Opaque, Signed};
use sdk::storage::Value;
use sdk::types::transaction::Uniquifier;
use sdk::types::{Ed25519PublicKey, Ed25519Signature};
use sdk::*;

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
    /// The operation's signer is not the key this wallet accepts.
    #[cfg_attr(feature = "dev", error("Signer is not the wallet owner"))]
    NotOwner = 2,
}

#[sdk::storage]
pub struct SignedOpWallet {
    owner: Value<Ed25519PublicKey>,
    replay: SignedOpReplayCache,
}

#[contract(client = SignedOpWalletClient, throws = WalletError)]
impl SignedOpWallet {
    /// Set the key this wallet accepts. Throws
    /// [`WalletError::AlreadyInitialized`] if called more than once.
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

#[contract(version = "0.1.0")]
impl SignedOp for SignedOpWallet {
    fn execute_signed_op(
        &mut self,
        op: Operation,
        signature: &Ed25519Signature,
    ) -> Signed<Opaque> {
        // Authority first: only the owner's operations are worth verifying.
        // A refusal throws and reverts, so nothing below leaves a mark.
        if op.signer != self.owner.get() {
            throw(WalletError::NotOwner);
        }

        // Replay, expiry, body decode and signature, against a preimage
        // bound to this network and this wallet; consumes the pair.
        let grant: Grant = self.replay.consume(&op, signature);

        let node =
            sdk::mintable::build_auth_token_node::<u64>(AuthTokenPreimage {
                consumer: get_caller(),
                data: grant.amount,
            });
        sdk::signer::mint_opaque_auth_token(
            sdk::block::get_block_height(),
            &sdk::mintable::encode_auth_token_node(&node),
        )
    }
}
