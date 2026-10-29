//! Standard ERC20 with optional issuer controls, holder burns and signed spending.
#![cfg_attr(not(feature = "dev"), no_std)]
thruput_sdk::declare_mock_contract!();
pub use thruput_erc20_controls_if::*;
pub use thruput_erc20_if::*;
pub use thruput_erc20_metadata_if::*;
pub use thruput_erc20_permit_if::*;
mod implementation;
pub use implementation::TokenContract;
