#![cfg_attr(not(feature = "mock"), no_std)]

mod implementation;

pub use implementation::ContractClient;

#[cfg(feature = "mock")]
pub use implementation::test_setup;

#[cfg(feature = "bottle-test")]
mod bottle_test;
