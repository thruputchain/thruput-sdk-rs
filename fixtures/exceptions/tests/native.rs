mod common;

use thruput_sdk::*;

#[test]
fn native_exceptions() {
    let env = thruput_sdk::TestEnvironment::default();

    // Register the contract with the test environment
    let test_contract = exceptions::register_mock_contract(&env);
    let code = CodeHash::of_mock(&test_contract);

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test"))
            .run(move || common::exceptions_test(&code))
    })
    .assert_success();
}

#[test]
fn native_panic() {
    let env = thruput_sdk::TestEnvironment::default();

    // Register the contract with the test environment
    let test_contract = exceptions::register_mock_contract(&env);
    let code = CodeHash::of_mock(&test_contract);

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test"))
            .run(move || common::panic_test(&code))
    })
    .assert_success();
}
