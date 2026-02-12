//! ERC20 contract interface and implementation

#![cfg_attr(not(feature = "dev"), no_std)]

// A trait describing the contract interface
use groundhog_erc20_if as interface;

// A client stub that provides the interface via cross-contract calls
pub use interface::ERC20Client;

// An implementation of the contract interface
mod implementation;

// Contract methods exposed with C ABI for running as guest
pub use implementation::exports::*;

#[cfg(feature = "dev")]
pub use implementation::test_setup;

#[cfg(feature = "bottle-test")]
pub use erc20_bottle_test::*;
