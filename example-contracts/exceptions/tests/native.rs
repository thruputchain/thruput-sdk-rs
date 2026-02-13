mod common;

use common::*;
use sdk::contract_test::*;
use sdk::*;
use exceptions::test_setup;
use core::convert::Infallible;

#[test]
fn native_exceptions() {
    let mut env = TestEnvironment::new(None);

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
