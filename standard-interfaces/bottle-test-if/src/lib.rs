//! An interface for a test-contract that gets loaded "in a bottle" alongside an example
//! contract and can interrogate it.

#![cfg_attr(not(any(feature = "dev")), no_std)]

use sdk::*;

#[contract(client = BottleTestClient)]
pub trait BottleTest {
    // Perform setup operations to be excluded from performance metrics
    fn setup(&mut self, target_contract_hash: &SHA256Hash);

    // Run the test
    fn run(&mut self);
}
