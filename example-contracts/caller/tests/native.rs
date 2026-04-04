mod common;
use common::*;

// Load two contracts into the same test environment, and
// have one call the other.
#[test]
fn caller_test_native() {
    let env = sdk::TestEnvironment::no_cost();

    let erc20_contract = erc20::register_mock_contract(&env);
    let erc20_hash = erc20_contract.as_hash().clone();

    let caller_contract = caller::register_mock_contract(&env);
    let caller_hash = caller_contract.as_hash().clone();

    sdk::contract_exec(env, move || {
        caller_test_inner(&erc20_hash, &caller_hash);
    })
    .unwrap_or_else(|e| panic!("Caller test failed: {}", e));
}
