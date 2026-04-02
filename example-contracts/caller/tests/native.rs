mod common;
use common::*;

// Load two contracts into the same test environment, and
// have one call the other.
#[test]
fn caller_test_native() {
    let env = sdk::TestEnvironment::no_cost();

    let erc20_contract =
        env.register_mock(<erc20::TokenContract as erc20::ERC20>::__mock);
    let erc20_hash = erc20_contract.as_hash().clone();

    let caller_contract = env.register_mock(caller::Caller::__mock);
    let caller_hash = caller_contract.as_hash().clone();

    sdk::contract_exec(env, move || {
        caller_test_inner(&erc20_hash, &caller_hash);
    })
    .unwrap_or_else(|e| panic!("Caller test failed: {}", e));
}
