//! An ERC20 conformance suite. [`erc20_test_setup`] deploys the contract
//! under test, and [`erc20_test`] drives it through the ERC20 operations,
//! checking every result against an in-memory [`Model`]. The model's
//! expected event log is then compared with what the contract emitted, using
//! [`check_events`].
//!
//! The suite runs inside a contract frame. `thruput-erc20-native-test` runs it
//! in the native test environment.

#![no_std]

extern crate alloc;

mod contract_test;
mod model;

pub use contract_test::*;
pub use model::*;
