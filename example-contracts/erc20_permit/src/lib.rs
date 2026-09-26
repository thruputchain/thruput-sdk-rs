//! An ERC20 with wallet-authorized signed transfers.
//!
//! The wallet validates the signature and mints a transfer grant. The token
//! consumes the grant to identify the wallet as the owner.

#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

mod implementation;

pub use groundhog_erc20_if::{ERC20, ERC20Client, ERC20Error};
pub use groundhog_erc20_permit_if::{
    ERC20Permit, ERC20PermitClient, ERC20PermitError, PermitGrant,
    PermitRequest,
};
pub use implementation::PermitToken;
