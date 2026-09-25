#![cfg_attr(not(feature = "dev"), no_std)]
#![allow(clippy::result_large_err)]

//! Off-chain approval for ERC20 tokens: the Groundhog analogue of ERC-2612.
//!
//! An owner signs a [`PermitRequest`] off-chain; anyone submits it. The token
//! never sees the owner's key, and never sees the signature either. The
//! submitter takes the signed operation to the owner's wallet — a contract
//! implementing `SignedOp` — which validates it under its own policy and
//! mints a [`PermitGrant`] auth token naming the token contract as consumer.
//! The submitter passes that token down into [`ERC20Permit::permit`] or
//! [`ERC20Permit::transfer_from_signed`], and the token contract consumes it,
//! learning the owner from the runtime as the minter address. Nothing the
//! submitter says about who is approving is trusted; a grant cannot reach any
//! contract but its named consumer, and cannot be consumed twice.
//!
//! Taking a `Signed<PermitGrant>` rather than an operation and a signature is
//! what keeps this interface independent of *how* the owner authorizes. A
//! `SignedOp` wallet is one minter; an ERC1271 wallet, a multisig, a
//! governance contract or any other holder of the balance can mint the same
//! grant, and the token cannot tell them apart — nor needs to. Rotation,
//! multiple keys and weighted thresholds are the minter's business, and a key
//! it no longer accepts stops producing grants the moment it says so.
//! Deriving an owner address from a signing key would instead pin the key an
//! account was *created* with, and keep honoring a rotated-out one.
//!
//! # Two ways to spend
//!
//! [`ERC20Permit::permit`] writes an allowance, so the approval outlives the
//! call and is spent later through the ordinary
//! [`ERC20::transfer_from`](groundhog_erc20_if::ERC20::transfer_from). Use it
//! when something downstream already speaks allowances.
//!
//! [`ERC20Permit::transfer_from_signed`] moves the tokens in the same call and
//! writes no allowance. Prefer it: an auth token is already a one-shot
//! capability, so turning one into a standing allowance makes a durable copy
//! of authority that was meant to be spent once. ERC-2612 has no such option —
//! allowances are Ethereum's workaround for not being able to hand over a
//! capability at all.

mod error;
pub use error::*;

use groundhog_erc20_if::{Amount, ERC20Error};
use sdk::signer::Signed;
use sdk::{ContractAddress, contract};

/// The body an owner signs to approve a spender: the payload of the signed
/// operation their wallet verifies.
///
/// The token contract never sees this type; it is the contract between the
/// owner, their wallet and any off-chain signer. Its `TYPE_DEF` goes into the
/// signed-operation preimage, so a client that reproduces these fields in a
/// different order, or under a different name, produces a signature no permit
/// wallet will accept.
///
/// `token` is load-bearing. The preimage binds the network and the *verifying
/// contract* — the wallet — but says nothing about which token the approval
/// is for, so without naming it here a submitter could have the grant minted
/// for a different ERC20 the owner also holds. The wallet uses it as the
/// grant's consumer.
#[sdk::contract_type]
pub struct PermitRequest {
    /// The token contract the approval is for.
    pub token: ContractAddress,
    /// Who may spend.
    pub spender: ContractAddress,
    /// How much they may spend.
    pub amount: Amount,
}

/// The auth token a wallet mints from a validated [`PermitRequest`], consumed
/// by the token contract.
///
/// It carries no owner: the consumer reads that from the minter address that
/// [`Signed::consume`] returns, which the runtime supplies and no caller can
/// forge. It carries no token address either — the mint names the token as
/// the node's consumer, so no other contract can consume it.
#[sdk::contract_type]
pub struct PermitGrant {
    pub spender: ContractAddress,
    pub amount: Amount,
}

/// An ERC20 that accepts approvals as auth tokens.
///
/// Both methods take a [`PermitGrant`] token and treat its minter as the
/// owner. The caller obtained it from the owner's wallet in this same frame —
/// typically `SignedOp::execute_signed_op` — and passes it down; the token
/// contract has no opinion about how it was minted.
///
/// # Error contract
///
/// Implementations must throw exactly these:
///
/// - [`ERC20PermitError::NotSpender`] — `transfer_from_signed` called by
///   anyone other than the grant's `spender`.
/// - [`ERC20Error::TooLarge`] — the owner's balance cannot cover the grant.
///
/// A grant minted for another token never reaches the implementation: the
/// runtime refuses to consume a token at any contract but its named consumer.
#[contract(
    client = ERC20PermitClient,
    throws(ERC20PermitError, ERC20Error)
)]
pub trait ERC20Permit {
    /// Consume `grant` and record the resulting allowance for its minter, to
    /// be spent later through `transfer_from`.
    ///
    /// Replace the remaining allowance with the granted amount, as with
    /// [`ERC20::approve`](groundhog_erc20_if::ERC20::approve). Zero revokes.
    /// Competing grants for the same owner/spender must conflict, even if
    /// their amounts are identical. Expiry limits submission of the signed
    /// operation, not the lifetime of the stored allowance.
    fn permit(&mut self, grant: Signed<PermitGrant>);

    /// Consume `grant` and immediately transfer the granted amount from its
    /// minter to `receiver`, writing no allowance.
    ///
    /// The caller must be the grant's `spender` — the same authority
    /// `transfer_from` gives a spender under an allowance, including the
    /// choice of `receiver`, with nothing left behind afterwards.
    fn transfer_from_signed(
        &mut self,
        grant: Signed<PermitGrant>,
        receiver: &ContractAddress,
    );
}
