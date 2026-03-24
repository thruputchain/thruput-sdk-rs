mod common;
use common::*;

// Load two contracts into the same test environment, and
// have one call the other.
#[test]
fn caller_test_native() {
    let env = sdk::TestEnvironment::no_cost();

    let erc20_contract = env.register_mock(<erc20::TokenContract as erc20::ERC20>::__register_mock);
    let erc20_hash = env.setup_create_contract(&erc20_contract);

    let caller_contract = env.register_mock(caller::Caller::__register_mock);
    let caller_hash = env.setup_create_contract(&caller_contract);

    sdk::contract_exec(env, move || {
        caller_test_inner(&erc20_hash, &caller_hash);
    })
    .unwrap_or_else(|e| panic!("Caller test failed: {}", e));
}
