//! A shared conformance suite for ERC20 contracts on Thruput.
//!
//! [`erc20_test_setup`] deploys the contract under test, and [`erc20_test`]
//! runs the suite against it, returning a [`Model`] of the expected state and
//! event log. [`check_events`] compares that log with the events a
//! transaction actually emitted. Enable `metadata` to also check the ERC20
//! metadata extension.

#![no_std]

extern crate alloc;

mod contract_test;
mod model;

pub use contract_test::*;
pub use model::*;
