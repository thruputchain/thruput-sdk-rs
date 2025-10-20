//! An interface for a test-contract that gets loaded "in a bottle" alongside an example
//! contract and can interrogate it.

#![no_std]

use sdk::{SHA256Hash, contract};

#[contract(client = BottleTestClient)]
pub trait BottleTest {

    // Perform setup operations to be excluded from performance metrics
    fn setup(&mut self, target_contract_hash: &SHA256Hash);

    // Run the test
    fn run(&mut self);
}
