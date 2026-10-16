//! An interface for a test-contract that gets loaded "in a bottle" alongside an example
//! contract and can interrogate it.

#![cfg_attr(not(any(feature = "dev")), no_std)]
#![allow(clippy::result_large_err)]

use thruput_sdk::*;

// Held at 0.1.0 deliberately: the ABI version otherwise defaults to the
// crate version, and every implementor would have to be re-pinned on each
// SDK minor bump. Move it when the trait signatures move.
#[contract(client = BottleTestClient, version = "0.1.0")]
pub trait BottleTest {
    // Perform setup operations to be excluded from performance metrics
    fn setup(&mut self, target_contract_hash: &SHA256Hash);

    // Run the test
    fn run(&mut self);
}
