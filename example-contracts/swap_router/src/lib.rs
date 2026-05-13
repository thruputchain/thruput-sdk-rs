#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

extern crate alloc;

// Interface
pub use router::{RouterClient, SwapRouterError};
pub use library::SwapLibraryError;

// Implementation
mod library;
mod cfg;
mod router;

// Testing
pub use router::Router;
