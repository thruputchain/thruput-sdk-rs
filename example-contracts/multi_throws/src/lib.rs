#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

mod interface;
pub use interface::*;

mod implementation;
pub use implementation::*;
