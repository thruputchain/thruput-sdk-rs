//! ERC721 non-fungible token: the standard interface, its metadata
//! extension, and ERC-165 interface detection on one storage struct, plus
//! an administrator-only mint and an owner-side burn.

#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

// The traits describing the contract interface
mod interface {
    pub use thruput_erc165_if::*;
    pub use thruput_erc721_if::*;
}

// Client stubs that provide the interfaces via cross-contract calls
pub use interface::{ERC165Client, ERC721Client, ERC721MetadataClient};

// An implementation of the contract interfaces
mod implementation;

// The actual implementation, exposed for use in tests
pub use implementation::{NftAdminClient, NftContract, NftError};

// Re-export the traits so tests can name `<NftContract as ERC721>`
pub use interface::{ERC721, ERC721Metadata};
