#![cfg_attr(not(feature = "dev"), no_std)]

thruput_sdk::declare_mock_contract!();

mod implementation;

pub use implementation::Stateless;
pub use implementation::StatelessClient;
pub use implementation::StatelessEntrypoints;

pub mod interface1;
pub mod interface2;
