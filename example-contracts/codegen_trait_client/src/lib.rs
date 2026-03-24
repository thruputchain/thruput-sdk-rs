#![cfg_attr(not(feature = "dev"), no_std)]

mod interface;
pub use interface::*;

// Verify that the custom client name "AdderProxy" is generated (not the default "AdderClient")
pub use interface::AdderProxy;

mod implementation;
