//! A contract that catches cross-contract exceptions **in guest code**.
//!
//! [`ExnCatch::classify`] catches through one `match_exn!` over
//! everything a caller can meet: two errors the interface declares, one it
//! does not, an SDK [`LibraryExn`](thruput_sdk::LibraryExn), and a fault.
#![cfg_attr(not(feature = "dev"), no_std)]

thruput_sdk::declare_mock_contract!();

mod implementation;
mod interface;

pub use implementation::Contract;
pub use interface::*;
