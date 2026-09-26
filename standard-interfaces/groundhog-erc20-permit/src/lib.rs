#![cfg_attr(not(feature = "dev"), no_std)]
#![allow(clippy::result_large_err)]

//! Signed ERC20 transfers authorized by the owner's wallet.
//!
//! The wallet validates a signed [`PermitRequest`] and mints a [`PermitGrant`]
//! with the token contract as consumer. The spender passes the grant to
//! [`ERC20Permit::transfer_from_signed`]. Successful consumption verifies the
//! consumer, identifies the wallet as the owner, and prevents grant reuse.
//! The token contract does not handle keys or signatures.
//!
//! Any holder contract can mint a grant under its own authorization policy.
//! A signed-operation wallet must check expiry and bound the grant's lifetime
//! to it when the transfer must occur by a deadline. The reference wallet
//! mints grants valid only in the current block.

mod error;
pub use error::*;

use groundhog_erc20_if::{Amount, ERC20Error};
use sdk::signer::Signed;
use sdk::{ContractAddress, contract};

/// The signed-operation body a wallet validates before minting a transfer grant.
/// Its `TYPE_DEF` is part of the signed preimage, alongside the network,
/// wallet address, nonce and expiry.
#[sdk::contract_type]
pub struct PermitRequest {
    /// The token contract allowed to consume the grant.
    pub token: ContractAddress,
    /// The caller allowed to redeem the grant and choose the receiver.
    pub spender: ContractAddress,
    pub amount: Amount,
}

/// Authorization for one transfer. The runtime supplies the owner as the
/// minter address and restricts consumption to the named token contract.
#[sdk::contract_type]
pub struct PermitGrant {
    pub spender: ContractAddress,
    pub amount: Amount,
}

/// An ERC20 that accepts transfer grants from holder contracts.
///
/// The base implementation throws [`ERC20PermitError::NotSpender`] for an
/// unauthorized caller and [`ERC20Error::TooLarge`] if the amount exceeds
/// `i128::MAX` or the owner's balance. Issuer-policy extensions may reject
/// transfers with their own errors.
#[contract(
    client = ERC20PermitClient,
    throws(ERC20PermitError, ERC20Error)
)]
pub trait ERC20Permit {
    /// Consume the grant and transfer its amount from its minter to `receiver`.
    /// The caller must be the grant's `spender`. Stored allowances are unchanged.
    fn transfer_from_signed(
        &mut self,
        grant: Signed<PermitGrant>,
        receiver: &ContractAddress,
    );
}
