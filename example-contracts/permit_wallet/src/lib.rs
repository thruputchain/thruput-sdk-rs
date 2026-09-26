//! A wallet that verifies signed ERC20 transfer requests and mints grants.
//!
//! The accepted signing key is wallet state and can rotate independently of
//! the address holding the balance. This example accepts one payload type;
//! a wallet supporting several operations would need to distinguish them in
//! the signed payload.

#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

use groundhog_erc20_permit_if::{PermitGrant, PermitRequest};
use groundhog_signed_op::{Operation, SignedOp, SignedOpReplayCache};
use sdk::mintable::{AuthTokenPreimage, MintableType};
use sdk::signer::{Opaque, Signed};
use sdk::storage::Value;
use sdk::types::transaction::Uniquifier;
use sdk::types::{Ed25519PublicKey, Ed25519Signature};
use sdk::*;

type PermitGrantPreimage = <PermitGrant as MintableType>::Preimage;

/// Wallet administration errors. Signature and replay errors use
/// [`SignedOpError`](groundhog_signed_op::SignedOpError).
#[contract_exception]
#[non_exhaustive]
#[cfg_attr(feature = "dev", derive(thiserror::Error))]
pub enum WalletError {
    #[cfg_attr(feature = "dev", error("Wallet already initialized"))]
    AlreadyInitialized = 1,
    /// `rotate_owner` called by anyone other than the administrator.
    #[cfg_attr(
        feature = "dev",
        error("Caller is not the wallet administrator")
    )]
    NotAdministrator = 2,
}

#[sdk::storage]
pub struct PermitWallet {
    administrator: Value<ContractAddress>,
    owner: Value<Ed25519PublicKey>,
    replay: SignedOpReplayCache,
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

    /// Replace the accepted key. Only the administrator may call this.
    /// Rotating back to a former key restores its unconsumed, unexpired requests.
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

    /// Drop a consumed uniquifier once its expiry has passed. Open to anyone:
    /// it can only remove entries for operations that could no longer execute.
    pub fn prune(&mut self, block_expiry_height: BlockHeight, nonce: u64) {
        self.replay.prune(&Uniquifier {
            block_expiry_height,
            nonce,
        });
    }
}

#[contract(version = "0.1.0")]
impl SignedOp for PermitWallet {
    fn execute_signed_op(
        &mut self,
        op: Operation,
        signature: &Ed25519Signature,
    ) -> Signed<Opaque> {
        let request: PermitRequest =
            self.replay.consume(&op, signature, &self.owner.get());

        // Bind consumption to the signed token address, not the submitter.
        let node = sdk::mintable::build_auth_token_node::<PermitGrant>(
            AuthTokenPreimage {
                consumer: request.token,
                data: PermitGrantPreimage {
                    spender: request.spender,
                    amount: request.amount,
                },
            },
        );

        // A grant must not outlive the signed operation's expiry.
        sdk::signer::mint_opaque_auth_token(
            sdk::block::get_block_height(),
            &sdk::mintable::encode_auth_token_node(&node),
        )
    }
}
