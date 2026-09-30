//! A wallet that authorizes ERC20 permits from off-chain signatures.
//!
//! This is the owner's half of the permit flow. The token contract never sees
//! a public key or a signature: a submitter brings the signed operation here,
//! [`SignedOp::execute_signed_op`] validates it and mints the grant auth token
//! the request asks for, naming the requested ERC20 as consumer, and the
//! submitter carries that token down into the token contract. Which key is
//! currently authoritative is therefore wallet state, not something derived
//! from the wallet's address.
//!
//! A signature binds exactly one payload type (the preimage covers its
//! `TYPE_DEF`), so the wallet signs [`WalletRequest`], one enum spanning every
//! operation it serves. Adding a variant changes that `TYPE_DEF` and retires
//! every outstanding signature.

#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

use groundhog_erc20_permit_if::{
    DeleteAllowanceGrant, DeleteAllowanceRequest, PermitGrant, PermitRequest,
};
use groundhog_signed_op::{Operation, SignedOp, SignedOpReplayCache};
use sdk::mintable::{AuthTokenPreimage, MintableType};
use sdk::signer::{Opaque, Signed};
use sdk::storage::Value;
use sdk::types::transaction::Uniquifier;
use sdk::types::{Ed25519PublicKey, Ed25519Signature};
use sdk::*;

/// What goes into a grant mint: `#[contract_type]` generates a preimage mirror
/// of every mintable type, and these name them.
type PermitGrantPreimage = <PermitGrant as MintableType>::Preimage;
type DeleteAllowanceGrantPreimage =
    <DeleteAllowanceGrant as MintableType>::Preimage;

/// The body the owner signs: which grant the wallet should mint.
#[contract_type]
pub enum WalletRequest {
    /// Mint a [`PermitGrant`]: a transfer or allowance of up to `amount`.
    Permit(PermitRequest) = 0,
    /// Mint a [`DeleteAllowanceGrant`] revoking the spender's allowance.
    DeleteAllowance(DeleteAllowanceRequest) = 1,
}

/// Why the wallet refused, as distinct from why an operation failed
/// verification — those are
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
    /// Who may rotate the owner key. A wallet with richer governance would put
    /// a multisig or a timelock here; the point is only that rotation is not
    /// open.
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
        // Expiry, replay, body decode and signature under the current owner
        // key, then the pair is consumed. A refusal throws and reverts the
        // frame, consuming nothing: a permit signed by a former key fails
        // as InvalidSignature while that key is not the owner, and is
        // honored again if the key is rotated back.
        let request: WalletRequest =
            self.replay.consume(&op, signature, &self.owner.get());

        // `consumer` comes from the signed body, so the owner picks which
        // token may consume this grant. Taking it from `get_caller()` instead
        // would let whoever submits the operation point it at a different
        // ERC20 the owner also holds.
        let node = match request {
            WalletRequest::Permit(request) => {
                sdk::mintable::build_auth_token_node::<PermitGrant>(
                    AuthTokenPreimage {
                        consumer: request.token,
                        data: PermitGrantPreimage {
                            spender: request.spender,
                            amount: request.amount,
                        },
                    },
                )
            }
            WalletRequest::DeleteAllowance(request) => {
                sdk::mintable::build_auth_token_node::<DeleteAllowanceGrant>(
                    AuthTokenPreimage {
                        consumer: request.token,
                        data: DeleteAllowanceGrantPreimage {
                            spender: request.spender,
                        },
                    },
                )
            }
        };

        // The submitter carries the token to the ERC20 in this same frame, so
        // it need not outlive the current block.
        sdk::signer::mint_opaque_auth_token(
            sdk::block::get_block_height(),
            &sdk::mintable::encode_auth_token_node(&node),
        )
    }
}
