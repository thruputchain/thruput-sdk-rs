//! Example contract moving `thruput_sdk::U256` and `thruput_sdk::I256` values
//! through a contract entrypoint (argument and typed return) and
//! `storage::Value`.

#![cfg_attr(not(feature = "dev"), no_std)]

thruput_sdk::declare_mock_contract!();

mod implementation;

pub use implementation::U256Roundtrip;
pub use implementation::U256RoundtripClient;
pub use implementation::U256RoundtripEntrypoints;

#[cfg(feature = "bottle-test")]
mod bottle_test;
