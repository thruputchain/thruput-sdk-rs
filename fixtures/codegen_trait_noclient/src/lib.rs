#![cfg_attr(not(feature = "dev"), no_std)]

mod interface;
pub use interface::*;

// Verify that the default client name "{Trait}Client" is generated
pub use interface::AdderClient;

mod implementation;
