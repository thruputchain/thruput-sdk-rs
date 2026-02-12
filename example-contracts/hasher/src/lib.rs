#![cfg_attr(not(feature = "dev"), no_std)]

mod implementation;

pub use implementation::ContractClient;

#[cfg(feature = "dev")]
pub use implementation::test_setup;

#[cfg(feature = "bottle-test")]
mod bottle_test;
