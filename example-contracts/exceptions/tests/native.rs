mod common;

use exceptions::{Contract, Interface};
use sdk::*;

#[test]
fn native_exceptions() {
    let env = sdk::TestEnvironment::default();

    // Register the contract with the test environment
    let test_contract = env.register_mock(<Contract as Interface>::__mock);
    let contract_hash = test_contract.as_hash().clone();

    contract_exec(env, move || common::exceptions_test(&contract_hash))
        .unwrap_or_else(|e| panic!("Test failed: {:?}", e));
}

#[test]
fn native_panic() {
    let env = sdk::TestEnvironment::default();

    // Register the contract with the test environment
    let test_contract = env.register_mock(<Contract as Interface>::__mock);
    let contract_hash = test_contract.as_hash().clone();

    contract_exec(env, move || common::panic_test(&contract_hash))
        .unwrap_or_else(|e| panic!("Test failed: {:?}", e));
}
