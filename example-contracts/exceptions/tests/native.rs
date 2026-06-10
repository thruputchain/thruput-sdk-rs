mod common;

use sdk::*;

#[test]
fn native_exceptions() {
    let env = sdk::TestEnvironment::default();

    // Register the contract with the test environment
    let test_contract = exceptions::register_mock_contract(&env);
    let contract_hash = test_contract.as_hash().clone();

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test"))
            .run(move || common::exceptions_test(&contract_hash))
    })
    .assert_success();
}

#[test]
fn native_panic() {
    let env = sdk::TestEnvironment::default();

    // Register the contract with the test environment
    let test_contract = exceptions::register_mock_contract(&env);
    let contract_hash = test_contract.as_hash().clone();

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test"))
            .run(move || common::panic_test(&contract_hash))
    })
    .assert_success();
}
