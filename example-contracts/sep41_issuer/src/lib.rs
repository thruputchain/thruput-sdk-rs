//! SEP-41 token operations with optional issuer controls and expiring allowances.
#![cfg_attr(not(feature = "dev"), no_std)]
sdk::declare_mock_contract!();
pub use groundhog_erc20_controls_if::*;
pub use groundhog_erc20_if::{
    Amount, BurnEvent, ERC20Error, MintEvent, TransferEvent, amount_to_diff,
};
pub use groundhog_sep41_if::*;
#[path = "../../issuer_common.rs"]
mod common;
mod implementation;
pub use common::TokenContract;
use implementation::Allowances;
pub use implementation::IssuerClient;
