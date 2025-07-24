//! ERC20 contract interface and implementation

#![cfg_attr(not(feature = "mock"), no_std)]

// A trait describing the contract interface
pub use erc20_if as interface;

// A client stub that provides the interface via cross-contract calls
pub use interface::ERC20Client;

// An implementation of the contract interface
#[cfg(feature = "guest")]
pub mod implementation;

// Contract methods exposed with C ABI for running as guest
#[cfg(feature = "guest")]
pub use implementation::guest_exports::*;

#[cfg(feature = "mock")]
pub use implementation::test_setup;
