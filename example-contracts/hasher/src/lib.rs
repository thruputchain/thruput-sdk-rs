#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

mod implementation;

pub use implementation::HasherClient;
pub use implementation::HasherImplEntrypoints;

pub use implementation::HasherImpl;

#[cfg(feature = "bottle-test")]
mod bottle_test;
