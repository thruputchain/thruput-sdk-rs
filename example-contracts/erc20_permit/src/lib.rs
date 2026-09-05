//! An ERC20 whose approvals can be authorized off-chain.
//!
//! The owner never submits a transaction: they sign a
//! [`PermitRequest`](groundhog_erc20_permit_if::PermitRequest) and someone
//! else brings it here. This contract asks the owner's wallet to validate the
//! signature and takes the owner's identity from the resulting auth token's
//! minter address, which the runtime supplies. Nothing the submitter asserts
//! about who is approving is trusted.
//!
//! The two spending paths sit side by side deliberately. `permit` writes an
//! allowance, matching ERC-2612 and whatever already speaks `transfer_from`.
//! `transfer_from_signed` moves the tokens and writes nothing, which is what
//! the auth token was already: a capability good for one use. Ethereum has no
//! way to hand one over, so allowances are its substitute; here the allowance
//! is the lossy step.

#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

mod implementation;

pub use groundhog_erc20_if::{ERC20, ERC20Client, ERC20Error};
pub use groundhog_erc20_permit_if::{
    ERC20Permit, ERC20PermitClient, ERC20PermitError, PermitGrant,
    PermitRequest,
};
pub use implementation::PermitToken;
