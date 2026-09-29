//! Standard ERC20 with optional issuer controls, holder burns and signed spending.
#![cfg_attr(not(feature = "dev"), no_std)]
sdk::declare_mock_contract!();
pub use groundhog_erc20_controls_if::*;
pub use groundhog_erc20_if::*;
pub use groundhog_erc20_metadata_if::*;
pub use groundhog_erc20_permit_if::*;
type Allowances = sdk::storage::Map<
    sdk::ContractAddress,
    sdk::storage::Map<sdk::ContractAddress, sdk::storage::NNInt>,
>;
#[path = "../../issuer_common.rs"]
mod common;
mod implementation;
pub use common::TokenContract;
