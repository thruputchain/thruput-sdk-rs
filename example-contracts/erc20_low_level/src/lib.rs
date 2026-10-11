//! ERC20 low-level implementation

#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

use thruput_erc20_if as interface;

// A client stub that provides the interface via cross-contract calls
pub use interface::ERC20Client;

// An implementation of the contract interface
mod implementation;

// Entrypoints
pub use implementation::*;

#[cfg(feature = "bottle-test")]
pub use erc20_bottle_test::*;
