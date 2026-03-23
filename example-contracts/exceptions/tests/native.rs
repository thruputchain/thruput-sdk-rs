mod common;

use exceptions::test_setup;
use sdk::*;

#[test]
#[ignore]
fn native_exceptions() {
    let mut env = sdk::TestEnvironment::default();

    // Register the contract with the test environment
    let test_contract = test_setup::register(&mut env);
    let contract_hash = env.setup_create_contract(&test_contract);

    contract_exec(env, move || common::exceptions_test(&contract_hash))
        .unwrap_or_else(|e| panic!("Test failed: {}", e));
}

#[test]
#[ignore] // XXX: segfault!
fn native_panic() {
    let mut env = sdk::TestEnvironment::default();

    // Register the contract with the test environment
    let test_contract = test_setup::register(&mut env);
    let contract_hash = env.setup_create_contract(&test_contract);

    contract_exec(env, move || common::panic_test(&contract_hash))
        .unwrap_or_else(|e| panic!("Test failed: {}", e));
}
