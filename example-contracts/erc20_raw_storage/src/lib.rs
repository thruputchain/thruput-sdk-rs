//! ERC20 implementation without storage feature

#![cfg_attr(not(feature = "dev"), no_std)]

thruput_sdk::declare_mock_contract!();

use thruput_erc20_if as interface;

// A client stub that provides the interface via cross-contract calls
pub use interface::ERC20Client;

// An implementation of the contract interface
mod implementation;

// The actual implementation, exposed for use in tests
pub use implementation::Token;

// Re-export the ERC20 trait so tests can write `dyn erc20_raw_storage::ERC20`
pub use interface::ERC20;
