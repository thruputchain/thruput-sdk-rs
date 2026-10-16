#![cfg_attr(not(feature = "dev"), no_std)]

thruput_sdk::declare_mock_contract!();

mod implementation;
mod interface;

pub use implementation::VersionedCalleeContract;
pub use interface::VersionedCallee;
pub use interface::VersionedCalleeClient;
