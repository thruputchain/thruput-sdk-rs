mod common;

use sdk::*;
use exceptions::test_setup;

#[test]
fn native_exceptions() {
    let mut env = sdk::TestEnvironment::default();

    // Register the contract with the test environment
    let test_contract = test_setup::register(&mut env);
    let contract_hash = env.setup_create_contract(&test_contract);

    contract_test_invoke(env, move || {
        with_abort_handler(&test_abort_handler::<Infallible>, || {
            common::exceptions_test(&contract_hash)
        })
    })
    .unwrap_or_else(|e|
        panic!("Test invocation failed: {}", e));
}

#[test]
fn native_panic() {
    let mut env = sdk::TestEnvironment::default();

    // Register the contract with the test environment
    let test_contract = test_setup::register(&mut env);
    let contract_hash = env.setup_create_contract(&test_contract);

    let result = sdk::contract_test_invoke(env, move || {
        sdk::with_abort_handler(&sdk::test_abort_handler::<Infallible>, || {
            common::panic_test(&contract_hash)
        })
    });

    if let Err(err) = result {
        panic!("Panic test failed: {}", err);
    }
}
