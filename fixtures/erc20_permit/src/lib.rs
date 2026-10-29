//! An ERC20 whose approvals can be authorized off-chain.
//!
//! The owner never submits a transaction: they sign a
//! [`PermitRequest`](thruput_erc20_permit_if::PermitRequest) and someone
//! else brings it here. This contract asks the owner's wallet to validate the
//! signature and takes the owner's identity from the resulting auth token's
//! minter address, which the runtime supplies. Nothing the submitter asserts
//! about who is approving is trusted.
//!
//! The spending paths sit side by side deliberately. `transfer_from_signed`
//! moves up to the granted amount without writing an allowance. The auth token
//! is a capability good for one use. `increase_allowance_signed`
//! turns a grant into a standing allowance for spenders that draw later or
//! through `transfer_from`, and `delete_allowance_signed` takes one back.

#![cfg_attr(not(feature = "dev"), no_std)]

thruput_sdk::declare_mock_contract!();

mod implementation;

pub use thruput_erc20_if::{ERC20, ERC20Client, ERC20Error};
pub use thruput_erc20_permit_if::{
    DeleteAllowanceGrant, DeleteAllowanceRequest, ERC20Permit,
    ERC20PermitClient, ERC20PermitError, PermitGrant, PermitRequest,
};
pub use implementation::PermitToken;
