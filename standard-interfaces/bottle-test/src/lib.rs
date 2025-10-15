//! An interface for a test-contract that gets loaded "in a bottle" alongside an example
//! contract and can interrogate it.

#![no_std]

use sdk::{contract, SHA256Hash};

#[contract(client = BottleTestClient)]
pub trait BottleTest {
    fn run(&mut self, target_contract_hash: &SHA256Hash);
}
