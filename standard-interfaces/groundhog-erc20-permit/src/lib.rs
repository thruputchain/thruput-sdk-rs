#![cfg_attr(not(feature = "dev"), no_std)]
#![allow(clippy::result_large_err)]

//! Off-chain approval for ERC20 tokens: the Groundhog analogue of ERC-2612.
//!
//! An owner signs a request off-chain; anyone submits it. The token never
//! sees the owner's key, and never sees the signature either. The submitter
//! takes the signed operation to the owner's wallet — a contract implementing
//! `SignedOp` — which validates it under its own policy and mints a grant auth
//! token naming the token contract as consumer. The submitter passes that
//! token down into an [`ERC20Permit`] method, and the token contract consumes
//! it, learning the owner from the runtime as the minter address. Nothing the
//! submitter says about who is approving is trusted; a grant cannot reach any
//! contract but its named consumer, and cannot be consumed twice.
//!
//! Taking a `Signed<...>` grant rather than an operation and a signature is
//! what keeps this interface independent of *how* the owner authorizes. A
//! `SignedOp` wallet is one minter; an ERC1271 wallet, a multisig, a
//! governance contract or any other holder of the balance can mint the same
//! grant, and the token cannot tell them apart — nor needs to. Rotation,
//! multiple keys and weighted thresholds are the minter's business, and a key
//! it no longer accepts stops producing grants the moment it says so.
//! Deriving an owner address from a signing key would instead pin the key an
//! account was *created* with, and keep honoring a rotated-out one.
//!
//! # A grant is a one-shot allowance
//!
//! An auth token lives only as long as the transaction that minted it, and is
//! spent by the one call that consumes it. That already makes it an allowance
//! scoped to a single use, so [`ERC20Permit::transfer_from_signed`] spends it
//! directly — up to the granted amount, with nothing left behind — and is the
//! path to prefer. ERC-2612 has no such option: allowances are Ethereum's
//! workaround for not being able to hand over a capability at all.
//!
//! A standing allowance is still what a spender needs when it draws later, in
//! another transaction, or through code that only speaks
//! [`ERC20::transfer_from`](groundhog_erc20_if::ERC20::transfer_from). For
//! that, [`ERC20Permit::increase_allowance_signed`] turns a grant into
//! allowance, and [`ERC20Permit::clear_allowance_signed`] is the owner's signed
//! way to take it back.

mod error;
pub use error::*;

use groundhog_erc20_if::{Amount, ERC20Error};
use sdk::signer::Signed;
use sdk::{ContractAddress, contract};

/// What an owner approves to grant a spender up to `amount`: the request a
/// wallet validates and turns into a [`PermitGrant`].
///
/// The token contract never sees this type; it is the contract between the
/// owner, their wallet and any off-chain signer. The body the owner actually
/// signs is the wallet's own type carrying this one (in the example
/// `permit_wallet`, a variant of its `WalletRequest` enum), and that type's
/// `TYPE_DEF` goes into the signed-operation preimage. It covers these fields,
/// so a client that reproduces them in a different order, or under a
/// different name, produces a signature the wallet will not accept.
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
    /// The most they may spend.
    pub amount: Amount,
}

/// The auth token a wallet mints from a validated [`PermitRequest`], consumed
/// by [`ERC20Permit::transfer_from_signed`] or
/// [`ERC20Permit::increase_allowance_signed`].
///
/// It carries no owner: the consumer reads that from the minter address that
/// [`Signed::consume`] returns, which the runtime supplies and no caller can
/// forge. It carries no token address either — the mint names the token as
/// the node's consumer, so no other contract can consume it.
#[sdk::contract_type]
pub struct PermitGrant {
    pub spender: ContractAddress,
    /// The most the grant authorizes; see each consuming method for how it is
    /// spent.
    pub amount: Amount,
}

/// What an owner approves to revoke a spender's standing allowance: the
/// counterpart of [`PermitRequest`], turned into a [`ClearAllowanceGrant`] and
/// signed the same way, inside the wallet's own body type.
///
/// A separate type rather than a [`PermitRequest`] with a zero amount, so the
/// signed body says which operation the owner approved: a grant signed to
/// raise an allowance can never be redeemed to clear one, or the reverse.
/// `token` is load-bearing for the same reason it is in [`PermitRequest`].
#[sdk::contract_type]
pub struct ClearAllowanceRequest {
    /// The token contract whose allowance is cleared.
    pub token: ContractAddress,
    /// Whose allowance is cleared.
    pub spender: ContractAddress,
}

/// The auth token a wallet mints from a validated [`ClearAllowanceRequest`],
/// consumed by [`ERC20Permit::clear_allowance_signed`]. Like [`PermitGrant`],
/// it carries neither the owner nor the token.
#[sdk::contract_type]
pub struct ClearAllowanceGrant {
    pub spender: ContractAddress,
}

/// An ERC20 that accepts approvals as auth tokens.
///
/// Every method takes a grant token and treats its minter as the owner. The
/// caller obtained it from the owner's wallet in this same transaction —
/// typically `SignedOp::execute_signed_op` — and passes it down; the token
/// contract has no opinion about how it was minted.
///
/// # Error contract
///
/// Implementations must throw exactly these:
///
/// - [`ERC20PermitError::NotSpender`] — `transfer_from_signed` called by
///   anyone other than the grant's `spender`.
/// - [`ERC20PermitError::ExceedsGrant`] — `transfer_from_signed` asked to move
///   more than the grant's `amount`.
/// - [`ERC20Error::TooLarge`] — the owner's balance cannot cover the transfer,
///   or an allowance would leave the representable range.
///
/// A grant minted for another token never reaches the implementation: the
/// runtime refuses to consume a token at any contract but its named consumer.
#[contract(
    client = ERC20PermitClient,
    throws(ERC20PermitError, ERC20Error)
)]
pub trait ERC20Permit {
    /// Consume `grant` and transfer `amount` from its minter to `receiver`,
    /// writing no allowance.
    ///
    /// The caller must be the grant's `spender`, and `amount` may not exceed
    /// the grant's: the same authority `transfer_from` gives a spender under
    /// an allowance of `grant.amount`, including the choice of `receiver`.
    /// The grant is spent whatever `amount` is; an unused remainder is not
    /// carried anywhere.
    fn transfer_from_signed(
        &mut self,
        grant: Signed<PermitGrant>,
        receiver: &ContractAddress,
        amount: Amount,
    );

    /// Consume `grant` and increase its minter's allowance for the grant's
    /// `spender` by `grant.amount`, to be spent later through `transfer_from`.
    ///
    /// Adds to the allowance rather than setting it, exactly as a positive
    /// [`ERC20::adjust_allowance`](groundhog_erc20_if::ERC20::adjust_allowance)
    /// by the owner would, and emits the same approval event. Unlike
    /// ERC-2612's `permit`, which overwrites, two grants for the same spender
    /// sum.
    fn increase_allowance_signed(&mut self, grant: Signed<PermitGrant>);

    /// Consume `grant` and delete its minter's allowance for the grant's
    /// `spender`, emitting the approval event for the amount removed.
    ///
    /// The entry is deleted rather than zeroed, freeing its storage, and
    /// deleted under an exclusive lock: another transaction in the same block
    /// that writes the same allowance conflicts at commit, so no concurrent
    /// grant or spend can land beside the clear. A write later in the same
    /// transaction still applies after it.
    fn clear_allowance_signed(&mut self, grant: Signed<ClearAllowanceGrant>);
}
