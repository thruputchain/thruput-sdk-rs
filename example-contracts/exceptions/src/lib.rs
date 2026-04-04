#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_package!();

// The contract interface
mod interface;
pub use interface::*;

// An implementation of the contract interface
mod implementation;
pub use implementation::*;
