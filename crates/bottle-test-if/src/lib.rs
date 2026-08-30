//! An interface for a test-contract that gets loaded "in a bottle" alongside an example
//! contract and can interrogate it.

#![cfg_attr(not(any(feature = "dev")), no_std)]
#![allow(clippy::result_large_err)]

use sdk::*;

/// The explicit `version` decouples this interface's ABI version from the SDK
/// release version the crate inherits: the macro otherwise defaults a trait's
/// version to its crate's, which would force every implementing contract's
/// `#[contract(version = ...)]` to be edited on each SDK bump — including the
/// ones in the `integration_test_contracts` submodule, which hogd builds too.
/// It moves only when the trait's own signatures do.
#[contract(client = BottleTestClient, version = "0.1.0")]
pub trait BottleTest {
    // Perform setup operations to be excluded from performance metrics
    fn setup(&mut self, target_contract_hash: &SHA256Hash);

    // Run the test
    fn run(&mut self);
}
