//! An ERC20 whose approvals can be authorized off-chain.
//!
//! The owner's wallet verifies a signed `PermitRequest` and mints a grant.
//! Its action selects either a standing allowance (`permit`) or an immediate
//! transfer (`transfer_from_signed`). Successful consumption identifies the
//! wallet as the owner; the token never handles the signature.

#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

mod implementation;

pub use groundhog_erc20_if::{ERC20, ERC20Client, ERC20Error};
pub use groundhog_erc20_permit_if::{
    ERC20Permit, ERC20PermitClient, ERC20PermitError, PermitAction,
    PermitGrant, PermitRequest,
};
pub use implementation::PermitToken;
