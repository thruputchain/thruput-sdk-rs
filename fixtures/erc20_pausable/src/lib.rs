//! Pausable ERC20 contract with metadata and role-based access control.
//!
//! Combines four standard interfaces on one storage struct: the ERC20
//! token core, the metadata extension, the pausable extension, and
//! role-based access control gating who may pause, plus ERC-165 interface
//! detection read off the interface table those impls leave behind.

#![cfg_attr(not(feature = "dev"), no_std)]

thruput_sdk::declare_mock_contract!();

// The traits describing the contract interface
mod interface {
    pub use thruput_access_control_if::*;
    pub use thruput_erc20_if::*;
    pub use thruput_erc20_metadata_if::*;
    pub use thruput_erc20_pausable_if::*;
    pub use thruput_erc165_if::*;
}

// Client stubs that provide the interfaces via cross-contract calls
pub use interface::{
    AccessControlClient, ERC20Client, ERC20MetadataClient,
    ERC20PausableClient, ERC165Client,
};

// An implementation of the contract interfaces
mod implementation;

// The actual implementation, exposed for use in tests
pub use implementation::{PAUSER_ROLE, PausableTokenContract};

// Re-export the traits so tests can name them
pub use interface::{
    AccessControl, ERC20, ERC20Metadata, ERC20Pausable, ERC165,
};
