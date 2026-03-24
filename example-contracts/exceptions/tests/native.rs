mod common;

use exceptions::{Contract, Interface};
use sdk::*;

#[test]
fn native_exceptions() {
    let env = sdk::TestEnvironment::default();

    // Register the contract with the test environment
    let test_contract = env.register_mock(<Contract as Interface>::__register_mock);
    let contract_hash = env.setup_create_contract(&test_contract);

    contract_exec(env, move || common::exceptions_test(&contract_hash))
        .unwrap_or_else(|e| panic!("Test failed: {}", e));
}

#[test]
fn native_panic() {
    let env = sdk::TestEnvironment::default();

    // Register the contract with the test environment
    let test_contract = env.register_mock(<Contract as Interface>::__register_mock);
    let contract_hash = env.setup_create_contract(&test_contract);

    contract_exec(env, move || common::panic_test(&contract_hash))
        .unwrap_or_else(|e| panic!("Test failed: {}", e));
}
