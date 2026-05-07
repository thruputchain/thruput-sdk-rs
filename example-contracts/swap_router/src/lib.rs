#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

extern crate alloc;

// Interface
pub use router::RouterClient;

// Implementation
mod cfg;
mod router;
pub use router::Router;
