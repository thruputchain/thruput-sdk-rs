//! ERC20 contract interface and implementation

#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

// A trait describing the contract interface
mod interface {
    pub use thruput_erc20_if::*;

    #[cfg(feature = "metadata")]
    pub use thruput_erc20_metadata_if::*;
}

// A client stub that provides the interface via cross-contract calls
pub use interface::ERC20Client;

#[cfg(feature = "metadata")]
pub use interface::ERC20MetadataClient;

// An implementation of the contract interface
mod implementation;

// The actual implementation, exposed for use in tests
pub use implementation::TokenContract;

// Re-export the ERC20 trait so tests can write `dyn erc20::ERC20`
pub use interface::ERC20;

#[cfg(feature = "bottle-test")]
pub use erc20_bottle_test::*;
